//! The movie player (`melee/lb/lbmthp.c`): plays an `.mth` file, a header and THP frames each
//! carrying the next one's size, through a ring of 32 frame buffers read from the disc one
//! after another, an alarm that moves the movie on every 60th of a second by a rate table,
//! and a sprite whose render callback decodes the frame due into three planes and draws them.
//!
//! The player's state is the audio driver's neighbour on the timeline: its disc reads go
//! through the device command queue, each read's callback (`fn_8001E910`) asks for the next
//! from the disc interrupt, and its alarm (`fn_8001F2A4`) runs as an OS alarm on the time
//! base's grid. So it lives in [`crate::audio::Audio`], whose timeline runs both
//! (`clock::EventKind::MovieAlarm`, `devcom::cb::MOVIE_READ`).
//!
//! What the reads bring into the ring is the file's bytes, which the player keeps as each
//! slot's place in the file; the decode (`thp`) is presentation and the planes' memory, so
//! the player keeps which frame the planes hold for the frontend to decode again.

use melee_data::archive::{ArchiveId, Archives};

use crate::audio::Audio;
use crate::audio::devcom::cb;

/// `MoviePlayer`'s address: the header is read into its first 0x40 bytes.
pub const PLAYER: u32 = 0x8043_33E0;
/// The ring's slots (`unk_104`).
pub const SLOTS: u32 = 0x20;
/// `fn_8001F2A4`, the alarm's handler, and its period: a 60th of a second.
pub const ALARM_HANDLER: u32 = 0x8001_F2A4;
pub const ALARM_PERIOD: u64 = 675_000;
/// `lbMthp_8001F67C`, the movie sprite's render callback.
pub const RENDER: u32 = 0x8001_F67C;

/// An `.mth` file's header (`THPDecComp`'s first 0x40 bytes), as read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Header {
    pub words: [u32; 16],
}

impl Header {
    pub fn version(&self) -> u32 {
        self.words[2]
    }
    pub fn buf_size(&self) -> u32 {
        self.words[3]
    }
    pub fn x_size(&self) -> u32 {
        self.words[4]
    }
    pub fn y_size(&self) -> u32 {
        self.words[5]
    }
    pub fn num_frames(&self) -> u32 {
        self.words[7]
    }
    pub fn first_frame(&self) -> u32 {
        self.words[8]
    }
    pub fn frame_offsets(&self) -> u32 {
        self.words[9]
    }
    pub fn first_frame_size(&self) -> u32 {
        self.words[10]
    }
}

/// The frame the movie's planes hold, for the presentation to decode
/// (`thp::decode_frame`) and draw: the record's bytes in the file and the image's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct MovieFrame {
    pub file: ArchiveId,
    pub at: u32,
    pub len: u32,
    pub width: u16,
    pub height: u16,
}

/// A rate table: runs of (frames, 60ths of a second each), and its address.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Rate {
    pub addr: u32,
    pub runs: Vec<(u32, u32)>,
}

/// `MoviePlayer` (`THPDecComp`). Field names follow what each does; the original's offsets
/// are in their docs.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MoviePlayer {
    /// 0x14C: a movie is open.
    pub power: bool,
    /// 0x00: the file's header.
    pub header: Header,
    /// The file's disc entry (0x128) and its bytes' archive.
    pub entrynum: i32,
    pub file: Option<ArchiveId>,
    /// 0x40, 0x44, 0x48: the frame count and the image's size.
    pub num_frames: u32,
    pub width: u32,
    pub height: u32,
    /// 0x4C: the ring's pointer table; 0x50, 0x54, 0x58: the planes; 0x98: the decode work
    /// area (each an address in the movie's buffer).
    pub ring_table: u32,
    pub plane_y: u32,
    pub plane_u: u32,
    pub plane_v: u32,
    pub work: u32,
    /// 0x64.
    pub x64: u32,
    /// 0x68: the movie loops.
    pub looping: i32,
    /// 0x6C, 0x11C: set at open.
    pub x6c: i32,
    pub x11c: i32,
    /// 0x70: reads may go on (cleared to close).
    pub reading: i32,
    /// 0x74: frames asked of the disc (the next frame's number, mod the frame count).
    pub requested: u32,
    /// 0x78: the frame the alarm has shown; 0x7C, 0x84: the game's copies of it and of 0x80.
    pub shown: u32,
    pub x7c: u32,
    /// 0x80: the alarm's 60ths since the start, the movie's clock.
    pub clock: u32,
    pub x84: u32,
    /// 0x88: the ring slot of the shown frame; 0x8C: the slot the next read fills; 0x90: the
    /// slot the game takes to decode; 0x94: the slot last decoded (-1 none).
    pub shown_slot: u32,
    pub write_slot: u32,
    pub decode_slot: u32,
    pub decoded_slot: i32,
    /// 0x9C: the image the work area is sized for; 0xA8: the decode's header.
    pub desc: crate::thp::ImageDesc,
    pub x_size: u16,
    pub y_size: u16,
    pub xac: u8,
    /// 0x100: a slot's size; 0x104: the slots.
    pub buf_size: u32,
    pub slots: u32,
    /// 0x108: frames in the ring not yet shown; 0x10C: the fewest there were.
    pub buffered: i32,
    pub min_buffered: i32,
    /// 0x110: a read is in flight.
    pub in_flight: i32,
    /// 0x120: the next read's place in the file; 0x124: its size.
    pub file_offset: u32,
    pub packed_size: u32,
    /// 0x12C: the rate table.
    pub rate: Rate,
    /// 0x130, 0x134: the last read's time (its sign, and the time base's ticks since its
    /// request); 0x138; 0x13C: when the read in flight was asked for.
    pub x130: i32,
    pub read_time: i32,
    pub x138: u32,
    pub requested_at: u32,
    /// 0x140: the buffer the player allocated (0 when the caller gave one).
    pub alloc: u32,
    /// 0x144: the movie has ended; 0x148: the sprite draws it.
    pub ended: i32,
    pub draw: i32,
    /// Each slot's frame: its place in the file and its read's size.
    pub ring: Vec<(u32, u32)>,
    /// The frame the planes hold (its place in the file and its record's size), for the
    /// presentation: the planes' bytes are what decoding it makes.
    pub planes: Option<(u32, u32)>,
    /// The alarm is set on the timeline.
    pub alarm: bool,
}

impl MoviePlayer {
    /// `lbMthp_8001F87C`, at boot: no movie open.
    #[doc(alias = "lbMthp_8001F87C")]
    pub fn init(&mut self) {
        self.power = false;
    }

    /// `lbMthp_8001F5C4`: the movie's clock as the game last took it.
    #[doc(alias = "lbMthp_8001F5C4")]
    pub fn game_clock(&self) -> u32 {
        self.x84
    }

    /// `lbMthp_8001F604`: whether the movie has ended.
    #[doc(alias = "lbMthp_8001F604")]
    pub fn has_ended(&self) -> bool {
        self.ended != 0
    }

    /// `lbMthp_8001F614`: whether the sprite draws the movie.
    #[doc(alias = "lbMthp_8001F614")]
    pub fn set_draw(&mut self, on: i32) {
        self.draw = on;
    }

    /// `lbMthp_8001F578`: the game takes the alarm's frame, its slot and the clock.
    #[doc(alias = "lbMthp_8001F578")]
    pub fn take_frame(&mut self) {
        self.decode_slot = self.shown_slot;
        self.x7c = self.shown;
        self.x84 = self.clock;
    }

    /// `lbMthp_GetFrame`: the frame due at `clock` 60ths of a second by the rate table.
    fn frame_at(&self, clock: u32) -> u32 {
        let mut counter = clock;
        let mut frame = 0u32;
        if self.rate.runs.is_empty() {
            return counter;
        }
        for &(count, ticks) in &self.rate.runs {
            let total = count.wrapping_mul(ticks);
            if counter >= total {
                frame = frame.wrapping_add(count);
                counter -= total;
            } else {
                return frame.wrapping_add(counter / ticks);
            }
        }
        unreachable!("a rate table ends in a run longer than any movie")
    }

    /// `fn_8001F06C`: the shown frame moves on when one is buffered.
    #[doc(alias = "fn_8001F06C")]
    fn advance(&mut self) {
        if self.buffered > 0 || self.x6c == 0 {
            self.shown = self.shown.wrapping_add(1);
            if self.shown == self.num_frames {
                if self.looping != 0 {
                    self.shown = 0;
                } else {
                    self.shown = self.shown.wrapping_sub(1);
                    return;
                }
            }
            self.shown_slot += 1;
            self.buffered -= 1;
            if self.shown_slot >= self.slots {
                self.shown_slot = 0;
            }
        }
        if self.buffered < self.min_buffered {
            self.min_buffered = self.buffered;
        }
    }

    /// `fn_8001F2A4`, the alarm: the movie's clock moves on when the frame it shows is the
    /// one due (stopping at the end unless it loops), and the shown frame follows the clock.
    #[doc(alias = "fn_8001F2A4")]
    pub fn alarm(&mut self) {
        if self.shown == self.frame_at(self.clock) {
            self.clock = self.clock.wrapping_add(1);
            if self.num_frames == self.frame_at(self.clock) {
                if self.looping != 0 {
                    self.clock = 0;
                } else {
                    self.clock = self.clock.wrapping_sub(1);
                    self.ended = 1;
                }
            }
        }
        if self.shown != self.frame_at(self.clock) {
            self.advance();
        }
    }

    /// The frame the planes hold, as the sprite draws it.
    pub fn planes_draw(&self) -> Option<MovieFrame> {
        let (at, len) = self.planes?;
        Some(MovieFrame {
            file: self.file?,
            at,
            len,
            width: self.width as u16,
            height: self.height as u16,
        })
    }

    /// The ring slot `i`'s address.
    pub fn slot_addr(&self, i: u32) -> u32 {
        self.ring_table + (SLOTS * 4).next_multiple_of(32) + i * self.buf_size
    }

    /// The bytes of the record read into slot `i`: the file from the slot's place.
    pub fn slot_bytes<'a>(&self, data: &'a Archives, i: u32) -> &'a [u8] {
        let (at, size) = self.ring[i as usize];
        let bytes = data.get(self.file.expect("the movie's file")).data();
        let end = (at as usize + size as usize).min(bytes.len());
        &bytes[(at as usize).min(end)..end]
    }

    /// The next frame's size, which the frame in slot `i` starts with.
    fn next_size(&self, data: &Archives, i: u32) -> u32 {
        let b = self.slot_bytes(data, i);
        u32::from_be_bytes(b[..4].try_into().expect("a frame's record"))
    }
}

/// `fn_8001EBF0`: the buffer's size for the ring, the planes and the decode, with the decode's
/// image and the ring's counters set.
#[doc(alias = "fn_8001EBF0")]
fn memory_required(m: &mut MoviePlayer) -> u32 {
    m.slots = SLOTS;
    let mut size = m.buf_size.next_multiple_of(32).wrapping_mul(m.slots);
    let wh = m.width.wrapping_mul(m.height);
    m.desc = crate::thp::ImageDesc {
        val0: 0,
        width: m.width as u16,
        height: m.height as u16,
        format: 4,
    };
    size = size.wrapping_add(wh);
    size = size.wrapping_add(wh >> 2);
    size = size.wrapping_add(wh >> 2);
    size = size.wrapping_add(crate::thp::work_size(&m.desc, m.height as u16) as u32);
    m.x7c = 0;
    m.shown = 0;
    m.x84 = 0;
    m.clock = 0;
    m.decode_slot = 0;
    m.shown_slot = 0;
    m.decoded_slot = -1;
    m.looping = 0;
    m.x_size = m.width as u16;
    m.y_size = m.height as u16;
    m.xac = 0;
    size = size.wrapping_add((m.slots * 4).next_multiple_of(32));
    size.wrapping_add((m.num_frames * 4).next_multiple_of(32))
}

impl Audio {
    /// `lbMthp_8001F410`: opens movie `entrynum` (its bytes `file`) at `rate`, its buffer from
    /// the heap: the header and the ring's first 32 frames read (the game waits for each), the
    /// counters set and the alarm started.
    #[doc(alias = "lbMthp_8001F410")]
    #[doc(alias = "fn_8001EB14")]
    #[doc(alias = "fn_8001ECF4")]
    #[doc(alias = "THPInit")]
    pub fn mthp_open(
        &mut self,
        data: &Archives,
        entrynum: i32,
        file: ArchiveId,
        rate: Rate,
        looping: i32,
    ) {
        assert!(
            !self.movie.power,
            "the original's assertion failed: lbmthp.c:833"
        );
        self.movie.power = true;
        // `fn_8001EB14`: `THPInit` (the locked cache and the paired singles' conversions, the
        // CPU's), the header read in.
        self.movie.entrynum = entrynum;
        self.movie.file = Some(file);
        self.file_load(data, entrynum, 0, PLAYER, 0x40, 0x21, 1);
        let bytes = data.get(file).data();
        let mut header = Header::default();
        for (i, w) in header.words.iter_mut().enumerate() {
            *w = u32::from_be_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap());
        }
        let m = &mut self.movie;
        m.header = header;
        m.num_frames = header.num_frames();
        m.width = header.x_size();
        m.height = header.y_size();
        m.buf_size = header.buf_size();
        // "Warning : frame offsets not supported", "Warning : file format is newer than
        // player": reports only.
        let _ = (header.frame_offsets(), header.version());
        m.x11c = 1;
        m.x6c = 1;
        m.in_flight = 0;
        m.reading = 1;
        m.read_time = 0;
        m.x130 = 0;
        m.rate = rate;
        let size = memory_required(m);
        let buf = crate::hsd::heap::mem_alloc(size);
        let m = &mut self.movie;
        m.alloc = buf;
        m.looping = looping;
        self.mthp_setup_buffer(data, buf);
        let m = &mut self.movie;
        m.ended = 0;
        m.draw = 1;
        // `OSCreateAlarm`, `OSSetPeriodicAlarm`: every 60th of a second on the time base's
        // grid from a 60th after boot.
        self.movie_alarm_start();
    }

    /// `fn_8001ECF4`: the buffer carved into the ring's table, its slots, the planes and the
    /// work area, and the ring filled from the first frame, each read waited for.
    fn mthp_setup_buffer(&mut self, data: &Archives, buf: u32) {
        let m = &mut self.movie;
        m.ring_table = buf;
        m.x64 = 0;
        let mut at = buf + (m.slots * 4).next_multiple_of(32);
        m.ring = vec![(0, 0); m.slots as usize];
        if m.x6c != 0 && m.x11c != 0 {
            let mut size = m.header.first_frame_size();
            m.file_offset = m.header.first_frame();
            let mut i = 0;
            while i < self.movie.slots {
                assert!(size != 0, "the original's assertion failed: lbmthp.c:266");
                let m = &self.movie;
                let (entry, offset) = (m.entrynum, m.file_offset);
                let len = size.next_multiple_of(32);
                self.movie.ring[i as usize] = (offset, len);
                self.file_load(data, entry, offset, at, len, 0x21, 1);
                let m = &mut self.movie;
                m.file_offset += size;
                size = m.next_size(data, i);
                at += m.buf_size;
                i += 1;
            }
            let m = &mut self.movie;
            m.requested = i;
            m.packed_size = size;
            if m.requested >= m.num_frames {
                m.requested = 0;
            }
            m.write_slot = 0;
            m.buffered = m.slots as i32 - 1;
            m.min_buffered = m.slots as i32 - 1;
        }
        let m = &mut self.movie;
        let y = m.width * m.height;
        m.plane_y = at;
        at += y;
        m.plane_u = at;
        at += y >> 2;
        m.plane_v = at;
        at += y >> 2;
        m.work = at;
    }

    /// The time base as a read of it gives it now (`OSGetTime`), the read's time passing
    /// after.
    pub fn tb_read(&mut self) -> u64 {
        let now = self.hw_now();
        self.hw.spent += crate::game::session::TB_READ;
        // The code's time base counts from the clock at power-on (`Clock::code_time`; the
        // clock is out of `self` while a handler runs, so the setup's copy).
        now.wrapping_add(self.power_on)
    }

    /// `OSGetTick`: the time base's low word.
    pub fn os_get_tick(&mut self) -> u32 {
        self.tb_read() as u32
    }

    /// `fn_8001F13C`: the next frame asked of the disc into the ring's next slot, when the
    /// ring has room, no read is in flight and the player reads; `fn_8001E910` is its
    /// callback.
    #[doc(alias = "fn_8001F13C")]
    pub fn mthp_request(&mut self, data: &Archives) {
        let m = &self.movie;
        if m.decode_slot != m.write_slot && m.in_flight == 0 && m.reading != 0 {
            let t = self.os_get_tick();
            let m = &mut self.movie;
            m.requested_at = t;
            m.x138 = 0;
            if m.requested != m.num_frames {
                self.mthp_read_next(data);
            }
        }
    }

    /// The read of the next frame into the write slot (`HSD_DevComRequest`), and the ring's
    /// counters on.
    fn mthp_read_next(&mut self, data: &Archives) {
        let m = &self.movie;
        assert!(
            m.packed_size != 0,
            "the original's assertion failed: lbmthp.c:289 (filnum = {}, ofs = {}, by sugano.)",
            m.entrynum,
            m.file_offset
        );
        let (entry, offset, slot) = (m.entrynum, m.file_offset, m.write_slot);
        let len = m.packed_size.next_multiple_of(32);
        let dest = m.slot_addr(slot);
        self.movie.ring[slot as usize] = (offset, len);
        self.devcom_request(
            data,
            entry,
            offset,
            dest,
            len,
            0x21,
            1,
            Some(cb::MOVIE_READ),
            0,
        );
        let m = &mut self.movie;
        m.requested += 1;
        if m.requested == m.num_frames && m.looping != 0 {
            m.requested = 0;
        }
        m.write_slot += 1;
        if m.write_slot >= m.slots {
            m.write_slot = 0;
        }
        m.in_flight = 1;
    }

    /// `fn_8001E910`, a read's callback (in the disc interrupt): the frame is buffered, the
    /// next one's place and size known from it, and the next read asked for while the ring has
    /// room and the player reads.
    #[doc(alias = "fn_8001E910")]
    pub fn mthp_read_done(&mut self, data: &Archives, cancelled: bool) {
        assert!(!cancelled, "the original's assertion failed: lbmthp.c:328");
        // The time base as the device command queue's memory callback enters (ssbm-rs's
        // DVD_TIMELINE: `OSGetTick` returns the callback's entry time).
        let t = self.os_get_tick();
        let m = &mut self.movie;
        let diff = t.wrapping_sub(m.requested_at) as i32;
        m.read_time = diff;
        m.x130 = diff >> 31;
        m.buffered += 1;
        if m.requested != 0 {
            m.file_offset = m.file_offset.wrapping_add(m.packed_size);
        } else {
            m.file_offset = m.header.first_frame();
        }
        let last = if m.write_slot == 0 {
            m.slots - 1
        } else {
            m.write_slot - 1
        };
        m.packed_size = m.next_size(data, last);
        let m = &self.movie;
        if m.decode_slot != m.write_slot && m.reading != 0 {
            let t = self.os_get_tick();
            let m = &mut self.movie;
            m.requested_at = t;
            m.x138 = 0;
            if m.requested != m.num_frames {
                self.mthp_read_next(data);
            } else {
                self.movie.in_flight = 0;
            }
            return;
        }
        self.movie.in_flight = 0;
    }

    /// `fn_8001EF5C`, from the sprite's render callback: the frame the game took decoded into
    /// the planes once (the decode itself is the presentation's, [`MoviePlayer::planes`]),
    /// then the next read asked for.
    #[doc(alias = "fn_8001EF5C")]
    #[doc(alias = "THPVideoDecode")]
    pub fn mthp_decode(&mut self, data: &Archives) {
        let m = &mut self.movie;
        if m.decoded_slot as u32 != m.decode_slot {
            let slot = m.decode_slot;
            m.planes = Some(m.ring[slot as usize]);
            m.decoded_slot = slot as i32;
            self.mthp_request(data);
        }
    }

    /// `lbMthp_8001F800`: closes the movie: reads stop, the one in flight is waited for, the
    /// alarm cancelled, the shown frames flushed and the buffer freed.
    #[doc(alias = "lbMthp_8001F800")]
    #[doc(alias = "fn_8001F294")]
    pub fn mthp_close(&mut self, data: &Archives) {
        if !self.movie.power {
            return;
        }
        self.movie.reading = 0;
        // `fn_8001F294`'s spin: a wait point each spin until the read's callback has run.
        if self.movie.in_flight != 0 {
            self.with_clock(data, |c, f| {
                c.wait_until(f, crate::audio::clock::Wait::MovieIdle)
            });
        }
        self.movie_alarm_cancel();
        // `HSD_VIWaitXFBFlush`: the scene's loop has ended and its frames shown already.
        if self.movie.alloc != 0 {
            crate::hsd::heap::mem_free(self.movie.alloc);
        }
        self.movie.power = false;
    }

    /// The alarm set on the timeline's grid (`OSSetPeriodicAlarm` from 1/60 s, every 1/60 s).
    fn movie_alarm_start(&mut self) {
        // `__OSGetSystemTime` in `InsertAlarm` reads the time base.
        let now = self.hw_now();
        self.hw.spent += crate::game::session::TB_READ;
        let c = self
            .clock
            .as_mut()
            .expect("the movie's alarm without the timeline");
        let first = c.next_period(now, ALARM_PERIOD);
        c.set_movie_alarm(first, ALARM_PERIOD);
        self.movie.alarm = true;
    }

    fn movie_alarm_cancel(&mut self) {
        if std::mem::take(&mut self.movie.alarm)
            && let Some(c) = self.clock.as_mut()
        {
            c.cancel_movie_alarm();
        }
    }

    /// The time base now, as [`Audio::os_get_tick`] counts it, without a read.
    fn hw_now(&self) -> u64 {
        self.hw_base
            .unwrap_or_else(|| self.clock.as_ref().map_or(0, |c| c.now))
            + self.hw.spent
    }
}

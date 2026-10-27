//! Camera Mode's snapshots as `lbsnap.c` writes them: the frame's copy compressed into the
//! snapshot's buffer with its header (`lbSnap_8001DC0C`), the card's banner made from it
//! (`lbSnap_8001DA5C`), the blocks it takes (`lbSnap_8001DF20`), its name from the clock
//! (`lbSnap_8001D4A4`) and its save to a card (`lbSnap_8001DF6C`, `lb_8001BB48`).
//!
//! The frame's copy (`cmsnap.c`'s `lb_800122C8`) is the console's: its pixels are the
//! renderer's, an input the frontend hands the simulation ([`give_frame`]). A console without
//! a renderer (ssbm-rs, whose traces the oracle compares with) copies nothing, so the buffer
//! keeps what it held; that is what a session without a frame takes ([`Running::cm_snap_copy`]).

use std::cell::RefCell;

use melee_data::archive::DataRef;

use super::save::{CardList, result};
use crate::game::state::Running;
use crate::hsd::heap;
use crate::hsd::image::Src;

/// The snapshot's image: 640 by 480 RGB565 (GX's 4 by 4 tiles).
pub const WIDTH: u16 = 640;
pub const HEIGHT: u16 = 480;
pub const IMAGE_SIZE: u32 = 640 * 480 * 2;
/// `Unk80433380_0`'s header before the compressed image (`x38`), and the image's room.
pub const HEADER: u32 = 0x38;
pub const ROOM: u32 = 256_000;
/// The card banner's bytes (96 by 32 RGB5A3) and the icon's with its palette.
const BANNER: usize = 0x1800;
const ICONS: usize = 0x600;
/// `lbSnap_803BACC8`'s file: kept once (flags 3).
const SNAP_FLAGS: i32 = 3;

/// The snapshot's header (`Unk80433380_0` up to its data): its kind (4), size, the quality
/// it was compressed at, the compressed image's bytes, the stage, which item kinds were out
/// (`it_8026C47C`) and which fighter kinds (`ft_GetFtKindMask`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapHeader {
    pub x0: i32,
    pub width: u16,
    pub height: u16,
    pub x8: u8,
    pub xc: i32,
    pub stkind: u16,
    pub items: [i32; 8],
    pub fighters: i32,
}

impl SnapHeader {
    /// The bytes as memory holds them (the padding the struct leaves as the buffer held it
    /// is the caller's).
    fn fields(&self) -> Vec<(u32, Vec<u8>)> {
        let mut items = Vec::new();
        for w in self.items {
            items.extend_from_slice(&w.to_be_bytes());
        }
        vec![
            (0, self.x0.to_be_bytes().to_vec()),
            (4, self.width.to_be_bytes().to_vec()),
            (6, self.height.to_be_bytes().to_vec()),
            (8, vec![self.x8]),
            (0xC, self.xc.to_be_bytes().to_vec()),
            (0x10, self.stkind.to_be_bytes().to_vec()),
            (0x14, items),
            (0x34, self.fighters.to_be_bytes().to_vec()),
        ]
    }
}

/// The snapshot's save to a card under way: the list's steps, each a call of
/// `lbCardNew_CompleteNextTask` from Camera Mode's menu, and the next. The card takes the
/// list's writes at its last step, its directory stamped with the times the steps that stamp
/// it (`stamps`) came to (`times`); the list is made of `request`.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapSave {
    pub list: CardList,
    pub next: usize,
    pub request: SnapRequest,
    pub stamps: Vec<usize>,
    pub times: Vec<u32>,
    pub seconds: u32,
}

/// What the snapshot's card file is made of (`lb_8001BB48`'s arguments): its name, its data's
/// size and bytes (the buffer's, past the data as far as its last block), its comment, banner
/// and icon.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapRequest {
    pub name: Vec<u8>,
    pub size: i32,
    pub data: Vec<u8>,
    pub comment: Vec<u8>,
    pub banner: Vec<u8>,
    pub icons: Vec<u8>,
}

impl SnapRequest {
    /// The task list's run over the request's files.
    fn run(&self) -> melee_save::tasks::Run {
        let mut bufs = melee_save::exec::Bufs::default();
        bufs.files[0] = self.data.clone();
        bufs.banner = self.banner.clone();
        bufs.icons = self.icons.clone();
        let mut run =
            melee_save::tasks::Run::new(melee_save::melee::snapshot_manifest(self.size), bufs);
        run.comment.copy_from_slice(&self.comment);
        run
    }

    fn tasks(&self) -> Vec<(melee_save::tasks::Task, u32)> {
        melee_save::tasks::lists::create(&self.name)
    }
}

/// `lbsnap.c`'s writing side's state: the snapshot as `lbSnap_8001DC0C` made it (its header,
/// the compressed image, the card file's comment, `filename`), the card banner it drew into
/// `LbMcSnap`'s icon data, the encoder's quality (`hsd_3B34.c`'s `lbl_804D6398`), and a save
/// under way.
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapWrite {
    pub header: Option<SnapHeader>,
    pub image: Vec<u8>,
    pub filename: Option<[u8; 0x40]>,
    pub banner: Option<Vec<u8>>,
    pub quality: i32,
    pub save: Option<SnapSave>,
}

impl Default for SnapWrite {
    fn default() -> SnapWrite {
        SnapWrite {
            header: None,
            image: Vec::new(),
            filename: None,
            banner: None,
            quality: 3,
            save: None,
        }
    }
}

thread_local! {
    /// The console's last frame copied for a snapshot ([`give_frame`]), and whether the
    /// frontend renders (it gives the frames) or not (the copy writes nothing, so the frame
    /// is what the image's buffer held before its first copy: kept with the buffer's address).
    static FRAME: RefCell<(bool, Option<Vec<u8>>, u32)> = const { RefCell::new((false, None, 0)) };
}

/// The frontend renders and gives each snapshot's frame: `pixels`, 640 by 480 RGB565 in GX's
/// tiles as the EFB copy writes them, for the copy the display pass asked for.
pub fn give_frame(pixels: Vec<u8>) {
    assert_eq!(
        pixels.len(),
        IMAGE_SIZE as usize,
        "a snapshot's frame's size"
    );
    FRAME.with(|f| *f.borrow_mut() = (true, Some(pixels), 0));
}

/// A new console: no frame copied, and none the frontend gives until it says so
/// ([`give_frame`]).
pub fn forget_frame() {
    FRAME.with(|f| *f.borrow_mut() = (false, None, 0));
}

/// The frame the copy left in the snapshot's image.
fn take_frame() -> Option<Vec<u8>> {
    FRAME.with(|f| f.borrow().1.clone())
}

impl Running<'_> {
    /// `lb_800122C8` for the frame's copy (`cmSnap_80031640`): a draw that copies the frame
    /// into the snapshot's image, whose bytes are then the GPU's (`Src::RenderTarget`). A
    /// console without a renderer copies nothing: the frame is what the buffer held.
    #[doc(alias = "lb_800122C8")]
    pub fn cm_snap_copy(&mut self) {
        let at = self.game.camera_mode.snap.image;
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::SnapCopy);
        let (renders, kept) = FRAME.with(|f| {
            let f = f.borrow();
            (f.0, f.1.is_some() && f.2 == at)
        });
        // Without a renderer, a buffer copied into before still holds what it held then.
        if !renders && !kept {
            let data = self.data;
            let held = heap::with(|h| h.image.bytes(data, at, IMAGE_SIZE));
            let (bytes, known) = held.expect("the heap's image");
            if let Some(i) = known.iter().position(|k| !k) {
                crate::game::code::unported_call(&format!(
                    "the snapshot's frame at {:#010x}: the heap's image doesn't know its byte {i:#x}",
                    at
                ));
            }
            FRAME.with(|f| *f.borrow_mut() = (false, Some(bytes), at));
        }
        heap::with(|h| h.image.hold(at, at + IMAGE_SIZE, Src::RenderTarget));
    }

    /// `lbSnap_8001DC0C`: the snapshot made from the frame's copy at `image`: its header (the
    /// stage, the items and fighters out), the image compressed at quality 3 into its buffer
    /// (`hsd_803B51C8`), the card banner drawn from it and the card file's comment dated from
    /// the clock. Whether the compressed image fit.
    #[doc(alias = "lbSnap_8001DC0C")]
    #[doc(alias = "hsd_803B5C2C")]
    #[doc(alias = "ft_GetFtKindMask")]
    pub fn snap_make(&mut self, image: u32) -> i32 {
        let _ = image;
        let snap = self.game.snap.snap.expect("the snapshot's buffer");
        let stkind = self.game.vs_stkind();
        let items = crate::game::it::spawn::kinds_out(self.game);
        let fighters = self.fighter_kinds();
        let w = &mut self.game.snap_write;
        w.quality = crate::hsd::jpeg::quality(3);
        let frame = take_frame().expect("the snapshot's frame (the copy made none)");
        let (size, bytes) = crate::hsd::jpeg::encode(
            &frame,
            i32::from(WIDTH),
            i32::from(HEIGHT),
            ROOM as usize,
            w.quality,
        );
        let header = SnapHeader {
            x0: 4,
            width: WIDTH,
            height: HEIGHT,
            x8: 3,
            xc: size as i32,
            stkind,
            items,
            fighters,
        };
        w.header = Some(header);
        heap::with(|h| {
            for (off, b) in header.fields() {
                h.image.put(snap + off, &b);
            }
            h.image.put(snap + HEADER, &bytes);
        });
        self.game.snap_write.image = bytes;
        self.snap_banner(&frame);
        // `OSGetTime`, then the calendar (`OSTicksToCalendarTime`).
        self.game
            .audio
            .clock_read(self.data, crate::game::session::TB_READ);
        let seconds = self.session_seconds();
        self.game.snap_write.filename = Some(snapshot_comment(
            seconds,
            crate::game::gm::language_setting_is_jp(),
        ));
        i32::from(size != 0)
    }

    /// `ft_GetFtKindMask`: a bit for each fighter kind out, Master Hand's and past left out.
    fn fighter_kinds(&self) -> i32 {
        const FT_KIND_MASTER_HAND: i32 = 0x1B;
        let mut mask = 0u32;
        let mut cur = self.game.world.first_in(8);
        while let Some(g) = cur {
            let k = self.game.fighter(g).kind;
            if k < FT_KIND_MASTER_HAND {
                mask |= 1 << k;
            }
            cur = self.game.world.gobjs[g.0].next;
        }
        mask as i32
    }

    /// `LbMcSnap`'s icon data: the banner's and the icon's places in the file
    /// (`MemSnapIconData`'s two pointers).
    fn snap_icons(&self) -> (DataRef, DataRef) {
        let d = self.game.snap.icon_data.expect("LbMcSnap's icon data");
        let banner = self.data.ptr(d).expect("the snapshot's banner");
        let icon = self.data.ptr(d.at(4)).expect("the snapshot's icon");
        (banner, icon)
    }

    /// `lbSnap_8001DA5C`: the card banner's middle 64 columns (of its 96 by 32, RGB5A3 in 4 by
    /// 4 tiles) drawn from the frame's 448 by 204 pixels at (96, 138), every seventh column
    /// and every 204/32nd row, into `LbMcSnap`'s banner.
    #[doc(alias = "lbSnap_8001DA5C")]
    fn snap_banner(&mut self, src: &[u8]) {
        let (at, _) = self.snap_icons();
        let mut banner = match self.game.snap_write.banner.take() {
            Some(b) => b,
            None => (0..BANNER as u32).map(|i| self.data.u8(at.at(i))).collect(),
        };
        let px = |offset: usize| u16::from_be_bytes([src[offset], src[offset + 1]]);
        let to5a3 = |p: u16| (p & 0x1F) | ((p >> 1) & 0x7FE0) | 0x8000;
        for row in 0..32usize {
            let src_row = row * 204 / 32 + 138;
            let dst_row = (row % 4) * 8;
            let dst_tile_row = (row / 4) * 24;
            let mut col_accum = 0usize;
            for pair in 0..32usize {
                for half in 0..2 {
                    let pixel_column = col_accum / 64 + 96;
                    let tile = pixel_column / 4 + (src_row / 4) * 160;
                    let offset = (tile << 5) + ((src_row % 4) << 3) + ((pixel_column % 4) << 1);
                    let column = pair * 2 + half + 16;
                    let d = dst_row + ((column / 4 + dst_tile_row) << 5) + ((column % 4) << 1);
                    banner[d..d + 2].copy_from_slice(&to5a3(px(offset)).to_be_bytes());
                    col_accum += 448;
                }
            }
        }
        let data = self.data;
        crate::hsd::export::with_image(|img| {
            if let Some(a) = img.data_addr(at) {
                img.put(a, &banner);
            }
        });
        let _ = data;
        self.game.snap_write.banner = Some(banner);
    }

    /// `lbSnap_8001DF20` (`lb_8001C4A8` with the snapshot's file): the blocks the snapshot's
    /// card file takes, its header (comment, banner and icon) and its compressed image kept
    /// once.
    #[doc(alias = "lbSnap_8001DF20")]
    #[doc(alias = "lb_8001C4A8")]
    pub fn snap_blocks(&mut self) -> i32 {
        let size = self.snap_file_size();
        let mut state = melee_save::state::CardState::new(0, 0x2000, 0);
        state.set_icon_info(&melee_save::melee::icon_info());
        state.set_file(0, size, SNAP_FLAGS, 1);
        state.sectors_needed()
    }

    /// The snapshot's card file's size: its header and compressed image
    /// (`lbSnap_GetSaveDataOffset`).
    fn snap_file_size(&self) -> i32 {
        let h = self.game.snap_write.header.expect("the snapshot's header");
        h.xc + HEADER as i32
    }

    /// `lbSnap_8001D4A4`: slot `chan`'s new snapshot's name: the clock's seconds, one more for
    /// each listed snapshot of that name met, until none is.
    #[doc(alias = "lbSnap_8001D4A4")]
    pub fn snap_name(&mut self, chan: usize) -> String {
        self.game
            .audio
            .clock_read(self.data, crate::game::session::TB_READ);
        let mut time = self.session_seconds();
        let slot = &self.game.snap.slots[chan];
        let num = slot.num.expect("a listed slot") as usize;
        loop {
            let hit = slot.entries[..num]
                .iter()
                .position(|e| e.time == Some(time));
            match hit {
                Some(_) => time = time.wrapping_add(1),
                None => break,
            }
        }
        time.to_string()
    }

    /// `lbSnap_8001DF6C`: the snapshot saved to slot `chan` (`lb_8001BB48`), its file named
    /// by [`Running::snap_name`] and holding its header and image; unless the slot's last
    /// listing failed or a card came or went since (then 8). The list's first result.
    #[doc(alias = "lbSnap_8001DF6C")]
    #[doc(alias = "lb_8001BB48")]
    pub fn snap_save(&mut self, chan: usize) -> i32 {
        let s = &mut self.game.snap;
        if s.slots[chan].card_result == 0 && s.state_changed[chan] != 0 {
            s.slots[chan].card_result = 8;
        }
        let r = s.slots[chan].card_result;
        if r != 0 {
            return r;
        }
        s.slots[chan].card_result = 8;
        assert_eq!(
            chan, 0,
            "a snapshot saved to slot B (no card is ever in it)"
        );
        let name = self.snap_name(chan);
        let size = self.snap_file_size();
        let snap = self.game.snap.snap.expect("the snapshot's buffer");
        // The file's data as the create writes it: its last block whole, so past the image
        // what the buffer holds after it.
        let data = self.data;
        let (bytes, known) = heap::with(|h| h.image.bytes(data, snap, crate::lb::snap::SNAP_SIZE))
            .expect("the heap's image");
        // The last block may start up to a block's data before the file's end.
        let need = (size as usize + 0x2000).min(bytes.len());
        if let Some(i) = known[..need].iter().position(|k| !k) {
            crate::game::code::unported_call(&format!(
                "the snapshot's buffer's byte {i:#x} past its image, which the heap's image doesn't know"
            ));
        }
        let (banner_at, icon_at) = self.snap_icons();
        let banner = self.game.snap_write.banner.clone().unwrap_or_else(|| {
            (0..BANNER as u32)
                .map(|i| self.data.u8(banner_at.at(i)))
                .collect()
        });
        let icons: Vec<u8> = (0..ICONS as u32)
            .map(|i| self.data.u8(icon_at.at(i)))
            .collect();
        let comment = self
            .game
            .snap_write
            .filename
            .expect("the snapshot's comment");
        let request = SnapRequest {
            name: name.into_bytes(),
            size,
            data: bytes[..need].to_vec(),
            comment: comment.to_vec(),
            banner,
            icons,
        };
        let now = self.game.audio.clock.as_ref().map_or(0, |c| c.now);
        let seconds = self.session_seconds();
        let (list, stamps) =
            super::save::start_list_dry(request.run(), &request.tasks(), now, seconds);
        let done = !list.card_in;
        let r = list.result;
        for &w in &list.start {
            self.game.audio.clock_card_work(self.data, w);
        }
        assert!(
            done || !list.steps.is_empty(),
            "a snapshot's save that ends in its first call"
        );
        if done {
            // Nothing written: the list failed at its mount.
            return r;
        }
        self.game.snap_write.save = Some(SnapSave {
            list,
            next: 0,
            request,
            stamps,
            times: Vec::new(),
            seconds,
        });
        result::BUSY
    }

    /// The snapshot's save's next step (`lbCardNew_CompleteNextTask`): its wait point and
    /// transfers; busy until its last, which returns the list's result.
    pub fn snap_save_step(&mut self) -> i32 {
        let Some(save) = self.game.snap_write.save.as_mut() else {
            crate::game::code::unported_call("lbCardNew_CompleteNextTask with no list")
        };
        let work = save.list.steps[save.next];
        save.next += 1;
        let last = save.next == save.list.steps.len();
        let r = save.list.result;
        let results: Vec<(usize, i32)> = save
            .list
            .results
            .iter()
            .filter(|(at, _, _)| *at + 1 == save.next)
            .map(|&(_, f, e)| (f, e))
            .collect();
        for (f, e) in results {
            self.game.card_tasks.file_errors[f] = e;
        }
        self.game.audio.clock_card_work(self.data, work);
        // A step that stamps the directory reads the clock in its transfers' chain, which
        // its end is a fraction of a millisecond past.
        let k = self.game.snap_write.save.as_ref().map_or(0, |s| s.next - 1);
        if self
            .game
            .snap_write
            .save
            .as_ref()
            .is_some_and(|s| s.stamps.contains(&k))
        {
            let now = self.session_seconds();
            if let Some(s) = self.game.snap_write.save.as_mut() {
                s.times.push(now);
            }
        }
        if last {
            let save = self
                .game
                .snap_write
                .save
                .take()
                .expect("the snapshot's save");
            let written = super::save::finish_list(
                save.request.run(),
                &save.request.tasks(),
                save.seconds,
                &save.times,
            );
            assert_eq!(
                written, r,
                "the snapshot's save's list ended otherwise on the card"
            );
            return r;
        }
        result::BUSY
    }
}

/// `lbSnap_8001DC0C`'s comment: the title (Japanese when the console's language setting is)
/// and the date and time `seconds` after 2000-01-01, `"%s %02d/%02d %02d:%02d:%02d"`.
pub fn snapshot_comment(seconds: u32, japanese: bool) -> [u8; 0x40] {
    const EN: &[u8] = b"Super Smash Bros. Melee         Snapshot";
    // "大乱闘スマッシュブラザーズＤＸ  写真データ" in Shift-JIS.
    const JP: &[u8] = b"\x91\xe5\x97\x90\x93\xac\x83X\x83}\x83b\x83V\x83\x85\x83u\x83\x89\x83U\x81[\x83Y\x82c\x82w  \x8e\xca\x90^\x83f\x81[\x83^";
    let (_, m, d) = melee_save::melee::civil(seconds);
    let t = seconds % 86_400;
    let (hh, mm, ss) = (t / 3600, t / 60 % 60, t % 60);
    let text = [
        if japanese { JP } else { EN },
        format!(" {m:02}/{d:02} {hh:02}:{mm:02}:{ss:02}").as_bytes(),
    ]
    .concat();
    let mut out = [0u8; 0x40];
    let n = text.len().min(0x3F);
    out[..n].copy_from_slice(&text[..n]);
    out
}

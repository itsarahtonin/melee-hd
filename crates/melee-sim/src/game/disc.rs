//! The game's disc loads as a session from power-on runs them: the preload cache's class for
//! each scene and its waits (`lbDvd`), files taken from the cache (`lbDvd_8001819C`,
//! `lbArchive_80017040`), and archives loaded into the scene's heap (`lbArchive_80016DBC`),
//! each wait and read on the audio timeline.
//!
//! The files' bytes are the frontend's: it reads every file a session may load before the
//! session starts (`session::FILES`), and the state names each by its disc entry.

use super::ft::data::MAIN_MEMORY;
use super::state::Running;
use crate::audio::clock::Wait;
use crate::lb::dvd::Dvd;

impl Running<'_> {
    /// The preload cache, which a session from power-on keeps.
    pub fn dvd_mut(&mut self) -> &mut Dvd {
        self.game.audio.dvd.as_mut().expect("the preload cache")
    }

    /// The preloads' reads into main memory, for the heap's image (`Dvd::image_reads`): a
    /// later scene's heap may lie over the preload heaps (a mode state that preloads nothing,
    /// the challenger approach's, makes its heap over them), and its blocks then take what the
    /// files left (docs/engine.md, a session's heap leftovers). A file read holds its bytes as
    /// the disc has them until its parse relocates them ([`Self::take_cached_blocks`]'s
    /// callers, [`crate::hsd::image::Image::place_held`]); its `HSD_Archive` block what its
    /// parse wrote, unknown. Taken up before each take and each scene's heaps, so a read
    /// always lands before the parse of what it read.
    pub fn image_preload_reads(&mut self) {
        let Some(d) = self.game.audio.dvd.as_mut() else {
            return;
        };
        if d.image_reads.is_empty() {
            return;
        }
        let reads = std::mem::take(&mut d.image_reads);
        if !crate::hsd::heap::image_on() {
            return;
        }
        for (at, len, entry) in reads {
            let f = &self.game.ft.data.files;
            let id = f
                .entries
                .iter()
                .find(|e| e.1 == entry && entry >= 0)
                .and_then(|e| f.archive_id(&e.0));
            crate::hsd::heap::with(|h| match id {
                Some(archive) => h.image.write(
                    at,
                    at + len,
                    crate::hsd::image::Src::File {
                        archive,
                        offset: 0,
                        base: 0,
                    },
                ),
                None => h.image.preloaded(
                    at,
                    at + len,
                    if entry < 0 {
                        "HSD_Archive"
                    } else {
                        "a preloaded file"
                    },
                ),
            });
        }
    }

    /// `lbDvd_80018CF4` for a scene: the preload class's heaps, waiting (`lb_800195D0`) for
    /// a read into a heap the class lets go, and the scene's heaps made: the main heap's
    /// allocator model starts over (`HSD_CreateMainHeap`).
    #[doc(alias = "lbDvd_80018CF4")]
    pub fn set_preload_class(&mut self, class: i32) {
        self.dvd_mut().set_class(class);
        for h in [
            crate::lb::heap::SEQ,
            crate::lb::heap::STAY,
            crate::lb::heap::ALL_M,
            crate::lb::heap::ALL_A,
        ] {
            if self.dvd_mut().heaps.transient(h) == 1 {
                self.game.audio.with_clock(self.data, |c, f| {
                    c.wait_until(f, Wait::HeapReleased(h as i32))
                });
            }
        }
        self.image_preload_reads();
        let layout = self.dvd_mut().make_scene_heaps();
        // The boot's first scene finds the heap as the session's start left it, empty.
        let carried = crate::hsd::heap::with(|h| h.carried())
            .filter(|c| !c.classes.is_empty())
            .unwrap_or_else(crate::hsd::heap::Carried::power_on);
        // What the scene's heap held, for the next scenes' leftovers.
        let (game, data) = (&*self.game, self.data);
        // The collision joints are this scene's when its heap holds their array's block.
        let joints = game.geometry.as_ref().map_or(0, |g| g.joints_addr);
        let geometry_live = joints != 0
            && crate::hsd::heap::with(|h| {
                h.os.allocated
                    .iter()
                    .any(|&(c, _)| c + crate::hsd::heap::HEADER == joints)
            })
            .unwrap_or(false);
        let links = game.ft.dynamics_block;
        let dynamics_live = links != 0
            && crate::hsd::heap::with(|h| {
                h.os.allocated
                    .iter()
                    .any(|&(c, _)| c + crate::hsd::heap::HEADER == links)
            })
            .unwrap_or(false);
        crate::hsd::export::with_image(|img| {
            super::export::scene_game_objects(game, data, img, geometry_live, dynamics_live)
        });
        crate::hsd::heap::with(|h| h.end_scene_image());
        self.export_heap_image();
        crate::hsd::heap::start_scene_lent(&layout);
        crate::hsd::heap::with(|h| h.carry(&carried));
        // A session's scenes take the heap the boot and the scenes before used: its blocks
        // hold their bytes (the fighters' animation buffers' unknown fill).
        crate::hsd::heap::with(|h| h.fresh = false);
        // `HSD_CreateMainHeap`'s forgetting of the old heap: the classes' amnesia
        // (`CObjAmnesia`, `JObjAmnesia`, `LObjAmnesia`, `MObjAmnesia`) and the ID table's
        // (`_HSD_IDForgetMemory`).
        let o = &mut self.game.objs;
        o.cobjs.current = None;
        o.cobjs.freed_current = 0;
        o.jobj.current = None;
        o.lobjs.current = None;
        // The lights the last scene made are gone with its heap; the active table keeps
        // naming them, by the addresses they had (the table is cleared before anything but
        // the ambient slot's test reads it).
        for k in 0..crate::hsd::lobj::MAX_GXLIGHT {
            if let Some(l) = o.lobjs.active[k].take() {
                o.lobjs.active_left[k] = o.lobjs.pool.addr(l.0);
            }
        }
        o.lobjs.pool = Default::default();
        // `PObjAmnesia`: the shape animations' blend buffers.
        o.pobjs.vertex_buffer = 0;
        o.pobjs.normal_buffer = 0;
        o.lobjs.nodes = Default::default();
        o.mobjs.toon = None;
        o.mobjs.shadows = None;
        o.ids = Default::default();
        // `_HSD_RObjForgetMemory`: the expressions' argument buffer, from the old heap.
        o.robjs.arg_buf = false;
    }

    /// The objects of the scene's heap as the game lays them out, into the heap's image as the
    /// heap is made anew ([`crate::hsd::image`]).
    fn export_heap_image(&mut self) {
        let (game, data) = (&*self.game, self.data);
        crate::hsd::export::with_image(|img| super::export::scene_end(game, data, img));
        crate::hsd::heap::with(|h| h.write_free_links());
        // The flash's user data goes with its scene's heap: a later scene's end mustn't lay it
        // out again over what took its block since (a preloaded file, where a scene that
        // preloads nothing made its heap over the preload heaps).
        self.game.bgflash.user = None;
    }

    /// `lbDvd_80017700`: waits until the heap's unwanted entries that are set up are let go.
    #[doc(alias = "lbDvd_80017700")]
    pub fn wait_preload_stale_released(&mut self, heap: i32) {
        self.game
            .audio
            .with_clock(self.data, |c, f| c.wait_until(f, Wait::StaleReleased(heap)));
    }

    /// `lbDvd_GetPreloadedArchive` through `lbDvd_8001819C`: the preloaded file of disc entry
    /// `entry_num`, waiting for its read if it's coming, and set up as its kind is (an
    /// archive parsed) the first time it's taken. Whether the cache had it.
    #[doc(alias = "lbDvd_8001819C")]
    #[doc(alias = "lbDvd_GetPreloadedArchive")]
    #[doc(alias = "lbDvd_800189EC")]
    pub fn take_preloaded(&mut self, entry_num: i32) -> bool {
        self.take_preloaded_entry(entry_num, true).is_some()
    }

    /// [`Running::take_preloaded`], with where the file is: its `HSD_Archive`'s block (the
    /// archive's kinds) or its bytes' (a raw file), as the cache's heaps put them.
    pub fn take_cached(&mut self, entry_num: i32) -> Option<u32> {
        self.take_cached_blocks(entry_num).map(|(a, _)| a)
    }

    /// [`Running::take_cached`], with the address of the file's bytes too (0 for none).
    pub fn take_cached_blocks(&mut self, entry_num: i32) -> Option<(u32, u32)> {
        self.take_cached_blocks_with(entry_num, true)
    }

    /// [`Running::take_cached_blocks`] without `lbDvd_8001819C`'s check.
    pub fn take_cached_blocks_unchecked(&mut self, entry_num: i32) -> Option<(u32, u32)> {
        self.take_cached_blocks_with(entry_num, false)
    }

    /// [`Running::take_cached_blocks`], with `lbDvd_8001819C`'s check that a mode expecting
    /// its files preloaded finds each, or without it (a load the original makes with
    /// `loadArchive`, which never asks the cache).
    fn take_cached_blocks_with(&mut self, entry_num: i32, check: bool) -> Option<(u32, u32)> {
        self.image_preload_reads();
        let i = self.take_preloaded_entry(entry_num, check)?;
        let d = self.dvd_mut();
        let e = d.cache.entries[i];
        let block = e.archive.or(e.raw_data).expect("a preloaded file's blocks");
        let raw = e
            .raw_data
            .map_or(0, |b| d.memory.entries[usize::from(b)].addr);
        Some((d.memory.entries[usize::from(block)].addr, raw))
    }

    /// A file the match's setup takes from the preload cache where it starts from the menus
    /// (`lbArchive_80017040`'s `lbDvd_8001819C`), nothing otherwise: its archive is the
    /// frontend's either way.
    ///
    /// Whether the file was set up before (an earlier scene took it, so its bytes hold what
    /// code wrote into them since, as the items' table of character articles does).
    pub fn take_cached_named(&mut self, name: &str) -> bool {
        let mut kept = false;
        if self.game.ft.data.files.from_cache {
            let (entrynum, len) = self.game.ft.data.files.entry(name);
            kept = self.cached_set_up(entrynum);
            // Where its data is, for a pointer into it a block of the scene's heap keeps once
            // freed (docs/engine.md, a session's heap leftovers).
            if let Some((_, file)) = self.take_cached_blocks(entrynum)
                && file != 0
                && let Some(id) = self.game.ft.data.files.archive_id(name)
            {
                crate::hsd::heap::with(|h| h.image.place_held(file, len, id));
            }
        }
        kept
    }

    /// Whether the preload cache holds disc entry `entry_num` set up already (taken by an
    /// earlier scene since its read).
    fn cached_set_up(&mut self, entry_num: i32) -> bool {
        self.dvd_mut().cache.entries.iter().any(|e| {
            e.state != 0
                && e.load_score > 0
                && i32::from(e.entry_num) == entry_num
                && e.load_state == 2
        })
    }

    /// A stage's buffer the preload cache may hold (its preload entry `entry`): taken from the
    /// cache as it stands in a VS match from the menus (its block's address), else by the
    /// match's list (0, the frontend's place); none when the scene's heap gives it.
    pub fn preloaded_buffer(&mut self, entry: i32) -> Option<u32> {
        if self.game.ft.data.files.from_cache {
            self.take_cached(entry)
        } else {
            self.game.ft.data.files.buffer_preloaded(entry).then_some(0)
        }
    }

    fn take_preloaded_entry(&mut self, entry_num: i32, check: bool) -> Option<usize> {
        let level = self.game.gm.db_level;
        let d = self.dvd_mut();
        let Some(i) =
            d.cache.entries.iter().position(|e| {
                e.state != 0 && e.load_score > 0 && i32::from(e.entry_num) == entry_num
            })
        else {
            // `lbDvd_8001819C`'s check, above retail's level, that a mode expecting its files
            // preloaded finds each (its other caller, `lbDvd_GetPreloadedArchive` alone, is
            // only ever asked for a file the cache holds).
            assert!(
                !(check && level != 0 && d.cache.preloaded),
                "the original's assertion failed: lbdvd.c:948 (entry {entry_num} is not PRELOADed)"
            );
            return None;
        };
        if d.cache.entries[i].state == 1 {
            d.cache.entries[i].load_score = 8980;
        }
        self.game.audio.with_clock(self.data, |c, f| {
            c.wait_until(f, Wait::PreloadFile(entry_num))
        });
        let e = &mut self.dvd_mut().cache.entries[i];
        if e.load_state == 1 {
            // `lbArchive_InitializeDAT`, `efAsync_OnLoad`, `grDatFiles_801C5FC0`: the
            // archive's parse touches only the file's own bytes and its `HSD_Archive`, which
            // melee-sim keeps as loaded data.
            assert!(
                matches!(e.kind, 2..=4),
                "the original's assertion failed: lbdvd.c:864"
            );
            e.load_state = 2;
        }
        Some(i)
    }

    /// `lb_80014534`, each scene's setup: the rumble scripts (`LbRb.dat`'s `lbRumbleData`),
    /// from the preload cache or the disc (`lbArchive_80017040`).
    #[doc(alias = "lb_80014534")]
    #[doc(alias = "lbArchive_80017040")]
    pub fn load_rumble_data(&mut self) {
        let name = "LbRb.dat";
        let (entrynum, _) = self.game.ft.data.files.entry(name);
        if !self.take_preloaded(entrynum) {
            self.disc_load(name, MAIN_MEMORY);
        }
        let id = self.game.ft.data.files.archive(name);
        let offset = self
            .data
            .get(id)
            .public("lbRumbleData")
            .expect("LbRb.dat's lbRumbleData");
        self.game.rumble_data = Some(melee_data::archive::DataRef {
            archive: id,
            offset,
        });
    }

    /// `lbArchive_80016DBC`: an archive loaded from the disc into the scene's heap; the
    /// archive.
    #[doc(alias = "lbArchive_80016DBC")]
    #[doc(alias = "lbArchive_LoadArchive")]
    pub fn load_archive(&mut self, name: &str) -> melee_data::archive::ArchiveId {
        self.disc_load(name, MAIN_MEMORY);
        self.game.ft.data.files.archive(name)
    }

    /// [`Running::load_archive`], with where its two blocks went in the scene's heap (the
    /// file's bytes, then its `HSD_Archive`), for its owner to free.
    pub fn load_archive_blocks(
        &mut self,
        name: &str,
    ) -> (melee_data::archive::ArchiveId, u32, u32) {
        let (entrynum, len) = self.game.ft.data.files.entry(name);
        let (file, archive) = crate::hsd::heap::with(|h| h.archive_alloc(len)).unwrap_or((0, 0));
        if file != 0
            && let Some(id) = self.game.ft.data.files.archive_id(name)
        {
            crate::hsd::heap::with(|h| h.hold_archive(self.data, file, len, archive, id));
        }
        if self.game.audio.clock.is_some() {
            self.game.audio.load_file(self.data, entrynum, file, len);
        }
        (self.game.ft.data.files.archive(name), file, archive)
    }

    /// `lbArchive_80016EFC`: an archive's two blocks back to the scene's heap
    /// (`lbHeap_80015CA8`), its bytes first.
    #[doc(alias = "lbArchive_80016EFC")]
    pub fn free_archive_blocks(&mut self, file: u32, archive: u32) {
        crate::hsd::heap::with(|h| {
            h.mem_free(file);
            h.mem_free(archive);
        });
    }
}

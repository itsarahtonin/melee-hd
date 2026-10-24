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
        let layout = self.dvd_mut().make_scene_heaps();
        // The boot's first scene finds the heap as the session's start left it, empty.
        let carried = crate::hsd::heap::with(|h| h.carried())
            .filter(|c| !c.classes.is_empty())
            .unwrap_or_else(crate::hsd::heap::Carried::power_on);
        crate::hsd::heap::start_scene_lent(&layout);
        crate::hsd::heap::with(|h| h.carry(&carried));
        // `HSD_CreateMainHeap`'s forgetting of the old heap: the classes' amnesia
        // (`CObjAmnesia`, `JObjAmnesia`, `LObjAmnesia`, `MObjAmnesia`) and the ID table's
        // (`_HSD_IDForgetMemory`).
        let o = &mut self.game.objs;
        o.cobjs.current = None;
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
        o.lobjs.nodes = Default::default();
        o.mobjs.toon = None;
        o.mobjs.shadows = None;
        o.ids = Default::default();
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
        let d = self.dvd_mut();
        let Some(i) =
            d.cache.entries.iter().position(|e| {
                e.state != 0 && e.load_score > 0 && i32::from(e.entry_num) == entry_num
            })
        else {
            return false;
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
        true
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

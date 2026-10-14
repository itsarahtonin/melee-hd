//! The audio library's sound objects in a captured game (`lbAudioAx_UserData`, the user data
//! of the GObjs of class 23): imported and compared field by field.

use melee_sim::game::handles::SoundObjId;
use melee_sim::game::mirror::{Mem, Mirror};
use melee_sim::lb::sound_obj::SoundObj;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::Importer;

fn mem(bytes: &[u8]) -> Mem<'_> {
    Mem { bytes, base: MEM1 }
}

impl Importer<'_> {
    /// A sound object's user data.
    pub fn sound_obj(&mut self, a: u32) -> SoundObjId {
        if let Some(&s) = self.maps.sound_objs.get(&a) {
            return s;
        }
        let id = SoundObjId(self.game.sound_objs.alloc(SoundObj::default()));
        self.maps.sound_objs.insert(a, id);
        let cap = self.cap;
        let o = SoundObj::import(&mem(&cap.call.mem1), a, self);
        self.game.sound_objs[id.0] = o;
        id
    }
}

impl Comparer<'_> {
    /// A sound object GObj's user data, the first time it is met.
    pub(crate) fn sound_obj(&mut self, path: &str, o: SoundObjId, t: u32) {
        if !Self::pair(
            &mut self.maps.sound_objs,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "sound object",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let m = mem(self.mem);
        let game = self.game;
        game.sound_objs[o.0].compare(&m, t, &format!("{path} sound"), self);
    }
}

//! The display pass's HSD statics in a captured state: the current camera, the current and
//! active lights with their GX slot masks, and the current joint, display object and
//! material, imported into melee-sim and compared back.

use melee_sim::hsd::list::{self, SListId};
use melee_sim::hsd::lobj::MAX_GXLIGHT;

use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// The light statics' symbols and what each holds.
const MASKS: [&str; 4] = [
    "lightmask_diffuse",
    "lightmask_specular",
    "lightmask_attnfunc",
    "lightmask_alpha",
];

impl Importer<'_> {
    /// The display statics: the current camera (`HSD_CObjGetCurrent`), the current lights
    /// (`current_lights`) and the active ones by GX slot, and the current joint, display object
    /// and material, which are null between draws.
    pub fn display(&mut self) {
        let c = self.cap.u32(sym("current"));
        self.game.objs.cobjs.current = (c != 0).then(|| self.cobj(c));
        let mut at = self.cap.u32(sym("current_lights"));
        let mut prev: Option<SListId> = None;
        while at != 0 {
            let l = self.cap.u32(at + 4);
            let l = self.lobj(l);
            let n = list::alloc(&mut self.game.objs.lobjs.nodes, l);
            match prev {
                Some(p) => self.game.objs.lobjs.nodes[p.0].next = Some(n),
                None => self.game.objs.lobjs.current = Some(n),
            }
            prev = Some(n);
            at = self.cap.u32(at);
        }
        let active = sym("active_lights");
        for i in 0..MAX_GXLIGHT {
            let l = self.cap.u32(active + 4 * i as u32);
            self.game.objs.lobjs.active[i] = (l != 0).then(|| self.lobj(l));
        }
        self.game.objs.lobjs.nb_active = self.cap.u32(sym("nb_active_lights")) as i32;
        let [d, s, a, al] = MASKS.map(|m| self.cap.u32(sym(m)));
        let l = &mut self.game.objs.lobjs;
        (
            l.lightmask_diffuse,
            l.lightmask_specular,
            l.lightmask_attnfunc,
            l.lightmask_alpha,
        ) = (d, s, a, al);
        let j = self.cap.u32(sym("current_jobj"));
        self.game.objs.jobj.current = (j != 0).then(|| self.jobj(j));
        for name in ["current_dobj", "current_mobj"] {
            if self.cap.u32(sym(name)) != 0 {
                self.unhandled.push(format!("{name} is set"));
            }
        }
    }
}

impl Comparer<'_> {
    /// [`Importer::display`]'s statics.
    pub fn display(&mut self) {
        let objs = &self.game.objs;
        let t = self.u32(sym("current"));
        match objs.cobjs.current {
            Some(c) if t != 0 => self.cobj("display current camera", c, t),
            ours => self.check("display", "current camera", ours.is_some(), t != 0),
        }
        let mut node = objs.lobjs.current;
        let mut at = self.u32(sym("current_lights"));
        let mut i = 0;
        while node.is_some() || at != 0 {
            let (Some(n), true) = (node, at != 0) else {
                self.check(
                    "display",
                    "current lights",
                    format!("{i} and more: {}", node.is_some()),
                    format!("{i} and more: {}", at != 0),
                );
                break;
            };
            let l = self.game.objs.lobjs.nodes[n.0];
            let t = self.u32(at + 4);
            self.lobjs(&format!("display current light {i}"), Some(l.data), t);
            node = l.next;
            at = self.u32(at);
            i += 1;
        }
        let active = sym("active_lights");
        for k in 0..MAX_GXLIGHT {
            let t = self.u32(active + 4 * k as u32);
            let o = self.game.objs.lobjs.active[k];
            let path = format!("display active light {k}");
            match o {
                Some(l) if t != 0 => self.lobjs(&path, Some(l), t),
                _ => self.check(&path, "set", o.is_some(), t != 0),
            }
        }
        let l = &self.game.objs.lobjs;
        let ours = (
            l.nb_active as u32,
            l.lightmask_diffuse,
            l.lightmask_specular,
            l.lightmask_attnfunc,
            l.lightmask_alpha,
        );
        let [d, s, a, al] = MASKS.map(|m| self.u32(sym(m)));
        let theirs = (self.u32(sym("nb_active_lights")), d, s, a, al);
        self.check("display", "active lights and masks", ours, theirs);
        let current = (
            self.game.objs.jobj.current.is_some(),
            self.game.objs.dobjs.current.is_some(),
            self.game.objs.mobjs.current.is_some(),
        );
        let theirs = (
            self.u32(sym("current_jobj")) != 0,
            self.u32(sym("current_dobj")) != 0,
            self.u32(sym("current_mobj")) != 0,
        );
        self.check("display", "current joint, dobj, mobj", current, theirs);
    }
}

//! The HUD's and the debug text's globals (`melee/if`): imported from a capture and compared
//! back, pairing the GObjs, joints and cameras they point to as the scheduler's do.

use melee_sim::game::hud::ModelDesc;
use melee_sim::game::hud::devtext::{DevText, DevTexts, Glyph, POOL};
use melee_sim::game::hud::nametag::NameTags;
use melee_sim::game::hud::small::{Coins, Hazard};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::UserData;
use melee_sim::game::structs::{
    Element_803F9628, HudIndex, IfStockUserData, ScInfStcModels, StaticModelDesc, ifAll_804A0FD8_t,
    ifStock_804A1ACC, ifStock_804A1378, ifStock_804A1774, ifTime_data, un_803F9E08_t,
    un_804A1F10_t, un_804A1F58_t,
};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::sis::TextId;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// The HUD units' statics' addresses.
const IFALL: u32 = 0x804A_0FD8;
const HIDDEN_FLAG: u32 = 0x804D_6D58;
const ARCHIVE: u32 = 0x804D_6D5C;
const STATUS: u32 = 0x804A_10C8;
const STATUS_COUNT: u32 = 0x804D_6D60;
const ANNOUNCEMENTS: u32 = 0x803F_9628;
const KO_MARKS: u32 = 0x804A_1340;
const TIME: u32 = 0x804A_1078;
const TIMER_MODELS: u32 = 0x804A_10B4;
const STOCK: u32 = 0x804A_1378;
const STOCK_USERS: u32 = 0x804A_157C;
const STOCK_1774: u32 = 0x804A_1774;
const STOCK_1A8C: u32 = 0x804A_1A8C;
const STOCK_1ACC: u32 = 0x804A_1ACC;
const NAMETAG_MODEL: u32 = 0x804A_1ED0;
const NAMETAG_GOBJS: u32 = 0x804A_1EE0;
const NAMETAG_ENTRIES: u32 = 0x804A_1EF8;
const NAMETAG_CAMERA: u32 = 0x804D_6D68;
const NAMETAG_HIDDEN: u32 = 0x804D_6D6C;
const NAMETAG_SHOWN: u32 = 0x804D_6D70;
const NAMETAG_TEXT: u32 = 0x804D_6D78;
const NAMETAG_CANVAS: u32 = 0x804D_6D7C;
const HAZARD: u32 = 0x804D_6D80;
const COINS: u32 = 0x803F_9E08;
const COINS_ARCHIVE: u32 = 0x804D_6DA0;
const COINS_SCENE: u32 = 0x804D_6DA4;
const ARROWS: u32 = 0x804A_1F10;
const ARROWS_HIDDEN: u32 = 0x804D_6D90;
const ARROW_SLOTS: u32 = 0x804D_DBA8;
const SCORES: u32 = 0x804A_1F58;
/// `mn_8022EB04`, a player tag's user data release.
const NAMETAG_RELEASE: u32 = 0x8022_EB04;

/// `DevText`'s size.
const DEVTEXT_SIZE: u32 = 0x34;

/// A screen's pool index from its address.
fn devtext_index(pool: u32, a: u32) -> Option<u8> {
    if a == 0 {
        return None;
    }
    let off = a.wrapping_sub(pool);
    assert!(
        off < POOL as u32 * DEVTEXT_SIZE && off.is_multiple_of(DEVTEXT_SIZE),
        "DevText {a:#x} outside the pool"
    );
    Some((off / DEVTEXT_SIZE) as u8)
}

/// Reads memory the way both sides need.
pub(crate) trait Words {
    fn b(&self, a: u32) -> u8;
    fn h(&self, a: u32) -> u16 {
        u16::from(self.b(a)) << 8 | u16::from(self.b(a + 1))
    }
    fn w(&self, a: u32) -> u32 {
        u32::from(self.h(a)) << 16 | u32::from(self.h(a + 2))
    }
    fn color(&self, a: u32) -> [u8; 4] {
        [self.b(a), self.b(a + 1), self.b(a + 2), self.b(a + 3)]
    }

    /// A screen as memory holds it, its buffer read where it points.
    fn devtext(&self, pool: u32, i: u32) -> DevText {
        let a = pool + DEVTEXT_SIZE * i;
        let (w, h) = (self.b(a + 4), self.b(a + 5));
        let buf_at = self.w(a + 0x28);
        let buf = if buf_at == 0 {
            Vec::new()
        } else {
            (0..u32::from(w) * u32::from(h))
                .map(|k| Glyph {
                    chr: self.b(buf_at + 2 * k),
                    attr: self.b(buf_at + 2 * k + 1),
                })
                .collect()
        };
        let packed = self.b(a + 0x27);
        DevText {
            x: self.h(a) as i16,
            y: self.h(a + 2) as i16,
            w,
            h,
            cursor_x: self.b(a + 6),
            cursor_y: self.b(a + 7),
            scale_x: f32::from_bits(self.w(a + 8)),
            scale_y: f32::from_bits(self.w(a + 0xC)),
            bg_color: self.color(a + 0x10),
            text_colors: core::array::from_fn(|k| self.color(a + 0x14 + 4 * k as u32)),
            id: self.b(a + 0x24) as i8,
            line_width: self.b(a + 0x25),
            flags: self.b(a + 0x26),
            unk: packed >> 2,
            current_color: packed & 3,
            buf,
            prev: devtext_index(pool, self.w(a + 0x2C)),
            next: devtext_index(pool, self.w(a + 0x30)),
        }
    }
}

impl Words for [u8] {
    fn b(&self, a: u32) -> u8 {
        self[(a - crate::MEM1) as usize]
    }
}

/// A screen with its floats as bit patterns, for exact comparison.
fn devtext_key(d: &DevText) -> String {
    let mut plain = d.clone();
    plain.scale_x = 0.0;
    plain.scale_y = 0.0;
    format!(
        "{plain:?} scale {:08x} {:08x}",
        d.scale_x.to_bits(),
        d.scale_y.to_bits()
    )
}

impl Importer<'_> {
    /// The HUD's and the debug text's globals.
    pub fn hud(&mut self) {
        self.devtext();
        self.sis();
        self.hud_statics();
    }

    /// The HUD units' statics.
    fn hud_statics(&mut self) {
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let m: &[u8] = &cap.call.mem1;
        let all = ifAll_804A0FD8_t::import(&mem, IFALL, self);
        let status = HudIndex::import(&mem, STATUS, self);
        let announcements = core::array::from_fn(|i| {
            Element_803F9628::import(
                &mem,
                ANNOUNCEMENTS + i as u32 * Element_803F9628::SIZE,
                self,
            )
        });
        let ko_marks = ScInfStcModels::import(&mem, KO_MARKS, self);
        let time = ifTime_data::import(&mem, TIME, self);
        let tm = |s: &mut Self, o: u32| s.import_data(m.w(TIMER_MODELS + o));
        let timer_models = ModelDesc {
            joint: tm(self, 0),
            anims: tm(self, 4),
            matanims: tm(self, 8),
            shapeanims: tm(self, 0xC),
        };
        let archive = m.w(ARCHIVE);
        let archive = (archive != 0)
            .then(|| self.data.get(m.w(archive + 0x20)).map(|d| d.archive))
            .flatten();
        let stock = ifStock_804A1378::import(&mem, STOCK, self);
        let stock_users = core::array::from_fn(|i| {
            IfStockUserData::import(&mem, STOCK_USERS + i as u32 * IfStockUserData::SIZE, self)
        });
        let stock_1774 = ifStock_804A1774::import(&mem, STOCK_1774, self);
        let stock_1a8c = core::array::from_fn(|i| self.gobj_at(m.w(STOCK_1A8C + 4 * i as u32)));
        let stock_1acc = ifStock_804A1ACC::import(&mem, STOCK_1ACC, self);
        let nametags = NameTags {
            model: StaticModelDesc::import(&mem, NAMETAG_MODEL, self),
            gobjs: core::array::from_fn(|i| self.gobj_at(m.w(NAMETAG_GOBJS + 4 * i as u32))),
            entries: core::array::from_fn(|i| m.w(NAMETAG_ENTRIES + 4 * i as u32) as i32),
            camera: self.gobj_at(m.w(NAMETAG_CAMERA)),
            hidden: m.b(NAMETAG_HIDDEN),
            shown: core::array::from_fn(|i| m.b(NAMETAG_SHOWN + i as u32)),
            text: self.import_handle("TextId", m.w(NAMETAG_TEXT)).map(TextId),
            canvas: m.w(NAMETAG_CANVAS) as i32,
        };
        let hazard = Hazard {
            gobj: self.gobj_at(m.w(HAZARD)),
            timer: m.w(HAZARD + 4) as i32,
            shown: m.w(HAZARD + 8) as i32,
            hidden: m.w(HAZARD + 0xC) as i32,
        };
        let ca = m.w(COINS_ARCHIVE);
        let coins = Coins {
            state: un_803F9E08_t::import(&mem, COINS, self),
            archive: (ca != 0)
                .then(|| self.data.get(m.w(ca + 0x20)).map(|d| d.archive))
                .flatten(),
            scene: self.import_data(m.w(COINS_SCENE)),
        };
        let arrows = un_804A1F10_t::import(&mem, ARROWS, self);
        let scores = un_804A1F58_t::import(&mem, SCORES, self);
        let h = &mut self.game.hud;
        h.stock = stock;
        h.stock_users = stock_users;
        h.stock_1774 = stock_1774;
        h.stock_1a8c = stock_1a8c;
        h.stock_1acc = stock_1acc;
        h.nametags = nametags;
        h.hazard = hazard;
        h.coins = coins;
        h.arrows = arrows;
        h.arrows_hidden = m.w(ARROWS_HIDDEN) as i32;
        h.scores = scores;
        h.all = all;
        h.hidden = m.b(HIDDEN_FLAG) != 0;
        h.archive = archive;
        h.status = status;
        h.status_count = m.b(STATUS_COUNT);
        h.status_flip = m.b(STATUS_COUNT + 1) as i8;
        h.announcements = announcements;
        h.ko_marks = ko_marks;
        h.time = time;
        h.timer_models = timer_models;
    }

    /// A GObj a global points to.
    pub(crate) fn gobj_at(&mut self, a: u32) -> Option<GObjId> {
        self.import_handle("GObjId", a).map(GObjId)
    }

    fn devtext(&mut self) {
        let mem: &[u8] = &self.cap.call.mem1;
        let pool = sym("devtext_pool");
        let mut d = DevTexts {
            pool: (0..POOL as u32).map(|i| mem.devtext(pool, i)).collect(),
            pool_head: devtext_index(pool, mem.w(sym("devtext_poolhead"))),
            draw_list: devtext_index(pool, mem.w(sym("devtext_drawlist"))),
            classifier: mem.w(sym("devtext_setup_classifier")) as i32,
            p_link: mem.w(sym("devtext_setup_p_link")) as i32,
            priority: mem.w(sym("devtext_setup_priority")) as i32,
            gx_link: mem.w(sym("devtext_setup_gx_link")) as i32,
            render_priority: mem.w(sym("devtext_setup_render_priority")) as i32,
            ..DevTexts::default()
        };
        let menus = sym("un_804A2688");
        let (used, free, peak) = (mem.w(menus + 8), mem.w(menus + 0xC), mem.w(menus + 0x10));
        assert!(
            used == 0 && free == 0 && peak == 0,
            "debug menus in use: their port is missing"
        );
        d.menus = (mem.w(menus + 0x20) != 0).then(Default::default);
        let cobj = mem.w(sym("devtext_cobj"));
        let gobj = mem.w(sym("devtext_gobj"));
        d.cobj = (cobj != 0).then(|| self.cobj(cobj));
        d.gobj = self
            .maps
            .gobjs
            .get(&gobj)
            .copied()
            .or_else(|| (gobj != 0).then(|| panic!("devtext_gobj {gobj:#x} isn't linked")));
        self.game.hud.devtext = d;
    }
}

impl Comparer<'_> {
    /// The HUD's and the debug text's globals, as [`Importer::hud`] imports them.
    pub fn hud(&mut self) {
        self.devtext();
        self.sis();
        self.hud_statics();
    }

    fn hud_statics(&mut self) {
        let m = self.mem;
        let mem = Mem {
            bytes: m,
            base: MEM1,
        };
        let game = self.game;
        let h = &game.hud;
        h.all.compare(&mem, IFALL, "hud all", self);
        h.status.compare(&mem, STATUS, "hud status", self);
        for (i, e) in h.announcements.iter().enumerate() {
            let at = ANNOUNCEMENTS + i as u32 * Element_803F9628::SIZE;
            e.compare(&mem, at, &format!("hud announcement {i}"), self);
        }
        h.ko_marks.compare(&mem, KO_MARKS, "hud KO marks", self);
        h.time.compare(&mem, TIME, "hud time", self);
        for (o, ours) in [
            (0, h.timer_models.joint),
            (4, h.timer_models.anims),
            (8, h.timer_models.matanims),
            (0xC, h.timer_models.shapeanims),
        ] {
            self.compare_data("hud timer models", ours, m.w(TIMER_MODELS + o));
        }
        h.stock.compare(&mem, STOCK, "hud stock", self);
        for (i, u) in h.stock_users.iter().enumerate() {
            let at = STOCK_USERS + i as u32 * IfStockUserData::SIZE;
            u.compare(&mem, at, &format!("hud stock user {i}"), self);
        }
        h.stock_1774
            .compare(&mem, STOCK_1774, "hud stock 1774", self);
        for (i, g) in h.stock_1a8c.iter().enumerate() {
            self.compare_gobj(
                &format!("hud stock 1a8c[{i}]"),
                *g,
                m.w(STOCK_1A8C + 4 * i as u32),
            );
        }
        h.stock_1acc
            .compare(&mem, STOCK_1ACC, "hud stock 1acc", self);
        let n = &h.nametags;
        n.model
            .compare(&mem, NAMETAG_MODEL, "hud nametag model", self);
        for (i, g) in n.gobjs.iter().enumerate() {
            self.compare_gobj(
                &format!("hud nametag {i}"),
                *g,
                m.w(NAMETAG_GOBJS + 4 * i as u32),
            );
        }
        self.compare_gobj("hud nametag camera", n.camera, m.w(NAMETAG_CAMERA));
        self.compare_handle(
            "hud nametag text",
            "TextId",
            n.text.map(|t| t.0),
            m.w(NAMETAG_TEXT),
        );
        self.check(
            "hud nametags",
            "entries, hidden, shown, canvas",
            (n.entries, n.hidden, n.shown, n.canvas),
            (
                core::array::from_fn::<i32, 6, _>(|i| m.w(NAMETAG_ENTRIES + 4 * i as u32) as i32),
                m.b(NAMETAG_HIDDEN),
                core::array::from_fn::<u8, 6, _>(|i| m.b(NAMETAG_SHOWN + i as u32)),
                m.w(NAMETAG_CANVAS) as i32,
            ),
        );
        let z = &h.hazard;
        self.compare_gobj("hud hazard", z.gobj, m.w(HAZARD));
        self.check(
            "hud hazard",
            "timer, shown, hidden",
            (z.timer, z.shown, z.hidden),
            (
                m.w(HAZARD + 4) as i32,
                m.w(HAZARD + 8) as i32,
                m.w(HAZARD + 0xC) as i32,
            ),
        );
        h.coins.state.compare(&mem, COINS, "hud coins", self);
        let ca = m.w(COINS_ARCHIVE);
        let theirs = (ca != 0)
            .then(|| self.data.get(m.w(ca + 0x20)).map(|d| d.archive))
            .flatten();
        self.check("hud coins", "archive", h.coins.archive, theirs);
        self.compare_data("hud coins scene", h.coins.scene, m.w(COINS_SCENE));
        h.arrows.compare(&mem, ARROWS, "hud arrows", self);
        self.check(
            "hud arrows",
            "hidden",
            h.arrows_hidden,
            m.w(ARROWS_HIDDEN) as i32,
        );
        h.scores.compare(&mem, SCORES, "hud scores", self);
        let archive = m.w(ARCHIVE);
        let theirs = (archive != 0)
            .then(|| self.data.get(m.w(archive + 0x20)).map(|d| d.archive))
            .flatten();
        self.check(
            "hud all",
            "hidden, archive, display count and flip",
            (h.hidden, h.archive, h.status_count, h.status_flip),
            (
                m.b(HIDDEN_FLAG) != 0,
                theirs,
                m.b(STATUS_COUNT),
                m.b(STATUS_COUNT + 1) as i8,
            ),
        );
    }

    fn devtext(&mut self) {
        let mem = self.mem;
        let game = self.game;
        let d = &game.hud.devtext;
        let pool = sym("devtext_pool");
        for i in 0..POOL as u32 {
            let theirs = mem.devtext(pool, i);
            let ours = d.pool.get(i as usize).cloned().unwrap_or_default();
            self.check(
                &format!("hud devtext[{i}]"),
                "screen",
                devtext_key(&ours),
                devtext_key(&theirs),
            );
        }
        self.check(
            "hud devtext",
            "free and draw lists",
            (d.pool_head, d.draw_list),
            (
                devtext_index(pool, mem.w(sym("devtext_poolhead"))),
                devtext_index(pool, mem.w(sym("devtext_drawlist"))),
            ),
        );
        self.check(
            "hud devtext",
            "setup",
            (
                d.classifier,
                d.p_link,
                d.priority,
                d.gx_link,
                d.render_priority,
            ),
            (
                mem.w(sym("devtext_setup_classifier")) as i32,
                mem.w(sym("devtext_setup_p_link")) as i32,
                mem.w(sym("devtext_setup_priority")) as i32,
                mem.w(sym("devtext_setup_gx_link")) as i32,
                mem.w(sym("devtext_setup_render_priority")) as i32,
            ),
        );
        let menus = sym("un_804A2688");
        self.check(
            "hud devtext",
            "debug menus' pool (used, peak, size, align)",
            d.menus
                .as_ref()
                .map_or((0, 0, 0, 0), |m| (m.used(), m.peak(), 32, 3)),
            (
                mem.w(menus + 8),
                mem.w(menus + 0x10),
                mem.w(menus + 0x20),
                mem.w(menus + 0x24),
            ),
        );
        let gobj = mem.w(sym("devtext_gobj"));
        self.compare_gobj("hud devtext gobj", d.gobj, gobj);
        let cobj = mem.w(sym("devtext_cobj"));
        match d.cobj {
            Some(c) => self.cobj("hud devtext cobj", c, cobj),
            None => self.check("hud devtext cobj", "CObj", 0, cobj),
        }
    }

    /// A GObj pointer: paired with ours, and compared in full if the scheduler doesn't
    /// hold it (the scheduler's comparison does the rest).
    pub(crate) fn compare_gobj(&mut self, path: &str, ours: Option<GObjId>, theirs: u32) {
        self.compare_handle(path, "GObjId", ours.map(|g| g.0), theirs);
    }
}

/// The HUD's and text system's GObj user data, by the release function the GObj holds.
pub(crate) fn user_data(imp: &mut Importer, release: u32, ud: u32) -> Option<UserData> {
    let mem: &[u8] = &imp.cap.call.mem1;
    match release {
        // HSD_SisLib_803A5A2C: a text.
        0x803A_5A2C => {
            let base = mem.w(sym("HSD_SisLib_804D796C"));
            Some(UserData::Text(TextId(ud - base)))
        }
        // fn_803A60EC: a canvas's camera, its own GObj.
        0x803A_60EC => Some(UserData::SisCamera(imp.gobj_at(ud)?)),
        NAMETAG_RELEASE => Some(UserData::NameTagSlot(mem.b(ud))),
        _ => None,
    }
}

/// The HUD's user data set without a kind (the stock icons' and the arrows'), by where it
/// points.
pub(crate) fn kindless_user_data(ud: u32) -> Option<UserData> {
    let size = IfStockUserData::SIZE;
    if (STOCK_USERS..STOCK_USERS + 6 * size).contains(&ud)
        && (ud - STOCK_USERS).is_multiple_of(size)
    {
        return Some(UserData::StockIcons(((ud - STOCK_USERS) / size) as u8));
    }
    if (ARROW_SLOTS..ARROW_SLOTS + 4).contains(&ud) {
        return Some(UserData::ArrowSlot((ud - ARROW_SLOTS) as u8));
    }
    None
}

/// Compares a GObj's HUD or text user data with memory (the GObj at `t`).
pub(crate) fn compare_user_data(cmp: &mut Comparer, path: &str, ud: UserData, t: u32) {
    let mem = cmp.mem;
    let theirs = mem.w(t + 0x2C);
    let release = mem.w(t + 0x30);
    match ud {
        UserData::Text(x) => {
            let base = mem.w(sym("HSD_SisLib_804D796C"));
            cmp.check(
                path,
                "text user data",
                (Some(x.0), 0x803A_5A2C),
                (theirs.checked_sub(base), release),
            );
        }
        UserData::SisCamera(g) => {
            cmp.compare_gobj(&format!("{path} canvas camera"), Some(g), theirs);
            cmp.check(path, "canvas camera release", 0x803A_60EC, release);
        }
        UserData::NameTagSlot(slot) => {
            cmp.check(
                path,
                "tag slot and release",
                (slot, NAMETAG_RELEASE),
                (mem.b(theirs), release),
            );
        }
        UserData::StockIcons(_) | UserData::ArrowSlot(_) => {
            cmp.check(
                path,
                "kindless user data",
                Some(ud),
                kindless_user_data(theirs),
            );
        }
        other => cmp.diff(path, "user data", other, format!("{theirs:#x}")),
    }
}

//! The trophy collection (`tydisplay.c`'s scene, mode 0xD): every trophy the save holds on
//! one table, each on its stand, seen from above. The stick turns the camera, X, R and the
//! D-pad's down zoom in, Y, L and its up zoom out, A puts the camera back, B or Start leave
//! (L, R or Start on any port leave for the main menu). Held at the entry, L lays the
//! trophies in rows, R in rings without jitter, Y in a triangle; else in jittered rings.
//!
//! The debug view (Z held at the entry with `DbLevel` at the debug ROM's or above): one trophy
//! at a time with its place in a debug text and the gallery's model beside it, its place
//! moved and written into `TyDatai`'s display table.
//! The display table's lookups the single-player modes use (`tyDisplay_8031C354` to
//! `tyDisplay_8031C8B8`) come with them in milestone 6.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::Loaded;
use super::view::dead_zone;
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::fog;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;

/// `tyDisplay_Scene_OnEnter`, `tyDisplay_Scene_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8031_B460);
pub const ON_FRAME: CodeRef = CodeRef(0x8031_B9A4);
/// `_tyDisplay_8031A4EC`: the camera's process.
pub const PROC_CAMERA: u32 = 0x8031_A4EC;
/// `_tyDisplay_8031A94C`: the debug view's camera process.
pub const PROC_DEBUG_CAMERA: u32 = 0x8031_A94C;
/// `Toy_80306930`: the fog's render (`HSD_FogSet`).
#[doc(alias = "Toy_80306930")]
pub const RENDER_FOG: u32 = 0x8030_6930;

/// How many trophies the display's blocks hold (`_tyDisplay_804D6F10`'s 300, the grid's 301).
const JOBJS: usize = 300;
const GRID: usize = 301;

/// `_tyDisplay_803B8988`: each category's model in its archive (the question mark's, then
/// the classes' five each, the stand, the question mark again).
const JOINTS: [&str; 43] = [
    "ToyDspQues_Top_joint",
    "ToyDspMycharaCmA_Top_joint",
    "ToyDspMycharaCmB_Top_joint",
    "ToyDspMycharaCmC_Top_joint",
    "ToyDspMycharaCmD_Top_joint",
    "ToyDspMycharaCmE_Top_joint",
    "ToyDspMycharaR1A_Top_joint",
    "ToyDspMycharaR1B_Top_joint",
    "ToyDspMycharaR1C_Top_joint",
    "ToyDspMycharaR1D_Top_joint",
    "ToyDspMycharaR1E_Top_joint",
    "ToyDspMycharaR2A_Top_joint",
    "ToyDspMycharaR2B_Top_joint",
    "ToyDspMycharaR2C_Top_joint",
    "ToyDspMycharaR2D_Top_joint",
    "ToyDspMycharaR2E_Top_joint",
    "ToyDspMapA_Top_joint",
    "ToyDspMapB_Top_joint",
    "ToyDspMapC_Top_joint",
    "ToyDspMapD_Top_joint",
    "ToyDspMapE_Top_joint",
    "ToyDspFgseriesA_Top_joint",
    "ToyDspFgseriesB_Top_joint",
    "ToyDspFgseriesC_Top_joint",
    "ToyDspFgseriesD_Top_joint",
    "ToyDspFgseriesE_Top_joint",
    "ToyDspFgetcA_Top_joint",
    "ToyDspFgetcB_Top_joint",
    "ToyDspFgetcC_Top_joint",
    "ToyDspFgetcD_Top_joint",
    "ToyDspFgetcE_Top_joint",
    "ToyDspPokemonA_Top_joint",
    "ToyDspPokemonB_Top_joint",
    "ToyDspPokemonC_Top_joint",
    "ToyDspPokemonD_Top_joint",
    "ToyDspPokemonE_Top_joint",
    "ToyDspItemA_Top_joint",
    "ToyDspItemB_Top_joint",
    "ToyDspItemC_Top_joint",
    "ToyDspItemD_Top_joint",
    "ToyDspItemE_Top_joint",
    "ToyDspStand_Top_joint",
    "ToyDspQues_Top_joint",
];

/// `_tyDisplay_803B8A34`'s names: each category model's material animation, `_Top_matanim_joint`
/// after the joint's stem (none for the question marks and the stand).
fn matanim(cat: usize) -> String {
    if cat == 0 || cat >= 41 {
        return String::new();
    }
    let stem = JOINTS[cat].trim_end_matches("_joint");
    format!("{stem}_matanim_joint")
}

/// `_tyDisplay_803B8AE0`: each category's archive (`_tyDisplay_8031BBF4`).
#[doc(alias = "_tyDisplay_8031BBF4")]
const FILES: [&str; 43] = [
    "TyQuesD.dat",
    "TyMycCmA.dat",
    "TyMycCmB.dat",
    "TyMycCmC.dat",
    "TyMycCmD.dat",
    "TyMycCmE.dat",
    "TyMycR1A.dat",
    "TyMycR1B.dat",
    "TyMycR1C.dat",
    "TyMycR1D.dat",
    "TyMycR1E.dat",
    "TyMycR2A.dat",
    "TyMycR2B.dat",
    "TyMycR2C.dat",
    "TyMycR2D.dat",
    "TyMycR2E.dat",
    "TyMapA.dat",
    "TyMapB.dat",
    "TyMapC.dat",
    "TyMapD.dat",
    "TyMapE.dat",
    "TySeriA.dat",
    "TySeriB.dat",
    "TySeriC.dat",
    "TySeriD.dat",
    "TySeriE.dat",
    "TyEtcA.dat",
    "TyEtcB.dat",
    "TyEtcC.dat",
    "TyEtcD.dat",
    "TyEtcE.dat",
    "TyPokeA.dat",
    "TyPokeB.dat",
    "TyPokeC.dat",
    "TyPokeD.dat",
    "TyPokeE.dat",
    "TyItemA.dat",
    "TyItemB.dat",
    "TyItemC.dat",
    "TyItemD.dat",
    "TyItemE.dat",
    "TyStandD.dat",
    "TyQuesD.dat",
];

/// The disc files the collection loads.
pub const DISC_FILES: [&str; 43] = FILES;

/// The collection's heap blocks and `.sbss` (`tydisplay.c`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Collection {
    /// `_tyDisplay_804D6F10` (300 words): the trophies' root joints in the grid's order;
    /// the words past the trophies placed are never written.
    pub jobjs_block: u32,
    pub jobjs: Vec<Option<JObjId>>,
    /// `_tyDisplay_804D6F14` (`TyDspGrid`, 0x12E4 bytes): the layout (cleared by each layout,
    /// so 0 after one), the places' bounds, the trophies with their order's key, and the
    /// places.
    pub grid_block: u32,
    pub mode: i32,
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
    /// `x14`: each place's trophy and key (`Toy_803060BC(trophy, 7)`).
    pub sort: Vec<(i32, i32)>,
    /// `x97C`: each place's x and z.
    pub pos: Vec<[f32; 2]>,
    /// `_tyDisplay_804D6F18` (`TyDspConfig`, 0x80 bytes): the camera and its view.
    pub cfg_block: u32,
    pub camera: Option<GObjId>,
    /// `x08`: how many trophies.
    pub count: i32,
    /// `x0C`, `x10`: the camera's pitch and yaw (degrees); `x18`, `x1C` their bounds.
    pub pitch: f32,
    pub yaw: f32,
    pub pitch_max: f32,
    pub yaw_max: f32,
    /// `x20`, `x24`: the stick past its dead zone; `x30`, `x34` the C-stick's.
    pub stick: [f32; 2],
    pub cstick: [f32; 2],
    /// `x40`: the table's width and its margin; `x44` the field of view that frames it,
    /// `x48` the zoom's step, `x4C` and `x50` its bounds; `x54`, `x58` the view's sides.
    pub width: f32,
    pub fov: f32,
    pub zoom_step: f32,
    pub fov_min: f32,
    pub fov_max: f32,
    pub left: f32,
    pub right: f32,
    /// `x5C`: the camera's interest at rest; `x68` its eye.
    pub interest: Vec3,
    pub eye: Vec3,
    /// `x74`: frames before input; `x75`, `x76` the columns and rows.
    pub wait: i8,
    pub cols: u8,
    pub rows: u8,
    /// `x78`: the last trophy made; `x7C` the debug view's trophy.
    pub last: Option<GObjId>,
    pub x7c: i32,
    /// `_tyDisplay_804D6F1C` (`TyDspBgData`, 0x108 bytes): the table (`gobj4`, and its
    /// scaled joint), `TyMnDisp` and the categories' 43 archives.
    pub bg_block: u32,
    pub bg: Option<GObjId>,
    pub bg_jobj: Option<JObjId>,
    pub archive: Option<Loaded>,
    pub archives: Vec<Option<Loaded>>,
    pub x104: i16,
    /// `_tyDisplay_804D6F20`: the debug view (0 on a retail console); `_tyDisplay_804D6F28`
    /// its B's frames, which no entry resets.
    pub debug: i32,
    pub debug_b: i32,
    /// `_tyDisplay_804D6F24`: the debug view's text, its trophy's place.
    pub text: Option<u8>,
    /// `_tyDisplay_804D6F2C`: the debug view's trophy as the gallery shows it, beside the
    /// display model.
    pub model: Option<GObjId>,
    /// `_tyDisplay_804A2DD0` (`.bss`, kept from scene to scene): that trophy's model file
    /// (`x8`, `xC`), the trophy (`x10`) and its archive (`x14`).
    pub holder: Holder,
}

/// `_tyDisplay_804A2DD0` (a `ToyListEntry` without its links).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Holder {
    pub file: Option<DataRef>,
    pub trophy: i16,
    pub archive: Option<Loaded>,
}

/// `TyDspEntry`: a trophy's place in the display table (`tyDisplayModelTbl`).
#[derive(Clone, Copy, Debug)]
pub struct DspEntry {
    /// Where it is in `TyDatai`.
    pub at: DataRef,
    pub id: i32,
    pub category: u8,
    pub frame: u8,
    pub x: f32,
    pub z: f32,
}

/// The deterministic quicksort `_tyDisplay_8031830C` and `_tyDisplay_80318714` spell out
/// (each recursion's first level inlined): the middle element as the pivot, swapped to `lo`;
/// the elements `before` it moved behind it; then each side.
#[doc(alias = "_tyDisplay_8031830C")]
#[doc(alias = "_tyDisplay_80318714")]
fn quicksort<T: Copy>(a: &mut [T], lo: i32, hi: i32, before: &impl Fn(&T, &T) -> bool) {
    if lo >= hi {
        return;
    }
    let mid = (lo + hi) / 2;
    a.swap(lo as usize, mid as usize);
    let mut p = lo;
    for i in lo + 1..=hi {
        if before(&a[i as usize], &a[lo as usize]) {
            p += 1;
            a.swap(p as usize, i as usize);
        }
    }
    a.swap(lo as usize, p as usize);
    quicksort(a, lo, p - 1, before);
    quicksort(a, p + 1, hi, before);
}

/// `(f32) n`.
fn int_f(n: i32) -> f64 {
    fp::frsp(f64::from(n))
}

impl Running<'_> {
    fn coll(&mut self) -> &mut Collection {
        &mut self.game.toys.collection
    }

    /// `tyDisplay_8031B9DC(id)`: a trophy's display entry, from the English table first on an
    /// English setting, else the other's; the terminator (id -1) if neither has it.
    #[doc(alias = "tyDisplay_8031B9DC")]
    pub fn coll_entry(&self, id: i32) -> DspEntry {
        let find = |name: &str| {
            let mut at = self.datai_sym(name);
            loop {
                let v = self.data.u32(at) as i32;
                if v == -1 || v == id {
                    break (at, v == id);
                }
                at = at.at(0x10);
            }
        };
        let mut at = None;
        if super::super::gm::language_setting_is_us() {
            let (a, found) = find("tyDisplayModelUsTbl");
            if found {
                at = Some(a);
            }
        }
        let at = at.unwrap_or_else(|| find("tyDisplayModelTbl").0);
        DspEntry {
            at,
            id: self.data.u32(at) as i32,
            category: self.data.u8(at.at(4)),
            frame: self.data.u8(at.at(5)),
            x: f32::from_bits(self.datai_u32(at.at(8))),
            z: f32::from_bits(self.datai_u32(at.at(0xC))),
        }
    }

    /// `_tyDisplay_80318B1C(n)`: the grid's `n` trophies with their keys
    /// (`Toy_803060BC(trophy, 7)`), from a random start (`HSD_Randi(n - 1)`), wrapping: the
    /// trophies held, or (with more than there are) every one with an entry and then random
    /// ones.
    #[doc(alias = "_tyDisplay_80318B1C")]
    fn coll_fill(&mut self, n: i32) {
        let mut start = if n > 1 {
            self.game.objs.rng.below(n - 1)
        } else {
            0
        };
        let put = |r: &mut Self, start: &mut i32, id: i32| {
            let key = r.toy_param_int(id, 7);
            r.coll().sort[*start as usize] = (id, key);
            *start += 1;
            if *start >= n {
                *start = 0;
            }
        };
        if n > super::TROPHY_COUNT as i32 {
            let (mut placed, mut i) = (0, 0);
            while placed < n {
                if i >= super::TROPHY_COUNT as i32 {
                    let mut id = self.game.objs.rng.below(0x124);
                    while self.coll_entry(id).id == -1 {
                        id = self.game.objs.rng.below(0x124);
                    }
                    put(self, &mut start, id);
                    placed += 1;
                } else if self.coll_entry(i).id != -1 {
                    put(self, &mut start, i);
                    placed += 1;
                }
                i += 1;
            }
        } else {
            for i in 0..super::TROPHY_COUNT as i32 {
                if self.toy_flags()[i as usize] & 0xFF != 0 {
                    put(self, &mut start, i);
                }
            }
        }
    }

    /// The layouts' shared end: the trophies filled in (`_tyDisplay_80318B1C`), the first two
    /// thirds of them (and one) sorted by key, largest first.
    fn coll_fill_sorted(&mut self) {
        let n = self.game.toys.collection.count;
        self.coll_fill(n);
        if n > 1 {
            let n2 = (n / 3) * 2;
            if n2 > 0 {
                quicksort(&mut self.coll().sort, 0, n2, &|a, b| a.1 > b.1);
            }
        }
    }

    /// The places sorted by z, nearest first (`_tyDisplay_8031830C` on the places).
    fn coll_sort_places(&mut self) {
        let n = self.game.toys.collection.count;
        if n > 1 && n - 1 > 0 {
            quicksort(&mut self.coll().pos, 0, n - 1, &|a, b| {
                reg(a[1]) < reg(b[1])
            });
        }
    }

    /// The layouts' last step: each trophy made (`_tyDisplay_8031BC54`, the last kept in
    /// `x78`) and its root put at its place.
    fn coll_place(&mut self) {
        let mut j = 0usize;
        for k in 0..self.game.toys.collection.count as usize {
            let id = self.game.toys.collection.sort[k].0;
            let g = self.coll_trophy(id);
            self.coll().last = Some(g);
            let Some(Obj::JObj(root)) = self.game.world.gobj(g).obj else {
                continue;
            };
            self.coll().jobjs[j] = Some(root);
            let [x, z] = self.game.toys.collection.pos[j];
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_srt(jobjs, root, |o| o.translate.x = x);
            jobj::set_srt(jobjs, root, |o| o.translate.z = z);
            j += 1;
        }
    }

    /// The grid cleared (`memzero`, its layout too), its bounds to ±3.5.
    fn coll_clear_grid(&mut self) {
        let c = self.coll();
        c.mode = 0;
        c.sort = vec![(0, 0); GRID];
        c.pos = vec![[0.0, 0.0]; GRID];
        c.min_z = -3.5;
        c.min_x = -3.5;
        c.max_z = 3.5;
        c.max_x = 3.5;
    }

    /// The bounds widened to a place.
    fn coll_bound(&mut self, i: usize) {
        let c = self.coll();
        let [x, z] = c.pos[i];
        if reg(x) < reg(c.min_x) {
            c.min_x = x;
        }
        if reg(x) > reg(c.max_x) {
            c.max_x = x;
        }
        if reg(z) < reg(c.min_z) {
            c.min_z = z;
        }
        if reg(z) > reg(c.max_z) {
            c.max_z = z;
        }
    }

    /// `_tyDisplay_80318CB4(even)`: the trophies in rings about the first, 9 apart (11 and
    /// jittered by up to 2 each way unless `even`), six more to each ring; two in three
    /// pulled in by tenths along their direction until they come within 8 of one placed
    /// before (any for the first 36, the last two rings' after), or as far as a random count
    /// of steps allows. The places sorted by z, the trophies by key, and placed.
    #[doc(alias = "_tyDisplay_80318CB4")]
    fn coll_rings(&mut self, even: i32) {
        self.coll_clear_grid();
        let step = |even: i32| if even != 0 { reg(9.0) } else { reg(11.0) };
        let deg = reg(0.017_453_292);
        let (mut ring_count, mut ring_max, mut prev_ring) = (0, 6, 0);
        let mut angle = reg(0.0);
        let mut radius = step(even);
        let limit = fp::fctiwz(reg(8.0));
        let mut i = 0usize;
        while (i as i32) < self.game.toys.collection.count {
            if i == 0 {
                self.coll().pos[0] = [0.0, 0.0];
            } else {
                let rad = mem(fp::fmuls(deg, angle));
                let x = mem(fp::fmuls(radius, reg(math::cos(rad))));
                let z = mem(fp::fmuls(radius, reg(math::sin(rad))));
                self.coll().pos[i] = [x, z];
                if even == 0 {
                    let f = self.game.objs.rng.draw_f32();
                    let x = mem(fp::fmadds(reg(2.0), reg(f), reg(x)));
                    self.coll().pos[i][0] = x;
                    let f = self.game.objs.rng.draw_f32();
                    let z = mem(fp::fmadds(reg(2.0), reg(f), reg(z)));
                    self.coll().pos[i][1] = z;
                }
                if self.game.objs.rng.below(3) != 0 {
                    let [x, z] = self.game.toys.collection.pos[i];
                    let theta = math::atan2(z, x);
                    let sq = mem(fp::fadds(
                        fp::fmuls(reg(x), reg(x)),
                        fp::fmuls(reg(z), reg(z)),
                    ));
                    let mut mag = reg(math::sqrt(sq));
                    let start = if i < 0x24 {
                        0
                    } else {
                        i as i32 - (prev_ring + prev_ring - 6)
                    };
                    let mut near = false;
                    while !near {
                        let x = mem(fp::fmuls(mag, reg(math::cos(theta))));
                        self.coll().pos[i][0] = x;
                        let s = math::sin(theta);
                        let mut tries = fp::fctiwz(fp::fdivs(mag, reg(0.1)));
                        let z = mem(fp::fmuls(mag, reg(s)));
                        self.coll().pos[i][1] = z;
                        if self.game.objs.rng.below(2) != 0 {
                            let half = fp::fctiwz(fp::fmuls(mag, reg(0.5)));
                            if half > 1 {
                                tries -= self.game.objs.rng.below(half);
                            }
                        }
                        let mut k = i as i32 - 1;
                        while k >= start {
                            let [kx, kz] = self.game.toys.collection.pos[k as usize];
                            let dz = fp::fsubs(reg(z), reg(kz));
                            let dx = fp::fsubs(reg(x), reg(kx));
                            let d = math::sqrt(mem(fp::fmadds(dx, dx, fp::fmuls(dz, dz))));
                            assert!(
                                reg(d) <= reg(2_147_483_600.0) && reg(d) >= reg(-2_147_483_600.0),
                                "*** tyDisplay Atari Irregul!"
                            );
                            if fp::fctiwz(reg(d)) <= limit {
                                near = true;
                                break;
                            }
                            k -= 1;
                        }
                        if tries == 0 {
                            break;
                        }
                        if !near {
                            mag = fp::fsubs(mag, reg(0.1));
                        }
                    }
                }
                ring_count += 1;
                if ring_count >= ring_max {
                    radius = fp::fadds(radius, step(even));
                    prev_ring = ring_max;
                    ring_count = 0;
                    ring_max += 6;
                    angle = if even != 0 {
                        reg(0.0)
                    } else {
                        int_f(self.game.objs.rng.below(0x1E))
                    };
                } else {
                    angle = fp::fadds(angle, fp::fdivs(reg(360.0), int_f(ring_max)));
                }
            }
            self.coll_bound(i);
            i += 1;
        }
        self.coll_sort_places();
        self.coll_fill_sorted();
        self.coll_place();
    }

    /// `_tyDisplay_80319540(staggered)`: the trophies in rows of the grid's columns, 9
    /// apart (the odd rows moved 3.5 along with `staggered`), the remainder of a full grid
    /// in the first row; sorted by key and placed.
    #[doc(alias = "_tyDisplay_80319540")]
    fn coll_rows(&mut self, staggered: i32) {
        self.coll_clear_grid();
        let (mut row, mut col) = (0i32, 0i32);
        let c = &self.game.toys.collection;
        let cols = i32::from(c.cols as i8);
        let mut rem = if c.count <= 1 { 0 } else { c.count % cols };
        let mut i = 0usize;
        while (i as i32) < self.game.toys.collection.count {
            if i == 0 {
                self.coll().pos[0] = [0.0, 0.0];
            } else {
                let mut x = mem(fp::fmuls(reg(9.0), int_f(col)));
                if staggered != 0 && row % 2 != 0 {
                    x = mem(fp::fadds(reg(x), reg(3.5)));
                }
                let z = mem(fp::fmuls(reg(9.0), int_f(row)));
                self.coll().pos[i] = [x, z];
            }
            col += 1;
            if rem != 0 {
                rem -= 1;
                if rem == 0 {
                    col = 0;
                    row += 1;
                }
            } else if col >= cols {
                col = 0;
                row += 1;
            }
            self.coll_bound(i);
            i += 1;
        }
        self.coll_fill_sorted();
        self.coll_place();
    }

    /// `_tyDisplay_80319994(back)`: the trophies in a triangle, one more to each row, 9
    /// apart, each row 4.5 further left (the rows going back with `back`, the places then
    /// sorted by z); sorted by key and placed.
    #[doc(alias = "_tyDisplay_80319994")]
    fn coll_triangle(&mut self, back: i32) {
        let mut xoff = reg(0.0);
        self.coll_clear_grid();
        let (mut row, mut col, mut ring) = (0i32, 0i32, 1i32);
        let mut i = 0usize;
        while (i as i32) < self.game.toys.collection.count {
            if i == 0 {
                self.coll().pos[0] = [0.0, 0.0];
            } else {
                let x = mem(fp::fmadds(reg(9.0), int_f(col), xoff));
                let z = if back != 0 {
                    mem(fp::fmuls(reg(-9.0), int_f(row)))
                } else {
                    mem(fp::fmuls(reg(9.0), int_f(row)))
                };
                self.coll().pos[i] = [x, z];
            }
            col += 1;
            if col >= ring {
                xoff = fp::fsubs(xoff, reg(4.5));
                col = 0;
                row += 1;
                ring += 1;
            }
            self.coll_bound(i);
            i += 1;
        }
        if back != 0 {
            self.coll_sort_places();
        }
        self.coll_fill_sorted();
        self.coll_place();
    }

    /// `_tyDisplay_80319EF0`: the camera framing the table: its interest 10 behind the
    /// places' middle (the layout's x, cleared by the layout, is always the rings'), its eye
    /// 500 in front; the field of view that takes the width and a margin of 14 (3.2 at least),
    /// the zoom's bounds and step, the turn's bounds; the table under the middle, scaled by
    /// tenths from a thirtieth of the trophies to take the places.
    #[doc(alias = "_tyDisplay_80319EF0")]
    fn coll_frame(&mut self) {
        let data = self.data;
        let cam = self.game.toys.collection.camera.expect("the camera");
        let Some(Obj::CObj(cobj)) = self.game.world.gobj(cam).obj else {
            panic!("the collection's camera has no CObj");
        };
        let c = &self.game.toys.collection;
        let absf = |v: f64| if v < 0.0 { -v } else { v };
        let range = absf(fp::fsubs(reg(c.max_x), reg(c.min_x)));
        let mut interest = Vec3::new(mem(fp::fmadds(range, reg(0.5), reg(c.min_x))), 0.0, 0.0);
        if c.mode == 3 {
            interest.x = 0.0;
        }
        let zr = absf(fp::fsubs(reg(c.max_z), reg(c.min_z)));
        interest.z = mem(fp::fmadds(zr, reg(0.5), reg(c.min_z)));
        let middle = interest;
        interest.z = mem(fp::fsubs(reg(interest.z), reg(10.0)));
        self.coll().interest = interest;
        let o = &mut self.game.objs;
        let mut eye = cobj::eye_position(o, data, cobj).expect("the camera's eye");
        eye.x = middle.x;
        eye.z = mem(fp::fadds(reg(500.0), reg(middle.z)));
        self.coll().eye = eye;
        let o = &mut self.game.objs;
        cobj::set_interest(o, cobj, interest);
        cobj::set_eye_position(o, cobj, eye);
        let c = &mut self.game.toys.collection;
        let xr = absf(fp::fsubs(reg(c.max_x), reg(c.min_x)));
        c.width = mem(fp::fadds(reg(14.0), xr));
        c.fov = 1.0;
        loop {
            let half = fp::fmuls(reg(c.fov), reg(0.5));
            let t = math::tan(mem(fp::fmuls(reg(0.017_453_292), half)));
            if fp::fmuls(reg(500.0), reg(t)) >= fp::fmuls(reg(c.width), reg(0.5)) {
                break;
            }
            c.fov = mem(fp::fadds(reg(c.fov), reg(0.1)));
        }
        if reg(c.fov) < reg(3.0) {
            c.fov = 3.2;
        }
        let fov = c.fov;
        cobj::set_fov(&mut self.game.objs.cobjs, Some(cobj), fov);
        let c = &mut self.game.toys.collection;
        c.fov_min = mem(fp::fmadds(int_f(c.count), reg(0.003_333_333_4), reg(3.0)));
        c.fov_max = mem(fp::fadds(reg(c.fov), fp::fdivs(reg(c.fov), reg(5.0))));
        let div = if reg(c.fov) < reg(3.0) { 30.0 } else { 60.0 };
        c.zoom_step = mem(fp::fdivs(
            fp::fsubs(reg(c.fov_max), reg(c.fov_min)),
            reg(div),
        ));
        let margin = match c.mode {
            0 | 1 => Some(fp::fadds(reg(14.0), reg(c.width))),
            2 => Some(fp::fadds(reg(7.0), reg(c.width))),
            3 => Some(reg(c.width)),
            _ => None,
        };
        if let Some(m) = margin {
            c.left = mem(fp::fnmsubs(m, reg(0.5), reg(c.interest.x)));
            c.right = mem(fp::fmadds(m, reg(0.5), reg(c.interest.x)));
        }
        let angle = |y: f64| {
            let a = super::super::ft::co::guard::stick_angle(y, reg(500.0));
            mem(fp::fmuls(reg(57.295_78), a))
        };
        c.yaw_max = angle(fp::fmuls(fp::fsubs(reg(c.right), reg(c.left)), reg(0.5)));
        let c = &mut self.game.toys.collection;
        c.pitch_max = angle(fp::fmuls(reg(c.width), reg(0.5)));
        let bg = self.game.toys.collection.bg.expect("the table");
        let Some(Obj::JObj(j)) = self.game.world.gobj(bg).obj else {
            panic!("the table has no joint");
        };
        jobj::set_translate(&mut self.game.objs.jobjs, j, middle);
        let c = &self.game.toys.collection;
        let zr = fp::fadds(reg(14.0), fp::fsubs(reg(c.max_z), reg(c.min_z)));
        let xr = fp::fsubs(reg(c.max_x), reg(c.min_x));
        let mut scale = int_f(c.count / 30);
        let mut span = zr;
        if zr < xr {
            span = fp::fadds(reg(14.0), xr);
        }
        if fp::fmuls(reg(38.0), scale) < span {
            while fp::fmuls(reg(38.0), scale) < span {
                scale = fp::fadds(scale, reg(0.1));
            }
        } else {
            while fp::fmuls(reg(38.0), scale) > span {
                scale = fp::fsubs(scale, reg(0.1));
            }
        }
        assert!(
            scale <= reg(2_147_483_600.0) && scale >= reg(-2_147_483_600.0),
            "*** tyDisplay Table Scale Irregul!"
        );
        if fp::fctiwz(scale) != 0 {
            let j = self
                .game
                .toys
                .collection
                .bg_jobj
                .expect("the table's joint");
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_srt(jobjs, j, |o| o.scale.x = mem(scale));
            jobj::set_srt(jobjs, j, |o| o.scale.z = mem(scale));
        }
    }

    /// `_tyDisplay_8031A4EC`, the camera's process: the sticks past their dead zones; after
    /// the entry's wait, L, R or Start on any port leave for the main menu, B or Start leave;
    /// the stick turns the camera (within the table's bounds, faster zoomed out), X, R and
    /// the D-pad's down zoom in, Y, L and its up out, A puts it back; the interest follows
    /// the turn.
    #[doc(alias = "_tyDisplay_8031A4EC")]
    #[doc(alias = "mn_8022F218")]
    #[doc(alias = "mn_8022F268")]
    pub fn coll_camera_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the collection's camera has no CObj");
        };
        let o = &mut self.game.objs;
        let _ = cobj::interest(o, data, c);
        let _ = cobj::eye_position(o, data, c);
        let mut fov = cobj::fov(&o.cobjs, Some(c));
        self.coll_sticks();
        let col = self.coll();
        if col.wait != 0 {
            col.wait -= 1;
            return;
        }
        if self.game.buttons_triggered(4) & super::super::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.toys.table.exit = 1;
            return;
        }
        if self.toy_triggered() & 0x1200 != 0 {
            self.lb_audio_call(0);
            self.game.toys.table.exit = 1;
            return;
        }
        self.coll_turn(fov);
        self.coll_zoom(c, &mut fov, 0x424, 0x848, 0x100);
        self.coll_follow_turn();
    }

    /// The camera processes' sticks: the control stick (`x20`, `x24`) and the C stick
    /// (`x30`, `x34`), each past its dead zone.
    fn coll_sticks(&mut self) {
        let sx = self.toy_stick_x();
        self.coll().stick[0] = sx;
        let sy = self.toy_axis(1);
        self.coll().stick[1] = sy;
        let col = self.coll();
        col.stick = [dead_zone(col.stick[0]), dead_zone(col.stick[1])];
        let cx = self.toy_axis(2);
        self.coll().cstick[0] = cx;
        let cy = self.toy_axis(3);
        self.coll().cstick[1] = cy;
        let col = self.coll();
        col.cstick = [dead_zone(col.cstick[0]), dead_zone(col.cstick[1])];
    }

    /// The camera processes' turn: the stick's x turns the yaw, its y the pitch, faster
    /// zoomed out, each within its bound.
    fn coll_turn(&mut self, fov: f32) {
        let col = self.coll();
        let s = col.stick[0];
        if reg(s) != 0.0 {
            col.yaw = mem(fp::fnmsubs(
                reg(s),
                fp::fmuls(reg(0.02), reg(fov)),
                reg(col.yaw),
            ));
            if reg(col.yaw) < -reg(col.yaw_max) {
                col.yaw = -col.yaw_max;
            }
            if reg(col.yaw) > reg(col.yaw_max) {
                col.yaw = col.yaw_max;
            }
        }
        let s = col.stick[1];
        if reg(s) != 0.0 {
            col.pitch = mem(fp::fmadds(
                reg(s),
                fp::fmuls(reg(0.02), reg(fov)),
                reg(col.pitch),
            ));
            if reg(col.pitch) < -reg(col.pitch_max) {
                col.pitch = -col.pitch_max;
            }
            if reg(col.pitch) > reg(col.pitch_max) {
                col.pitch = col.pitch_max;
            }
        }
    }

    /// The camera processes' zoom: the field of view widened a step while `wider` is held
    /// (to its bound), narrowed while `narrower` is, and the camera put back when `reset` is
    /// pressed.
    fn coll_zoom(&mut self, c: cobj::CObjId, fov: &mut f32, wider: u32, narrower: u32, reset: u32) {
        if self.toy_held() & wider != 0 {
            let col = &self.game.toys.collection;
            *fov = mem(fp::fadds(reg(*fov), reg(col.zoom_step)));
            if reg(*fov) > reg(col.fov_max) {
                *fov = col.fov_max;
            }
            cobj::set_fov(&mut self.game.objs.cobjs, Some(c), *fov);
        }
        if self.toy_held() & narrower != 0 {
            let col = &self.game.toys.collection;
            *fov = mem(fp::fsubs(reg(*fov), reg(col.zoom_step)));
            if reg(*fov) < reg(col.fov_min) {
                *fov = col.fov_min;
            }
            cobj::set_fov(&mut self.game.objs.cobjs, Some(c), *fov);
        }
        if self.toy_triggered() & reset != 0 {
            let col = &self.game.toys.collection;
            let (i, e, f) = (col.interest, col.eye, col.fov);
            let o = &mut self.game.objs;
            cobj::set_interest(o, c, i);
            cobj::set_fov(&mut o.cobjs, Some(c), f);
            let col = self.coll();
            col.yaw = 0.0;
            col.pitch = 0.0;
            cobj::set_eye_position(&mut self.game.objs, c, e);
        }
    }

    /// The camera processes' end: the camera's interest at the turn's angles from the eye's
    /// x, 500 ahead, at the resting interest's depth.
    fn coll_follow_turn(&mut self) {
        let data = self.data;
        let cam = self.game.toys.collection.camera.expect("the camera");
        let Some(Obj::CObj(c2)) = self.game.world.gobj(cam).obj else {
            panic!("the collection's camera has no CObj");
        };
        let o = &mut self.game.objs;
        let _ = cobj::interest(o, data, c2);
        let _ = cobj::eye_position(o, data, c2);
        let col = &self.game.toys.collection;
        let mut v = Vec3::new(col.eye.x, 0.0, -500.0);
        let deg = reg(0.017_453_292);
        v.rotate_euler(Vec3::new(
            mem(fp::fmuls(deg, reg(col.pitch))),
            mem(fp::fmuls(deg, reg(col.yaw))),
            0.0,
        ));
        v.z = col.interest.z;
        cobj::set_interest(&mut self.game.objs, c2, v);
    }

    /// `_tyDisplay_8031B1FC`: the table (`TyMnDisp`'s `ToyDspBg_Top`) on a GObj (class 9,
    /// link 9, drawn on link 0x3C), its part 0x15 kept to scale.
    #[doc(alias = "_tyDisplay_8031B1FC")]
    fn coll_table(&mut self) {
        let a = self
            .game
            .toys
            .collection
            .archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        if let Some(old) = self.coll().bg.take() {
            gobj::free(self, old);
        }
        let Some(joint) = self.coll_sym(a.id, "ToyDspBg_Top_joint") else {
            panic!("*** Can not Load Panel Label(ToyDspBg_Top_joint)");
        };
        let g = self.game.world.create(9, 9, 0);
        self.coll().bg = Some(g);
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, Some(joint)).expect("the table");
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        let [part] = super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[0x15]);
        self.coll().bg_jobj = part;
    }

    /// `_tyDisplay_8031B328`: the scene's lights (`ScMenDisplay_scene_lights`, on link 0x34)
    /// and fog (`ScMenDisplay_fog`, on link 0x35) in the lights' block.
    #[doc(alias = "_tyDisplay_8031B328")]
    fn coll_lights(&mut self) {
        let a = self
            .game
            .toys
            .collection
            .archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        if let Some(list) = self.coll_sym(a.id, "ScMenDisplay_scene_lights") {
            let g = self.game.world.create(2, 3, 0);
            self.game.toys.gallery.scene_lights = Some(g);
            let (l, _) = self.toy_load_lights(list, false);
            let w = &mut self.game.world;
            if let Some(l) = l {
                w.set_obj(g, OBJ_LIGHT, Obj::LObj(l));
            }
            w.setup_gx_link(g, CodeRef(LOBJ_CALLBACK), 0x34, 0);
            // The debug view's grey light (`_tyDisplay_color_C8C8C8FF`) on the list's first.
            if self.game.toys.collection.debug != 0
                && let Some(l) = l
            {
                let grey = [0xC8, 0xC8, 0xC8, 0xFF];
                crate::hsd::lobj::set_color(&mut self.game.objs.lobjs, l, grey);
            }
        }
        if let Some(desc) = self.coll_sym(a.id, "ScMenDisplay_fog") {
            let g = self.game.world.create(3, 4, 0);
            self.game.toys.gallery.fog = Some(g);
            let data = self.data;
            let f = fog::load_desc(&mut self.game.objs.fogs, data, desc);
            let w = &mut self.game.world;
            w.set_obj(g, OBJ_FOG, Obj::Fog(f));
            w.setup_gx_link(g, CodeRef(RENDER_FOG), 0x35, 0);
        }
    }

    fn coll_sym(&self, a: melee_data::archive::ArchiveId, name: &str) -> Option<DataRef> {
        if name.is_empty() {
            return None;
        }
        self.data
            .get(a)
            .public(name)
            .map(|offset| DataRef { archive: a, offset })
    }

    /// The grid's columns and rows for the trophies (the scene's entry): grown by turns,
    /// the columns to 20 at most; one more of each for the rings.
    fn coll_grid_size(&mut self) {
        let c = self.coll();
        let rings = matches!(c.mode, 0 | 1);
        if !matches!(c.mode, 0..=3) {
            return;
        }
        c.cols = 1;
        c.rows = 1;
        let mut toggle = false;
        while i32::from(c.cols as i8) * i32::from(c.rows as i8) < c.count {
            toggle = !toggle;
            if toggle && (c.cols as i8) < 0x14 {
                c.cols = c.cols.wrapping_add(1);
            } else {
                c.rows = c.rows.wrapping_add(1);
            }
        }
        if rings {
            c.cols = c.cols.wrapping_add(1);
            c.rows = c.rows.wrapping_add(1);
        }
    }

    /// `tyDisplay_Scene_OnEnter`: the five blocks; the layout from the buttons held (L
    /// rows, R even rings, Y the triangle, else jittered rings); the table prepared; the
    /// trophies counted, the grid sized; `TyMnDisp` and the categories' archives; the camera
    /// (`ScMenDisplay_cam_int1_camera`, with its process), lights, fog and table; the layout
    /// (rows and triangle with a coin toss) and the framing.
    #[doc(alias = "tyDisplay_Scene_OnEnter")]
    pub fn coll_on_enter(&mut self) {
        let alloc = |n: u32| crate::hsd::heap::mem_alloc(n);
        let jobjs_block = alloc(4 * JOBJS as u32);
        let grid_block = alloc(0x12E4);
        let cfg_block = alloc(0x80);
        let bg_block = alloc(0x108);
        let light_block = alloc(0xE4);
        let (debug_b, holder) = (
            self.game.toys.collection.debug_b,
            self.game.toys.collection.holder,
        );
        self.game.toys.collection = Collection {
            debug_b,
            holder,
            jobjs_block,
            jobjs: vec![None; JOBJS],
            grid_block,
            sort: vec![(0, 0); GRID],
            pos: vec![[0.0, 0.0]; GRID],
            cfg_block,
            bg_block,
            archives: vec![None; 43],
            ..Collection::default()
        };
        self.game.toys.gallery.light_block = light_block;
        let mode = if self.toy_held() & 0x40 != 0 {
            2
        } else if self.toy_held() & 0x20 != 0 {
            1
        } else if self.toy_held() & 0x800 != 0 {
            3
        } else {
            0
        };
        self.coll().mode = mode;
        // From the debug ROM's level, Z held: the debug view (`_tyDisplay_804D6F20`), from
        // the first trophy.
        if self.game.gm.db_level >= crate::game::gm::DB_DEBUG_ROM && self.toy_held() & 0x10 != 0 {
            self.coll().debug = 1;
        }
        self.toy_scene_prepare();
        self.gal_reset_lights_block(light_block);
        let total = self.toy_total();
        self.coll().count = total;
        if self.game.toys.collection.debug != 0 {
            self.coll().count = 1;
            self.coll_debug_text();
        }
        self.coll_grid_size();
        let name = if self.saved_us() {
            "TyMnDisp.usd"
        } else {
            "TyMnDisp.dat"
        };
        let a = self.toy_load(name);
        self.coll().archive = Some(a);
        for (i, f) in FILES.iter().enumerate() {
            let l = self.toy_load(f);
            self.coll().archives[i] = Some(l);
        }
        self.coll().x104 = 0;
        let data = self.data;
        let desc = self.coll_sym(a.id, "ScMenDisplay_cam_int1_camera");
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Data(desc.expect("the collection's camera")),
        );
        let g = self.game.world.create(1, 2, 0);
        self.coll().camera = Some(g);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(g, CodeRef(super::gallery::RENDER_CAMERA), 0, false);
        w.gobj_mut(g).gxlink_prios = 0x1230_0000_0000_0000;
        let proc = if self.game.toys.collection.debug != 0 {
            PROC_DEBUG_CAMERA
        } else {
            PROC_CAMERA
        };
        w.setup_proc(g, CodeRef(proc), 0);
        w.mark_ran(g);
        self.coll_lights();
        self.coll_table();
        if self.game.toys.collection.debug != 0 {
            self.coll_clear_grid();
            let id = self.game.toys.collection.x7c;
            let g = self.coll_trophy(id);
            self.coll().last = Some(g);
            self.coll_frame();
            return;
        }
        match mode {
            0 | 1 => self.coll_rings(mode),
            2 => {
                let s = self.game.objs.rng.below(2);
                self.coll_rows(s);
            }
            _ => {
                let s = self.game.objs.rng.below(2);
                self.coll_triangle(s);
            }
        }
        self.coll_frame();
    }

    /// `tyDisplay_8031BC54(id)`: a trophy's display model: a root (on a GObj of class 6,
    /// link 7, drawn on link 0x3C) holding its category's model (its material animation at
    /// the entry's frame, then let go) at the entry's place, and the stand.
    #[doc(alias = "_tyDisplay_8031BC54")]
    fn coll_trophy(&mut self, id: i32) -> GObjId {
        let e = self.coll_entry(id);
        let g = self.game.world.create(6, 7, 0);
        let root = jobj::alloc(&mut self.game.objs);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(root));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        let cat = usize::from(e.category);
        let name_at = if e.category as i8 == -1 { 0 } else { cat };
        let archive = |r: &Self, i: usize| {
            r.game.toys.collection.archives[i]
                .expect("a category's archive")
                .id
        };
        let a = archive(self, cat);
        let data = self.data;
        let joint = self.coll_sym(a, JOINTS[name_at]);
        let child = jobj::load_joint(&mut self.game.objs, data, joint);
        jobj::add_child(&mut self.game.objs.jobjs, Some(root), child);
        let mat = matanim(name_at);
        let m = (!mat.is_empty()).then_some(mat.as_str());
        self.toy_add_anims(child, [None, m, None], a, i32::from(e.frame));
        jobj::remove_anim_all(&mut self.game.objs, child);
        let child = child.expect("a category's model");
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, child, |o| o.translate.x = e.x);
        jobj::set_srt(jobjs, child, |o| o.translate.z = e.z);
        let stand = self.coll_sym(archive(self, 41), JOINTS[41]);
        let s = jobj::load_joint(&mut self.game.objs, data, stand);
        jobj::add_child(&mut self.game.objs.jobjs, Some(root), s);
        // The debug view: the entry's place in its text, and the trophy as the gallery shows
        // it beside.
        if self.game.toys.collection.text.is_some() {
            self.coll_debug_print(e.x, e.z);
            self.coll_debug_model(id);
        }
        g
    }

    /// The debug view's text: erased, then `X  %3.2f` and `Z  %3.2f` of a place.
    fn coll_debug_print(&mut self, x: f32, z: f32) {
        use crate::msl::Arg;
        let t = self
            .game
            .toys
            .collection
            .text
            .expect("the debug view's text");
        let s = crate::msl::vsprintf(
            b"X  %3.2f\nZ  %3.2f",
            &[Arg::Float(f64::from(x)), Arg::Float(f64::from(z))],
        );
        let text = &mut self.game.hud.devtext.pool[usize::from(t)];
        text.erase();
        text.set_cursor_xy(0, 0);
        text.print(&s);
    }

    /// `_tyDisplay_8031C1D0`: the debug view's text (id 1, at 40, 40, 9 by 3 cells): shown
    /// without a cursor or wrapping, blue, its cells 12 by 18, light grey `X  0.00` and
    /// `Z  0.00`.
    #[doc(alias = "_tyDisplay_8031C1D0")]
    fn coll_debug_text(&mut self) {
        use crate::msl::Arg;
        let t = self.game.hud.devtext.create(1, 40, 40, 9, 3);
        self.coll().text = t;
        let Some(t) = t else {
            return;
        };
        self.game.hud.devtext.show(t);
        let text = &mut self.game.hud.devtext.pool[usize::from(t)];
        text.hide_cursor();
        text.no_wrap();
        text.set_bg_color([0x40, 0x80, 0xD0, 0x60]);
        text.set_scale(12.0, 18.0);
        text.erase();
        text.set_cursor_xy(0, 0);
        text.store_color_index(0);
        text.set_text_color([0xE2, 0xE2, 0xE2, 0xFF]);
        let s = crate::msl::vsprintf(b"X  %3.2f\nZ  %3.2f", &[Arg::Float(0.0), Arg::Float(0.0)]);
        text.print(&s);
    }

    /// `_tyDisplay_8031BA78(id, axis, v)`: a trophy's display entry's x (`axis` 0) or z (2)
    /// set to `v`, written into `TyDatai`, and the debug view's text.
    #[doc(alias = "_tyDisplay_8031BA78")]
    fn coll_debug_place(&mut self, id: i32, axis: i32, v: f32) {
        let at = self.coll_entry(id).at;
        match axis {
            0 => self.datai_write(at.at(8), v.to_bits()),
            2 => self.datai_write(at.at(0xC), v.to_bits()),
            _ => {}
        }
        if self.game.toys.collection.text.is_some() {
            let e = self.coll_entry(id);
            self.coll_debug_print(e.x, e.z);
        }
    }

    /// `_tyDisplay_8031BF34(id)`: the trophy as the gallery shows it: the last one's archive
    /// let go, its model file into `_tyDisplay_804A2DD0` and loaded (`Toy_80308250`), shown
    /// (`Toy_803087F4`) on its first stand, at six tenths and 12 to the right.
    #[doc(alias = "_tyDisplay_8031BF34")]
    fn coll_debug_model(&mut self, id: i32) {
        if self.game.toys.collection.model.is_some()
            && let Some(a) = self.coll().holder.archive.take()
        {
            self.toy_free(a);
        }
        // `Toy_80308250(holder, id, 0)`.
        let trophy = id as i16;
        let file = self.toy_model_file(i32::from(trophy));
        if let Some(a) = self.coll().holder.archive.take() {
            self.toy_free(a);
        }
        let name = self.toy_file_name(file);
        let l = self.toy_load(&name);
        self.coll().holder = Holder {
            file: Some(file),
            trophy,
            archive: Some(l),
        };
        let (g, a) = self.toy_show(file, trophy, Some(l));
        self.coll().holder.archive = Some(a);
        self.coll().model = Some(g);
        let an = self.game.toys.anim;
        let jobjs = &mut self.game.objs.jobjs;
        jobj::clear_flags_all(jobjs, an.jobj[0], jobj::HIDDEN);
        jobj::set_flags_all(jobjs, an.jobj[1], jobj::HIDDEN);
        jobj::clear_flags_all(jobjs, an.jobj[0], jobj::HIDDEN);
        let Some(Obj::JObj(j)) = self.game.world.gobj(g).obj else {
            panic!("the debug view's trophy has no joint");
        };
        jobj::set_srt(jobjs, j, |o| o.scale.x = 0.6);
        jobj::set_srt(jobjs, j, |o| o.scale.y = 0.6);
        jobj::set_srt(jobjs, j, |o| o.scale.z = 0.6);
        jobj::set_srt(jobjs, j, |o| {
            o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(12.0)));
        });
    }

    /// The debug view's trophy's model (`GET_JOBJ(x78)->child`).
    fn coll_debug_child(&self) -> JObjId {
        let last = self
            .game
            .toys
            .collection
            .last
            .expect("the debug view's trophy");
        let Some(Obj::JObj(root)) = self.game.world.gobj(last).obj else {
            panic!("the debug view's trophy has no joint");
        };
        self.game.objs.jobjs[root]
            .child
            .expect("the trophy's model")
    }

    /// `_tyDisplay_8031A94C`, the debug view's camera process: the sticks past their dead
    /// zones; B held two seconds leaves; A with the stick past 0.8, or the D-pad, moves the
    /// trophy a hundredth in its entry; R and L show the next and the last trophy; else the
    /// stick turns the camera (without A), the C stick's y raises or lowers the eye, X and Y
    /// zoom, Start puts it back; the interest follows the turn.
    #[doc(alias = "_tyDisplay_8031A94C")]
    pub fn coll_debug_camera_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the collection's camera has no CObj");
        };
        let o = &mut self.game.objs;
        let _ = cobj::interest(o, data, c);
        let mut eye = cobj::eye_position(o, data, c).expect("the camera's eye");
        let mut fov = cobj::fov(&o.cobjs, Some(c));
        self.coll_sticks();
        let col = self.coll();
        if col.wait != 0 {
            col.wait -= 1;
            return;
        }
        if self.toy_held() & 0x200 != 0 {
            self.coll().debug_b += 1;
            if self.game.toys.collection.debug_b > 0x78 {
                self.lb_audio_call(0);
                self.game.toys.table.exit = 1;
            }
            return;
        }
        self.coll().debug_b = 0;
        // Left, right, back and forward: A with the stick past 0.8, or the D-pad pressed.
        let stick = self.game.toys.collection.stick;
        for (axis, step, past, pad) in [
            (0, -0.01, reg(stick[0]) < reg(-0.8), 1),
            (0, 0.01, reg(stick[0]) > reg(0.8), 2),
            (2, -0.01, reg(stick[1]) > reg(0.8), 8),
            (2, 0.01, reg(stick[1]) < reg(-0.8), 4),
        ] {
            if (self.toy_held() & 0x100 != 0 && past) || self.toy_triggered() & pad != 0 {
                let j = self.coll_debug_child();
                let jobjs = &mut self.game.objs.jobjs;
                let v = if axis == 0 {
                    jobj::set_srt(jobjs, j, |o| {
                        o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(step)));
                    });
                    jobjs[j].translate.x
                } else {
                    jobj::set_srt(jobjs, j, |o| {
                        o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(step)));
                    });
                    jobjs[j].translate.z
                };
                let id = self.game.toys.collection.x7c;
                self.coll_debug_place(id, axis, v);
            }
        }
        for (pad, next) in [(0x20, true), (0x40, false)] {
            if self.toy_triggered() & pad != 0 {
                let last = self.coll().last.take().expect("the debug view's trophy");
                gobj::free(self, last);
                let col = self.coll();
                col.x7c = if next {
                    if col.x7c + 1 >= super::TROPHY_COUNT as i32 {
                        0
                    } else {
                        col.x7c + 1
                    }
                } else if col.x7c - 1 < 0 {
                    0x124
                } else {
                    col.x7c - 1
                };
                let id = col.x7c;
                let g = self.coll_trophy(id);
                self.coll().last = Some(g);
                return;
            }
        }
        if self.toy_held() & 0x100 == 0 {
            self.coll_turn(fov);
        }
        let cy = self.game.toys.collection.cstick[1];
        if reg(cy) > reg(0.8) {
            eye.y = mem(fp::fadds(reg(eye.y), reg(1.0)));
            cobj::set_eye_position(&mut self.game.objs, c, eye);
        }
        if reg(cy) < reg(-0.8) {
            eye.y = mem(fp::fsubs(reg(eye.y), reg(1.0)));
            cobj::set_eye_position(&mut self.game.objs, c, eye);
        }
        self.coll_zoom(c, &mut fov, 0x400, 0x800, 0x1000);
        self.coll_follow_turn();
    }

    /// `tyDisplay_Scene_OnFrame`: once the table says so, the collection let go
    /// (`_tyDisplay_8031B850`: its pointers forgotten, the camera's processes removed) and
    /// the scene ends.
    #[doc(alias = "tyDisplay_Scene_OnFrame")]
    #[doc(alias = "_tyDisplay_8031B850")]
    pub fn coll_on_frame(&mut self) {
        if self.game.toys.table.exit == 0 {
            return;
        }
        self.coll().archive = None;
        self.game.toys.stand = None;
        if self.game.toys.anim.gobj.is_some() {
            let a = &mut self.game.toys.anim;
            a.gobj = None;
            a.jobj = [None, None];
        }
        self.coll().bg = None;
        let g = &mut self.game.toys.gallery;
        g.scene_lights = None;
        g.lights = None;
        g.fog = None;
        if let Some(c) = self.coll().camera.take() {
            self.game.world.remove_all_procs(c);
        }
        // The debug view's trophy and text. `DevText_Remove(&_tyDisplay_804D6F24)`: the
        // static stands for the list's link, so a screen first in the draw list stays its
        // head ([`Link::Held`](crate::game::hud::devtext::Link::Held)).
        if let Some(m) = self.coll().model.take() {
            if let Some(a) = self.coll().holder.archive.take() {
                self.toy_free(a);
            }
            gobj::free(self, m);
        }
        if let Some(t) = self.coll().text.take() {
            use crate::game::hud::devtext::Link;
            self.game.hud.devtext.remove(Link::Held(t));
        }
        self.game.end_scene();
    }
}

#[cfg(test)]
mod tests {
    use super::quicksort;

    #[test]
    fn quicksort_sorts() {
        let mut a = [5, 3, 9, 1, 7, 3, 2, 8];
        let n = a.len() as i32 - 1;
        quicksort(&mut a, 0, n, &|x, y| x < y);
        assert_eq!(a, [1, 2, 3, 3, 5, 7, 8, 9]);
    }
}

//! The stage's debug drawings (`mplib.c`'s `mpLib_8005A2DC`, `mpLib_DrawZones` and
//! `mpLib_DrawSpecialPoints`), which the develop tools' stage display steps turn on: the
//! fighters' and items' ECBs and ledge snaps, the collision lines by kind, terrain or ledge,
//! the blast zones, camera bounds and camera boxes, and the stage's special points. The
//! display pass computes every vertex as the original does and hands the presentation the
//! primitives ([`MapDraw`]); the state the drawing changes (the lights set up, a camera box's
//! framing timer) changes here.

use gekko_fp as fp;

use super::camera;
use super::gr::{ground, stage};
use super::state::Running;
use crate::fpu::{mem, reg};
use crate::game::structs::CollData;
use crate::hsd::gobj::GObjId;
use crate::lb::coll_draw::{self, DebugDraw};
use crate::vec::Vec3;

/// A primitive's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Prim {
    Quads,
    Lines,
    LineStrip,
}

/// One of the stage drawings' GX steps.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum MapDraw {
    /// The vertex setup: positions alone, given directly, the view matrix loaded; culling off
    /// through HAL's cache (`hsd`, `HSD_StateSetCullMode`) or in GX directly.
    Setup { hsd: bool },
    /// `mpLib_SetupDraw(color)`: HAL's TEV stage passing the channel's colour, the channel's
    /// material colour `color`, unlit.
    Color([u8; 4]),
    /// `GXSetZMode(enable, func, false)` and `GXSetLineWidth(16, GX_TO_ZERO)`; `func` 3 is
    /// `GX_LEQUAL`, 4 `GX_GREATER`.
    ZMode { enable: bool, func: u8 },
    /// A primitive of `count` vertices.
    Prims {
        prim: Prim,
        count: u16,
        verts: Vec<Vec3>,
    },
    /// `HSD_StateInvalidate(-1)`.
    Invalidate,
}

fn push(r: &mut Running, d: MapDraw) {
    coll_draw::push(&mut r.game.objs, DebugDraw::Map(d));
}

fn prims(r: &mut Running, prim: Prim, count: u16, verts: Vec<Vec3>) {
    push(r, MapDraw::Prims { prim, count, verts });
}

fn adds(a: f32, b: f32) -> f32 {
    mem(fp::fadds(reg(a), reg(b)))
}

fn subs(a: f32, b: f32) -> f32 {
    mem(fp::fsubs(reg(a), reg(b)))
}

/// A cross of `GX_LINES` around `p`, one unit each way in x and y.
fn small_cross(p: Vec3, z: f32) -> Vec<Vec3> {
    vec![
        Vec3::new(subs(p.x, 1.0), p.y, z),
        Vec3::new(adds(1.0, p.x), p.y, z),
        Vec3::new(p.x, subs(p.y, 1.0), z),
        Vec3::new(p.x, adds(1.0, p.y), z),
    ]
}

/// An ECB's four points around `at`, in depth `z`.
fn diamond(at: Vec3, e: &crate::game::structs::_ftECB, z: f32) -> Vec<Vec3> {
    [&e.top, &e.right, &e.bottom, &e.left]
        .iter()
        .map(|p| Vec3::new(adds(at.x, p.x), adds(at.y, p.y), z))
        .collect()
}

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

impl Running<'_> {
    /// A link's GObjs in order.
    fn links(&self, link: u8) -> Vec<GObjId> {
        let mut out = Vec::new();
        let mut g = self.game.world.first_in(link);
        while let Some(x) = g {
            out.push(x);
            g = self.game.world.gobj(x).next;
        }
        out
    }

    /// The end of the game camera's pass (`Camera_80030788`'s debug drawings): with the stage
    /// shown, its collisions, its zones and its special points as the camera's flags ask.
    pub fn stage_debug_drawings(&mut self) {
        let cam = &self.game.camera.cam;
        if !camera::stage_visible(cam) {
            return;
        }
        if camera::draw_collisions(cam) {
            self.map_collisions();
        }
        if self.game.camera.cam.x399_b4 {
            self.map_zones();
        }
        if camera::draw_special_points(&self.game.camera.cam) {
            self.map_special_points();
        }
    }

    /// `mpLib_8005A2DC`: the ECBs and ledge snaps, the collision lines, then HAL's state out of
    /// date. Its once-only report of the geometry's counts (`mpLib_804D64D0`, `OSReport`)
    /// shows nothing.
    #[doc(alias = "mpLib_8005A2DC")]
    fn map_collisions(&mut self) {
        self.map_snapping();
        self.map_lines();
        push(self, MapDraw::Invalidate);
    }

    /// An object's ECBs, `mpLib_DrawEcbs`: this frame's (filled, with a cross at its place),
    /// the previous frame's half a unit back, the frame before's a unit back, and the one the
    /// collision checks last stood at (`x28_vec`, its own ECB when `x34_flags.b6`) half a unit
    /// forward, outlined.
    #[doc(alias = "mpLib_DrawEcbs")]
    fn map_ecbs(&mut self, cd: &CollData) {
        let z = cd.cur_pos.z;
        push(self, MapDraw::Color([0xFF, 0xA0, 0x30, 0xFF]));
        prims(self, Prim::Quads, 4, diamond(cd.cur_pos, &cd.ecb, z));
        push(self, MapDraw::Color([0x80, 0x50, 0x18, 0xFF]));
        prims(self, Prim::Lines, 4, small_cross(cd.cur_pos, z));
        let z = subs(cd.prev_pos.z, 0.5);
        push(self, MapDraw::Color([0xFF, 0x5A, 0x20, 0xFF]));
        prims(self, Prim::Quads, 4, diamond(cd.prev_pos, &cd.prev_ecb, z));
        push(self, MapDraw::Color([0x80, 0x2D, 0x10, 0xFF]));
        prims(self, Prim::Lines, 4, small_cross(cd.prev_pos, z));
        let z = subs(cd.last_pos.z, 1.0);
        push(self, MapDraw::Color([0x20, 0x20, 0xFF, 0xFF]));
        prims(self, Prim::Quads, 4, diamond(cd.last_pos, &cd.xE4_ecb, z));
        push(self, MapDraw::Color([0x10, 0x10, 0x80, 0xFF]));
        prims(self, Prim::Lines, 4, small_cross(cd.last_pos, z));
        push(self, MapDraw::Color([0xFF, 0xFF, 0xFF, 0x80]));
        let ecb = if cd.x34_flags.b6 {
            &cd.x64_ecb
        } else {
            &cd.ecb
        };
        let mut v = diamond(cd.x28_vec, ecb, adds(0.5, cd.x28_vec.z));
        v.push(v[0]);
        prims(self, Prim::LineStrip, 5, v);
        push(self, MapDraw::Color([0xFF, 0xFF, 0xFF, 0x80]));
        let z = mem(fp::frsp(fp::fadd(0.5, reg(cd.x28_vec.z))));
        prims(self, Prim::Lines, 4, small_cross(cd.x28_vec, z));
    }

    /// `mpLib_DrawSnapping`: each fighter's ECBs and ledge snap boxes (each side's), for a
    /// fighter whose collision ran against the current geometry; each item's ECBs, and a
    /// hookshot's tip's.
    #[doc(alias = "mpLib_DrawSnapping")]
    fn map_snapping(&mut self) {
        let generation = ground::generation(self.game);
        let fighters = self.links(P_LINK_FIGHTER);
        let any_fighter = !fighters.is_empty();
        if any_fighter {
            push(self, MapDraw::Setup { hsd: true });
        }
        for g in fighters {
            let f = self.game.fighter_id(g);
            let cd = self.game.fighters[f.0].coll_data.clone();
            if !cd.x35_flags.b0 || cd.x38 != generation {
                continue;
            }
            self.map_ecbs(&cd);
            if reg(cd.ledge_snap_x) != 0.0 || reg(cd.ledge_snap_y) != 0.0 {
                let half = mem(fp::fmuls(0.5, reg(cd.ledge_snap_height)));
                let mid = adds(cd.cur_pos.y, cd.ledge_snap_y);
                let (top, bottom) = (adds(half, mid), subs(mid, half));
                let (x, z) = (cd.cur_pos.x, cd.cur_pos.z);
                let right = adds(adds(x, cd.ecb.right.x), cd.ledge_snap_x);
                let left = subs(adds(x, cd.ecb.left.x), cd.ledge_snap_x);
                for (color, out) in [
                    ([0x37, 0x37, 0xFF, 0x80], right),
                    ([0xFF, 0x37, 0x37, 0x80], left),
                ] {
                    push(self, MapDraw::Color(color));
                    let v = vec![
                        Vec3::new(out, top, z),
                        Vec3::new(out, bottom, z),
                        Vec3::new(x, bottom, z),
                        Vec3::new(x, top, z),
                        Vec3::new(out, top, z),
                    ];
                    prims(self, Prim::LineStrip, 5, v);
                }
            }
        }
        let items = self.links(super::it::P_LINK_ITEM);
        if items.is_empty() {
            return;
        }
        if !any_fighter {
            push(self, MapDraw::Setup { hsd: false });
        }
        for g in items {
            let ip = super::it::item(self.game, g);
            let (cd, kind) = (ip.x378_itemColl.clone(), ip.kind);
            if !cd.x35_flags.b0 {
                continue;
            }
            if cd.x38 == generation {
                self.map_ecbs(&cd);
            }
            if kind == super::it::kinds::LINK_HSHOT {
                let tip = super::it::item_mut(self.game, g)
                    .xDD4_itemVar
                    .link_hookshot()
                    .x0
                    .get()
                    .expect("a hookshot without its tip");
                let cd = super::it::link::link(self.game, tip).coll_data.clone();
                self.map_ecbs(&cd);
            }
        }
    }

    /// The lines whose `lo_flags` masked by `mask` are `value`, enabled and shown, as quads
    /// fifty units deep, `mpLib_DrawMatchingLines`.
    #[doc(alias = "mpLib_DrawMatchingLines")]
    fn map_matching_lines(&mut self, value: u32, mask: u32, color: [u8; 4]) {
        let g = self.game.geometry.as_ref().expect("the stage's geometry");
        let shown = |l: &crate::mp::Line| {
            l.flags & crate::mp::LINE_ENABLED != 0
                && l.flags & crate::mp::LINE_HIDDEN == 0
                && value == u32::from(l.lo_flags) & mask
        };
        let lines: Vec<_> = g.lines.iter().filter(|l| shown(l)).copied().collect();
        if lines.is_empty() {
            return;
        }
        let verts = quads(g, &lines);
        push(self, MapDraw::Color(color));
        prims(
            self,
            Prim::Quads,
            ((lines.len() as u32 * 4) & 0xFFFC) as u16,
            verts,
        );
    }

    /// The collision lines by kind, `mpLib_80059554`: floors, ceilings, right and left walls
    /// in their colours, then the dynamic lines by kind in darker ones.
    #[doc(alias = "mpLib_80059554")]
    fn map_lines_by_kind(&mut self) {
        use crate::mp::{CEILING, FLOOR, LEFT_WALL, RIGHT_WALL};
        let groups = [
            (crate::mp::GROUP_FLOOR, None, [0xC0, 0xC0, 0xC0, 0xFF]),
            (crate::mp::GROUP_CEILING, None, [0xC0, 0x80, 0x80, 0xFF]),
            (crate::mp::GROUP_RIGHT_WALL, None, [0x80, 0xC0, 0x80, 0xFF]),
            (crate::mp::GROUP_LEFT_WALL, None, [0x80, 0x80, 0xC0, 0xFF]),
            (
                crate::mp::GROUP_DYNAMIC,
                Some(FLOOR),
                [0x60, 0x60, 0x60, 0xFF],
            ),
            (
                crate::mp::GROUP_DYNAMIC,
                Some(CEILING),
                [0x60, 0x40, 0x40, 0xFF],
            ),
            (
                crate::mp::GROUP_DYNAMIC,
                Some(RIGHT_WALL),
                [0x40, 0x60, 0x40, 0xFF],
            ),
            (
                crate::mp::GROUP_DYNAMIC,
                Some(LEFT_WALL),
                [0x40, 0x40, 0x60, 0xFF],
            ),
        ];
        for (group, kind, color) in groups {
            let g = self.game.geometry.as_ref().expect("the stage's geometry");
            let r = g.ranges[group];
            let start = r.start.max(0) as usize;
            let lines: Vec<_> = g.lines[start..start + r.count.max(0) as usize]
                .iter()
                .filter(|l| {
                    l.flags & crate::mp::LINE_ENABLED != 0
                        && kind.is_none_or(|k| l.flags & k != 0)
                        && l.flags & crate::mp::LINE_HIDDEN == 0
                })
                .copied()
                .collect();
            if lines.is_empty() {
                continue;
            }
            let verts = quads(g, &lines);
            push(self, MapDraw::Color(color));
            prims(
                self,
                Prim::Quads,
                ((lines.len() as u32 * 4) & 0xFFFC) as u16,
                verts,
            );
        }
    }

    /// The collision lines, `mpLib_80059E60`: the lights set up and the vertex setup, then by
    /// terrain (the camera's `x39A_b0`: each terrain's colour, then the basic terrain's), by
    /// ledge and platform (`x39A_b1`), or by kind.
    #[doc(alias = "mpLib_80059E60")]
    fn map_lines(&mut self) {
        let c = self.game.objs.cobjs.current.expect("the game camera");
        crate::hsd::display::lobj_setup_init(&mut self.game.objs, self.data, c);
        push(self, MapDraw::Setup { hsd: false });
        if camera::x39a_b0(&self.game.camera.cam) {
            for (kind, color) in TERRAIN_PALETTE {
                self.map_matching_lines(kind, 0xFF, color);
            }
            // `mp_Terrain_Basic`.
            self.map_matching_lines(0, 0xFF, [0x80, 0x80, 0x80, 0xFF]);
        } else if camera::x39a_b1(&self.game.camera.cam) {
            use crate::mp::{LINE_LEDGE, LINE_PLATFORM};
            self.map_matching_lines(LINE_LEDGE, LINE_LEDGE, [0xFF, 0x40, 0x40, 0xFF]);
            self.map_matching_lines(LINE_PLATFORM, LINE_PLATFORM, [0x40, 0x40, 0xFF, 0xFF]);
            let both = LINE_LEDGE | LINE_PLATFORM;
            self.map_matching_lines(both, both, [0xFF, 0x40, 0xFF, 0xFF]);
            self.map_matching_lines(0, both, [0x80, 0x80, 0x80, 0xFF]);
        } else {
            self.map_lines_by_kind();
        }
    }

    /// `mpLib_DrawZones`: the blast zones and the camera bounds, each drawn twice (in front of
    /// the scene, and behind it in its fainter colour), then every camera box the camera
    /// frames (asking it advances an automatic box's timer), through everything.
    #[doc(alias = "mpLib_DrawZones")]
    fn map_zones(&mut self) {
        let c = self.game.objs.cobjs.current.expect("the game camera");
        crate::hsd::display::lobj_setup_init(&mut self.game.objs, self.data, c);
        push(self, MapDraw::Setup { hsd: false });
        let rect = |l: f32, r: f32, b: f32, t: f32| {
            vec![
                Vec3::new(l, b, 0.0),
                Vec3::new(r, b, 0.0),
                Vec3::new(r, t, 0.0),
                Vec3::new(l, t, 0.0),
                Vec3::new(l, b, 0.0),
            ]
        };
        let g = &*self.game;
        let blast = rect(
            stage::blast_left(g),
            stage::blast_right(g),
            stage::blast_bottom(g),
            stage::blast_top(g),
        );
        for (color, func) in [([0xFF, 0xFF, 0xC0, 0xFF], 3), ([0xFF, 0xFF, 0xFF, 0x80], 4)] {
            push(self, MapDraw::Color(color));
            push(self, MapDraw::ZMode { enable: true, func });
            prims(self, Prim::LineStrip, 5, blast.clone());
        }
        let g = &*self.game;
        let bounds = rect(
            stage::cam_left(g),
            stage::cam_right(g),
            stage::cam_bottom(g),
            stage::cam_top(g),
        );
        for (color, func) in [([0x80, 0xC0, 0xC0, 0xFF], 3), ([0xC0, 0xFF, 0xFF, 0x80], 4)] {
            push(self, MapDraw::Color(color));
            push(self, MapDraw::ZMode { enable: true, func });
            prims(self, Prim::LineStrip, 5, bounds.clone());
        }
        push(self, MapDraw::Color([0x40, 0x40, 0xFF, 0x00]));
        push(
            self,
            MapDraw::ZMode {
                enable: false,
                func: 3,
            },
        );
        let mut cur = self.game.camera.boxes.last;
        while let Some(b) = cur {
            let framed = camera::is_framed(&self.game.stage_info, &mut self.game.camera.boxes[b]);
            let s = &self.game.camera.boxes[b];
            if framed {
                let (l, r) = (adds(s.pos.x, s.ext.h.x), adds(s.pos.x, s.ext.h.y));
                let (bt, t) = (adds(s.pos.y, s.ext.v.x), adds(s.pos.y, s.ext.v.y));
                prims(self, Prim::LineStrip, 5, rect(l, r, bt, t));
            }
            cur = self.game.camera.boxes[b].prev;
        }
        push(self, MapDraw::Invalidate);
    }

    /// `mpLib_DrawCrosses`: a cross six units wide in each axis at each of the points the
    /// stage has (`Ground_801C2D24`), at most 0x80. The original keeps the points in
    /// `mpLib_80458888`, which only this call reads.
    #[doc(alias = "mpLib_DrawCrosses")]
    fn map_crosses(&mut self, ids: &[i16], color: [u8; 4]) {
        let mut pts = Vec::new();
        for &id in ids {
            if pts.len() >= 0x80 {
                break;
            }
            if let Some(p) = ground::point(self.game, self.data, i32::from(id)) {
                pts.push(p);
            }
        }
        if pts.is_empty() {
            return;
        }
        let mut verts = Vec::with_capacity(pts.len() * 6);
        for p in &pts {
            verts.extend([
                Vec3::new(subs(p.x, 3.0), p.y, p.z),
                Vec3::new(adds(3.0, p.x), p.y, p.z),
                Vec3::new(p.x, subs(p.y, 3.0), p.z),
                Vec3::new(p.x, adds(3.0, p.y), p.z),
                Vec3::new(p.x, p.y, subs(p.z, 3.0)),
                Vec3::new(p.x, p.y, adds(3.0, p.z)),
            ]);
        }
        push(self, MapDraw::Color(color));
        prims(self, Prim::Lines, (pts.len() * 6) as u16, verts);
    }

    /// `mpLib_DrawSpecialPoints`: the spawn points, the respawn points, the item spawns, the
    /// enemies', the trophies' and the exits', each in its colour.
    #[doc(alias = "mpLib_DrawSpecialPoints")]
    fn map_special_points(&mut self) {
        self.map_crosses(&[0, 1, 2, 3], [0x7D, 0x7D, 0xFF, 0xFF]);
        self.map_crosses(&[4, 5, 6, 7], [0x7D, 0xFF, 0x80, 0xFF]);
        self.map_crosses(&ITEM_SPAWN_IDS[..0x15], [0xFF, 0xFF, 0x80, 0xFF]);
        self.map_crosses(&ITEM_SPAWN_IDS[0x16..0x16 + 0x50], [0xFF, 0x40, 0x40, 0xFF]);
        self.map_crosses(&ITEM_SPAWN_IDS[0x66..0x66 + 0x19], [0xFF, 0x40, 0xC0, 0xFF]);
        self.map_crosses(&ITEM_SPAWN_IDS[0x80..0x80 + 0x2E], [0xFF, 0xFF, 0xFF, 0xFF]);
    }
}

/// The lines' quads: each line from its first vertex to its second at depth 25, and back at
/// -25.
fn quads(g: &super::gr::Geometry, lines: &[crate::mp::Line]) -> Vec<Vec3> {
    let mut v = Vec::with_capacity(lines.len() * 4);
    for l in lines {
        let (a, b) = (g.vertices[l.v0 as usize].pos, g.vertices[l.v1 as usize].pos);
        v.extend([
            Vec3::new(a.x, a.y, 25.0),
            Vec3::new(b.x, b.y, 25.0),
            Vec3::new(b.x, b.y, -25.0),
            Vec3::new(a.x, a.y, -25.0),
        ]);
    }
    v
}

/// The terrains' colours (`mpIsland_TerrainPalette`), by terrain kind.
const TERRAIN_PALETTE: [(u32, [u8; 4]); 19] = [
    (1, [0x80, 0x60, 0x60, 0xFF]),
    (2, [0x40, 0xFF, 0x40, 0xFF]),
    (3, [0xC0, 0x60, 0x60, 0xFF]),
    (4, [0xC0, 0x80, 0x40, 0xFF]),
    (5, [0x40, 0x40, 0x40, 0xFF]),
    (6, [0x60, 0x40, 0x40, 0xFF]),
    (7, [0xC0, 0xC0, 0x60, 0xFF]),
    (8, [0xFF, 0xFF, 0x60, 0xFF]),
    (9, [0xC0, 0xFF, 0xC0, 0xFF]),
    (10, [0x40, 0x40, 0xFF, 0xFF]),
    (11, [0x40, 0xFF, 0xFF, 0xFF]),
    (12, [0xC0, 0xC0, 0xFF, 0xFF]),
    (13, [0xC0, 0xFF, 0x40, 0xFF]),
    (14, [0xFF, 0xFF, 0xFF, 0xFF]),
    (15, [0xC0, 0xC0, 0xFF, 0xFF]),
    (16, [0xC0, 0xC0, 0xC0, 0xFF]),
    (17, [0x0C, 0x0C, 0x0C, 0xFF]),
    (18, [0xFF, 0xFF, 0xC0, 0xFF]),
    (19, [0xFF, 0x05, 0x05, 0xFF]),
];

/// The special points' ids (`mpLib_ItemSpawnVtxIds`): the item spawns from 0, the enemies'
/// from 0x16, the trophies' from 0x66, the exits' from 0x80.
const ITEM_SPAWN_IDS: [i16; 0xAE] = [
    0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E,
    0x8F, 0x90, 0x91, 0x92, 0x93, 0, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
    0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5,
    0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0,
    0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8,
    0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8,
    0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6,
];

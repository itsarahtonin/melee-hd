//! The full mplib state on both sides, compared field by field.

use leaf_check::sym;
use melee_data::archive::Archive;
use melee_sim::mp::{Island, IslandId};

use super::Stage;

/// ssbm-rs's joint pointer as our index.
fn joint_index(base: u32, p: u32) -> Option<u16> {
    (p != 0).then(|| ((p - base) / 0x34) as u16)
}

/// The descriptors decoded at load against the data ssbm-rs reads them from.
pub fn check_load(s: &Stage, archive: &Archive, coll_off: u32) {
    let ctx = s.ctx;
    let r32 = |a: u32| ctx.read_u32(a);
    let r16 = |a: u32| ctx.read_u16(a);
    let coll = s.coll;
    let what = s.what();
    assert_eq!(
        s.g.vertices.len() as u32,
        r32(coll + 4),
        "{what}: vertex count"
    );
    assert_eq!(
        s.g.lines.len() as u32,
        r32(coll + 0xC),
        "{what}: line count"
    );
    assert_eq!(
        s.g.joints.len() as u32,
        r32(coll + 0x28),
        "{what}: joint count"
    );
    for (g, r) in s.g.ranges.iter().enumerate() {
        let at = coll + 0x10 + 4 * g as u32;
        assert_eq!(
            (r.start as u16, r.count as u16),
            (r16(at), r16(at + 2)),
            "{what}: range {g}"
        );
    }
    let descs = r32(coll + 0x24);
    for (j, joint) in s.g.joints.iter().enumerate() {
        let at = descs + 0x28 * j as u32;
        let d = &joint.desc;
        for (g, r) in d.ranges.iter().enumerate() {
            let a = at + 4 * g as u32;
            assert_eq!(
                (r.start as u16, r.count as u16),
                (r16(a), r16(a + 2)),
                "{what}: joint {j} range {g}"
            );
        }
        assert_eq!(
            [d.left, d.bottom, d.right, d.top].map(f32::to_bits),
            [
                r32(at + 0x14),
                r32(at + 0x18),
                r32(at + 0x1C),
                r32(at + 0x20)
            ],
            "{what}: joint {j} bounds"
        );
        assert_eq!(
            (d.vtx_start as u16, d.vtx_count as u16),
            (r16(at + 0x24), r16(at + 0x26)),
            "{what}: joint {j} vertices"
        );
    }
    // Every line belongs to a group: the original leaves a line outside them without flags
    // (uninitialized heap), so the comparison relies on the groups covering the lines.
    let mut covered = vec![false; s.g.lines.len()];
    for r in s.g.ranges {
        for c in covered
            .iter_mut()
            .skip(r.start as usize)
            .take(r.count.max(0) as usize)
        {
            *c = true;
        }
    }
    assert!(
        covered.iter().all(|&c| c),
        "{what}: a line belongs to no group"
    );
    // Facts the port relies on for inputs the game produces: no line is both a floor and a
    // wall (`mpLib_80056C54` would read its uninitialized wall length), and every terrain has
    // an entry in the terrain tables.
    for (i, l) in s.g.lines.iter().enumerate() {
        assert!(
            !(l.hi_flags & 1 != 0 && l.hi_flags & 0xC != 0),
            "{what}: line {i} is a floor and a wall"
        );
        assert!(
            l.lo_flags & 0xFF < 20,
            "{what}: line {i} has terrain {}",
            l.lo_flags & 0xFF
        );
    }
    let _ = (archive, coll_off);
}

/// Pairs ssbm-rs's island at `addr` with ours, checking the pairing is one to one.
fn pair_island(s: &mut Stage, addr: u32, ours: IslandId, what: &str) {
    let known = s.islands.get(&addr).copied();
    let back = s.islands_back.get(&ours.0).copied();
    match (known, back) {
        (None, None) => {
            s.islands.insert(addr, ours.0);
            s.islands_back.insert(ours.0, addr);
        }
        (Some(o), Some(a)) if o == ours.0 && a == addr => {}
        _ => panic!(
            "{what}: island {addr:#x} pairs with ours {ours:?}, but was {known:?} / {back:x?}"
        ),
    }
}

fn island_fields(i: &Island) -> [u32; 13] {
    [
        u32::from(i.left_vtx),
        u32::from(i.right_vtx),
        i.left.x.to_bits(),
        i.left.y.to_bits(),
        i.left.z.to_bits(),
        i.right.x.to_bits(),
        i.right.y.to_bits(),
        i.right.z.to_bits(),
        i.flags as u32,
        i.first_line as u16 as u32,
        i.last_line as u16 as u32,
        i.joint as u16 as u32,
        0,
    ]
}

/// Compares the whole state: vertices, lines, joints, the joint list, the bounding flag, the
/// boxes, the update counter and every island list.
pub fn check(s: &mut Stage, step: &str) {
    let ctx = s.ctx;
    let what = format!("{}: {step}", s.what());
    let r32 = |a: u32| ctx.read_u32(a);
    let r16 = |a: u32| ctx.read_u16(a);
    let coll = s.coll;
    let vtx = r32(sym("groundCollVtx"));
    let lines = r32(sym("groundCollLine"));
    let joints = r32(sym("groundCollJoint"));
    let g = &s.g;
    for (i, v) in g.vertices.iter().enumerate() {
        let a = vtx + 0x18 * i as u32;
        let theirs: [u32; 6] = std::array::from_fn(|k| r32(a + 4 * k as u32));
        let ours = [v.base.x, v.base.y, v.pos.x, v.pos.y, v.prev.x, v.prev.y].map(f32::to_bits);
        assert_eq!(ours, theirs, "{what}: vertex {i}");
    }
    let maplines = r32(coll + 8);
    for (i, l) in g.lines.iter().enumerate() {
        let cl = lines + 8 * i as u32;
        let ml = r32(cl);
        assert_eq!(ml, maplines + 0x10 * i as u32, "{what}: line {i}'s record");
        let theirs: [u16; 8] = std::array::from_fn(|k| r16(ml + 2 * k as u32));
        let ours = [
            l.v0,
            l.v1,
            l.prev as u16,
            l.next as u16,
            l.prev_link as u16,
            l.next_link as u16,
            l.hi_flags,
            l.lo_flags,
        ];
        assert_eq!(ours, theirs, "{what}: line {i}");
        assert_eq!(l.flags, r32(cl + 4), "{what}: line {i} flags");
    }
    let descs = r32(coll + 0x24);
    for (j, joint) in g.joints.iter().enumerate() {
        let a = joints + 0x34 * j as u32;
        assert_eq!(
            joint.next,
            joint_index(joints, r32(a)),
            "{what}: joint {j} next"
        );
        assert_eq!(
            r32(a + 4),
            descs + 0x28 * j as u32,
            "{what}: joint {j} descriptor"
        );
        assert_eq!(
            joint.flags,
            r32(a + 8),
            "{what}: joint {j} flags {:#x}",
            joint.flags
        );
        // `xC` is the stage code's stamp, unset at load; only bit 7 of `xE` is a field.
        assert_eq!(
            joint.dirty,
            ctx.read_u8(a + 0xE) & 0x80 != 0,
            "{what}: joint {j} dirty"
        );
        let ours = [
            joint.bounding_min.x,
            joint.bounding_min.y,
            joint.bounding_max.x,
            joint.bounding_max.y,
        ]
        .map(f32::to_bits);
        let theirs: [u32; 4] = std::array::from_fn(|k| r32(a + 0x10 + 4 * k as u32));
        assert_eq!(ours, theirs, "{what}: joint {j} bounds");
        let ours = [
            joint.jobj.unwrap_or(0),
            joint.ledge_callback.unwrap_or(0),
            joint.ledge_user_data.unwrap_or(0),
            joint.callback.unwrap_or(0),
            joint.user_data.unwrap_or(0),
        ];
        let theirs: [u32; 5] = std::array::from_fn(|k| r32(a + 0x20 + 4 * k as u32));
        assert_eq!(ours, theirs, "{what}: joint {j} links");
    }
    assert_eq!(
        (g.list_start, g.list_end),
        (
            joint_index(joints, r32(sym("jointListStart"))),
            joint_index(joints, r32(sym("jointListEnd")))
        ),
        "{what}: joint list"
    );
    assert_eq!(
        g.checked_bounding,
        r32(sym("didCheckBounding")) != 0,
        "{what}: bounding flag"
    );
    let boxes = sym("mpLib_80458868");
    let ours: Vec<u32> = g
        .bounds
        .iter()
        .flat_map(|b| [b.top, b.bottom, b.left, b.right])
        .map(f32::to_bits)
        .collect();
    let theirs: Vec<u32> = (0..8).map(|k| r32(boxes + 4 * k)).collect();
    assert_eq!(ours, theirs, "{what}: boxes");
    assert_eq!(
        g.generation,
        (r32(sym("mpColl_804D64AC")) as i32).wrapping_sub(s.gen0),
        "{what}: update counter"
    );
    // The island lists, walked side by side.
    let heads = sym("mpIsland_80458E88");
    let isl = &g.islands;
    let ours_heads = [
        isl.floors,
        isl.ceilings,
        isl.floors_end,
        isl.ceilings_end,
        isl.dynamic_floors,
        isl.dynamic_ceilings,
        isl.dead_floors,
        isl.dead_ceilings,
        isl.spare,
    ];
    let mut seen = std::collections::HashSet::new();
    let mut pairs = Vec::new();
    for (k, &head) in ours_heads.iter().enumerate() {
        let (mut a, mut o) = (r32(heads + 4 * k as u32), head);
        let mut n = 0;
        loop {
            match (a, o) {
                (0, None) => break,
                (0, Some(_)) | (_, None) => {
                    panic!("{what}: island list {k} differs in length at {n}")
                }
                (addr, Some(id)) => {
                    pairs.push((addr, id, k, n));
                    seen.insert(id.0);
                    a = r32(addr);
                    o = isl.get(id).next;
                    n += 1;
                    assert!(n < 10_000, "{what}: island list {k} loops");
                }
            }
        }
    }
    let arena = isl.arena.len();
    let islands: Vec<(u32, Island)> = pairs.iter().map(|&(a, id, ..)| (a, *isl.get(id))).collect();
    for (&(addr, id, k, n), (_, island)) in pairs.iter().zip(&islands) {
        pair_island(s, addr, id, &what);
        let theirs = [
            u32::from(r16(addr + 4)),
            u32::from(r16(addr + 6)),
            r32(addr + 8),
            r32(addr + 0xC),
            r32(addr + 0x10),
            r32(addr + 0x14),
            r32(addr + 0x18),
            r32(addr + 0x1C),
            r32(addr + 0x20),
            u32::from(r16(addr + 0x24)),
            u32::from(r16(addr + 0x26)),
            u32::from(r16(addr + 0x28)),
            0,
        ];
        assert_eq!(
            island_fields(island),
            theirs,
            "{what}: island {n} of list {k}"
        );
    }
    assert_eq!(seen.len(), arena, "{what}: islands outside every list");
}

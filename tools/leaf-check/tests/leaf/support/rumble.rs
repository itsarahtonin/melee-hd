//! Rumble state on both sides in one raw form, and random rumble requests applied to both:
//! shared by the rumble and input tests.

use std::cell::RefCell;

use leaf_check::{handle, sym};
use melee_data::archive::ArchiveId;
use melee_sim::input::rumble::{Motor, NodeId, Rumble, Script, Table};
use ssbm_rt::{Addr, Ctx};
use ssbm_types::fns;

const NODE_SIZE: u32 = 0x20;
const PORT_SIZE: u32 = 0xC;

/// Where things are in ssbm-rs's memory: the nodes `HSD_PadRumbleInit` was given, and where
/// each archive's data block was copied.
pub struct Places {
    pub nodes: u32,
    pub archives: Vec<(ArchiveId, u32)>,
}

impl Places {
    /// The address ssbm-rs holds for one of our script positions.
    pub fn script(&self, s: Option<Script>) -> u32 {
        match s {
            None => 0,
            Some(Script::Data(r)) => {
                let base = self
                    .archives
                    .iter()
                    .find(|a| a.0 == r.archive)
                    .expect("archive copied into ssbm-rs")
                    .1;
                base + r.offset
            }
            Some(Script::Static {
                table: Table::Vibration,
                index,
            }) => sym("mnVibration_804D4FF0") + 2 * index,
        }
    }

    fn node(&self, n: Option<NodeId>) -> u32 {
        n.map_or(0, |n| self.nodes + NODE_SIZE * u32::from(n.0))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawNode {
    next: u32,
    id: u32,
    pause: u8,
    pri: u8,
    status: u8,
    loop_count: u16,
    wait: u16,
    frame: i32,
    stack: u32,
    listp: u32,
    headp: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawPort {
    last_status: u8,
    status: u8,
    direct_status: u8,
    nb_list: u16,
    list: u32,
}

/// `rumble_info`, `HSD_Rumble_804C22E0` and the nodes, with pointers as ssbm-rs's addresses.
#[derive(Debug, PartialEq, Eq)]
pub struct Raw {
    max_list: u16,
    unk2: u8,
    free: u32,
    ports: Vec<RawPort>,
    nodes: Vec<RawNode>,
}

pub fn ours(r: &Rumble, at: &Places) -> Raw {
    Raw {
        max_list: r.max_list,
        unk2: r.suspended as u8,
        free: at.node(r.free),
        ports: r
            .ports
            .iter()
            .map(|p| RawPort {
                last_status: p.last_status as u8,
                status: p.status as u8,
                direct_status: p.direct_status as u8,
                nb_list: p.nb_list,
                list: at.node(p.list),
            })
            .collect(),
        nodes: r
            .nodes
            .iter()
            .map(|n| RawNode {
                next: at.node(n.next),
                id: n.id,
                pause: n.paused as u8,
                pri: n.pri,
                status: n.status as u8,
                loop_count: n.loop_count,
                wait: n.wait,
                frame: n.frame,
                stack: at.script(n.stack),
                listp: at.script(n.listp),
                headp: at.script(n.headp),
            })
            .collect(),
    }
}

/// ssbm-rs's state, reading as many nodes as `nodes`.
pub fn theirs(ctx: &Ctx, at: &Places, nodes: usize) -> Raw {
    let info = sym("HSD_PadLibData") + 0x2C;
    let ports = sym("HSD_Rumble_804C22E0");
    Raw {
        max_list: ctx.read_u16(info),
        unk2: ctx.read_u8(info + 2),
        free: ctx.read_u32(info + 4),
        ports: (0..4)
            .map(|i| {
                let p = ports + PORT_SIZE * i;
                RawPort {
                    last_status: ctx.read_u8(p),
                    status: ctx.read_u8(p + 1),
                    direct_status: ctx.read_u8(p + 2),
                    nb_list: ctx.read_u16(p + 4),
                    list: ctx.read_u32(p + 8),
                }
            })
            .collect(),
        nodes: (0..nodes as u32)
            .map(|i| {
                let n = at.nodes + NODE_SIZE * i;
                RawNode {
                    next: ctx.read_u32(n),
                    id: ctx.read_u32(n + 4),
                    pause: ctx.read_u8(n + 8),
                    pri: ctx.read_u8(n + 9),
                    status: ctx.read_u8(n + 0xA),
                    loop_count: ctx.read_u16(n + 0xC),
                    wait: ctx.read_u16(n + 0xE),
                    frame: ctx.read_u32(n + 0x10) as i32,
                    stack: ctx.read_u32(n + 0x14),
                    listp: ctx.read_u32(n + 0x18),
                    headp: ctx.read_u32(n + 0x1C),
                }
            })
            .collect(),
    }
}

/// Zeroes `count` nodes at `at`: the game's node array is zero from boot and set up once.
pub fn clear_nodes(ctx: &Ctx, at: u32, count: u32) {
    for a in (0..count * NODE_SIZE).step_by(4) {
        ctx.write_u32(at + a, 0);
    }
}

thread_local! {
    static MOTORS: RefCell<Vec<(i32, u32)>> = const { RefCell::new(Vec::new()) };
}

/// `PADControlMotor`, recording each command.
pub fn stub_control_motor(ctx: &Ctx) {
    let (chan, cmd) = (ctx.regs.r(3) as i32, ctx.regs.r(4));
    MOTORS.with(|m| m.borrow_mut().push((chan, cmd)));
}

/// The commands ssbm-rs sent since the last call.
pub fn take_motor_commands() -> Vec<(i32, u32)> {
    MOTORS.with(|m| std::mem::take(&mut *m.borrow_mut()))
}

/// Our motor changes as the `PADControlMotor` calls the original makes, in port order.
pub fn as_commands(changes: [Option<Motor>; 4]) -> Vec<(i32, u32)> {
    changes
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.map(|m| (i as i32, m.command())))
        .collect()
}

/// A rumble request a game makes.
#[derive(Clone, Copy, Debug)]
pub enum Op {
    Add {
        port: u8,
        id: u32,
        frame: i32,
        pri: u8,
        script: Script,
    },
    Remove(u8),
    RemoveAll,
    RemoveId(u8, u32),
    On(u8),
    Off(u8),
    PauseAll,
    UnpauseAll,
    Suspend(bool),
}

/// A random request: an add of one of `scripts` half the time.
pub fn random_op(rand: &mut dyn FnMut() -> u64, scripts: &[(Script, u8)]) -> Op {
    let port = (rand() % 4) as u8;
    // Ids are small in the game (fighters use 1 and 2 plus a kind); a few distinct ones make
    // RemoveId hit.
    let id = match rand() % 8 {
        0 => rand() as u32,
        k => (k % 4) as u32,
    };
    match rand() % 100 {
        0..=49 => {
            let (script, pri) = scripts[(rand() % scripts.len() as u64) as usize];
            // lb_80014574 passes -2 for "until the script ends" and positive frame counts;
            // the rumble menu 14; -1 runs forever.
            let frame = match rand() % 10 {
                0..=2 => -2,
                3 => -1,
                4 => 14,
                5 => 1,
                _ => 1 + (rand() % 300) as i32,
            };
            // The scripts' own priority usually, any byte sometimes.
            let pri = if rand().is_multiple_of(4) {
                rand() as u8
            } else {
                pri
            };
            Op::Add {
                port,
                id,
                frame,
                pri,
                script,
            }
        }
        50..=57 => Op::Remove(port),
        58..=60 => Op::RemoveAll,
        61..=72 => Op::RemoveId(port, id),
        73..=79 => Op::On(port),
        80..=85 => Op::Off(port),
        86..=88 => Op::PauseAll,
        89..=95 => Op::UnpauseAll,
        // lb_0192 passes 1 and 0.
        _ => Op::Suspend(rand().is_multiple_of(3)),
    }
}

/// Applies `op` to both sides; for an add, both must report the same success.
pub fn apply(ctx: &Ctx, ours: &mut Rumble, at: &Places, op: Op, what: &dyn Fn() -> String) {
    match op {
        Op::Add {
            port,
            id,
            frame,
            pri,
            script,
        } => {
            let theirs = fns::HSD_PadRumbleAdd(
                ctx,
                port,
                id as i32,
                frame,
                i32::from(pri),
                handle::<Addr>(ctx, at.script(Some(script))),
            );
            let mine = ours.add(port, id, frame, pri, script);
            assert_eq!(
                mine,
                theirs != 0,
                "HSD_PadRumbleAdd's result after {}",
                what()
            );
        }
        Op::Remove(port) => {
            fns::HSD_PadRumbleRemove(ctx, port);
            ours.remove(port);
        }
        Op::RemoveAll => {
            fns::HSD_PadRumbleRemoveAll(ctx);
            ours.remove_all();
        }
        Op::RemoveId(port, id) => {
            fns::HSD_PadRumbleRemoveId(ctx, port, id as i32);
            ours.remove_id(port, id);
        }
        Op::On(port) => {
            fns::HSD_PadRumbleOn(ctx, port);
            ours.on(port);
        }
        Op::Off(port) => {
            fns::HSD_PadRumbleOffN(ctx, port);
            ours.off(port);
        }
        Op::PauseAll => {
            fns::HSD_PadRumblePauseAll(ctx);
            ours.pause_all();
        }
        Op::UnpauseAll => {
            fns::HSD_PadRumbleUnpauseAll(ctx);
            ours.unpause_all();
        }
        Op::Suspend(on) => {
            fns::HSD_Rumble_80378524(ctx, on as i32);
            ours.suspend(on);
        }
    }
}

//! melee-sim's rumble against ssbm-rs's port of HAL's, on random sequences of requests and
//! frames on all four ports: the full state (ports, the free list, every node, stale fields
//! included) after every step, and every `PADControlMotor` command each frame sends.
//!
//! Scripts are every entry of the disc's `lbRumbleData` (`LbRb.dat`), the rumble menu's
//! static pulse, and random well-formed scripts that reach every opcode, loop counts and
//! zero-length commands. Rumble allocates from its own node array, not HAL's allocators, so
//! the plain reference is enough.

#[path = "support/rumble.rs"]
mod support;

use leaf_check::{SCRATCH, handle, reference, sym};
use melee_data::archive::{Archive, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::input::rumble::{Rumble, Script, Table};
use ssbm_rt::Addr;
use ssbm_types::fns;
use support::{Op, Places};

/// Where the archives' data blocks are copied in ssbm-rs's memory, and nodes other than
/// the game's array.
const NODES: u32 = SCRATCH;
const LBRB: u32 = SCRATCH + 0x1000;
const SYNTHETIC: u32 = SCRATCH + 0x4_0000;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A DAT file holding `data` with no relocations or symbols.
fn dat(data: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    for w in [
        0x20 + data.len() as u32,
        data.len() as u32,
        0,
        0,
        0,
        0,
        0,
        0,
    ] {
        f.extend_from_slice(&w.to_be_bytes());
    }
    f.extend_from_slice(data);
    f
}

fn command(op: u16, frame: u16) -> u16 {
    op << 13 | frame
}

/// A script the interpreter can run: every cycle through it, and every loop body, has a
/// command that waits at least a frame (the original spins forever on one that doesn't),
/// loop ends follow loop starts (the original jumps through a null pointer otherwise), and
/// only opcodes 0 to 5 appear (the original spins forever on 6 and 7).
fn random_script(rng: &mut Rng) -> Vec<u16> {
    let wait = |rng: &mut Rng, nonzero: bool| {
        let op = 1 + rng.below(3) as u16;
        let frame = match rng.below(10) {
            0 if !nonzero => 0,
            1 => 0x1FFF,
            _ => 1 + rng.below(40) as u16,
        };
        command(op, frame)
    };
    let mut v = Vec::new();
    for seg in 0..1 + rng.below(4) {
        if rng.below(10) < 3 {
            // A loop count of 0 wraps to 65535 passes.
            let count = if rng.below(20) == 0 {
                0
            } else {
                1 + rng.below(5) as u16
            };
            v.push(command(4, count));
            let body = 1 + rng.below(3);
            let long = rng.below(body);
            for k in 0..body {
                v.push(wait(rng, k == long));
            }
            v.push(command(5, rng.below(0x2000) as u16));
        } else {
            v.push(wait(rng, seg == 0));
        }
    }
    // The end's frame field is ignored.
    v.push(command(0, rng.below(4) as u16));
    v
}

#[test]
fn rumble_sequences_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    ctx.register(sym("PADControlMotor"), support::stub_control_motor);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let lbrb = Archive::parse(&disc.read_file("LbRb.dat").unwrap()).unwrap();
    let table = lbrb.public("lbRumbleData").expect("lbRumbleData");

    let mut rng = Rng(0x5EED_0F0B_5E55_ED00);
    let mut syn_data = Vec::new();
    let mut syn_scripts = Vec::new();
    for _ in 0..64 {
        syn_scripts.push(syn_data.len() as u32);
        for c in random_script(&mut rng) {
            syn_data.extend_from_slice(&c.to_be_bytes());
        }
    }
    let syn = Archive::parse(&dat(&syn_data)).unwrap();

    for (base, a) in [(LBRB, &lbrb), (SYNTHETIC, &syn)] {
        for (i, b) in a.data().iter().enumerate() {
            ctx.write_u8(base + i as u32, *b);
        }
    }
    let mut data = Archives::default();
    let lbrb_id = data.add(lbrb.clone());
    let syn_id = data.add(syn);
    let mut places = Places {
        nodes: 0,
        archives: vec![(lbrb_id, LBRB), (syn_id, SYNTHETIC)],
    };

    // `lbRumbleData`: a script pointer and a priority byte per 8-byte entry, as lb_80014574
    // reads them.
    let mut scripts = Vec::new();
    let mut entry = table;
    while let Some(script) = lbrb.ptr(entry) {
        let pri = lbrb.u8(entry + 4);
        scripts.push((
            Script::Data(DataRef {
                archive: lbrb_id,
                offset: script,
            }),
            pri,
        ));
        entry += 8;
    }
    let game_scripts = scripts.len();
    // The game's scripts are well formed, which the interpreter's panics rely on: opcodes 0
    // to 5 only, and every loop end after a loop start.
    for (script, _) in &scripts {
        let Script::Data(mut at) = *script else {
            unreachable!()
        };
        let mut looped = false;
        loop {
            let c = data.u16(at);
            match c >> 13 {
                0 => break,
                4 => looped = true,
                5 => assert!(looped, "loop end before a loop start at {at:?}"),
                6 | 7 => panic!("command {c:#06x} at {at:?}"),
                _ => {}
            }
            at = at.at(2);
        }
    }
    assert!(game_scripts > 0, "no lbRumbleData entries");
    scripts.push((Script::table(Table::Vibration), 0));
    for &offset in &syn_scripts {
        let pri = rng.below(8) as u8;
        let script = Script::Data(DataRef {
            archive: syn_id,
            offset,
        });
        scripts.push((script, pri));
    }

    let game_nodes = sym("gmMain_8046B1F8");
    let (mut steps, mut frames) = (0u64, 0u64);
    let mut commands = [0u64; 3];
    let (mut rejected, mut ended, mut looping, mut paused, mut suspended) = (0u64, 0, 0, 0, 0);
    for seq in 0..300u32 {
        // Melee sets up twelve nodes in gmMain_8046B1F8; fewer make adds fail sooner. With no
        // nodes, the list pointer is null, as ours has none to point at.
        let nb_list: u16 = match seq % 5 {
            0..=2 => 12,
            3 => 1 + rng.below(12) as u16,
            _ => 0,
        };
        places.nodes = if nb_list == 12 { game_nodes } else { NODES };
        support::clear_nodes(ctx, places.nodes, 12);
        let list = if nb_list == 0 { 0 } else { places.nodes };
        fns::HSD_PadRumbleInit(ctx, nb_list, handle::<Addr>(ctx, list));
        let mut ours = Rumble::new(nb_list);
        let compare = |ours: &Rumble, what: &dyn Fn() -> String| {
            assert_eq!(
                support::ours(ours, &places),
                support::theirs(ctx, &places, ours.nodes.len()),
                "after {}",
                what()
            );
        };
        compare(&ours, &|| format!("init, sequence {seq}"));
        let length = 200 + rng.below(1500);
        for step in 0..length {
            let what = |name: &str| format!("{name}, sequence {seq} step {step}");
            if rng.below(100) < 45 {
                for n in &ours.nodes {
                    looping += u64::from(n.loop_count > 1);
                }
                let live: u16 = ours.ports.iter().map(|p| p.nb_list).sum();
                paused += ours.nodes.iter().filter(|n| n.paused).count() as u64;
                suspended += u64::from(ours.suspended);
                fns::HSD_PadRumbleInterpret(ctx);
                let changes = ours.interpret(&data);
                let theirs = support::take_motor_commands();
                assert_eq!(
                    support::as_commands(changes),
                    theirs,
                    "{}",
                    what("motor commands")
                );
                for m in changes.into_iter().flatten() {
                    commands[m as usize] += 1;
                }
                ended += u64::from(live - ours.ports.iter().map(|p| p.nb_list).sum::<u16>());
                frames += 1;
                compare(&ours, &|| what("Interpret"));
            } else {
                let mut rand = || rng.next();
                let op = support::random_op(&mut rand, &scripts);
                let before = ours.free;
                support::apply(ctx, &mut ours, &places, op, &|| what(&format!("{op:?}")));
                rejected += u64::from(matches!(op, Op::Add { .. }) && before == ours.free);
                compare(&ours, &|| what(&format!("{op:?}")));
            }
            steps += 1;
        }
    }
    let [hard, stop, rumble] = commands;
    eprintln!(
        "rumble: {game_scripts} lbRumbleData scripts; {steps} steps, {frames} frames; motor \
         commands: {hard} hard stops, {stop} stops, {rumble} rumbles; adds rejected {rejected}, \
         scripts ended {ended}, looping node-frames {looping}, paused node-frames {paused}, \
         suspended frames {suspended}"
    );
    assert!(hard > 0 && stop > 0 && rumble > 0 && rejected > 0 && ended > 0 && looping > 0);
}

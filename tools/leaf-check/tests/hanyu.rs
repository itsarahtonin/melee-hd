//! The debug menu's Hanyu character select's entry (`gm_801BED3C`, the select mode's state
//! enter) against ssbm-rs's port. The select crashes as it opens (`mnCharSel_8025D1C4` reads
//! its KO counts through a null pointer, docs/melee-bugs.md), in the same tick its entry
//! runs, so no session's trace holds the entry's writes: this checks them. Random VS data and
//! unlock bits, and a random select block whose match type is even (its only value on a
//! console is 0: the odd types' relock, `gm_80164A0C`, is in `ledger/dead.txt`).

use leaf_check::{Rand, SCRATCH, hsd_reference, sym};
use melee_data::archive::{Archives, DataRef};
use melee_sim::game::gm::vs_melee::VS_MODE_MELEE;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Running};
use melee_sim::game::structs::VsModeData;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

#[derive(Default)]
struct Diffs(Vec<String>);

impl Cx for Diffs {
    fn import_handle(&mut self, _: &'static str, addr: u32) -> Option<u32> {
        (addr != 0).then_some(addr)
    }
    fn import_data(&mut self, _: u32) -> Option<DataRef> {
        None
    }
    fn compare_handle(&mut self, path: &str, _: &'static str, ours: Option<u32>, theirs: u32) {
        if ours.unwrap_or(0) != theirs {
            self.0.push(format!("{path}: {ours:?} vs {theirs:#x}"));
        }
    }
    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {}
    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.0.push(format!("{path}: {ours} vs {theirs}"));
    }
}

fn read(ctx: &Ctx, at: u32, len: u32) -> Vec<u8> {
    let mut b = vec![0; len as usize];
    let _ = ctx.mem.read_bytes(at, &mut b);
    b
}

/// Random VS data, its start data's pointers (the hooks) zeroed: nothing here reads them.
fn vs_bytes(rand: &mut Rand) -> Vec<u8> {
    let mut b: Vec<u8> = (0..VsModeData::SIZE).map(|_| rand.bits() as u8).collect();
    b[8 + 0x38..8 + 0x60].fill(0);
    b
}

fn compare(ours: &VsModeData, after: &[u8], at: u32, what: &str) {
    let mut d = Diffs::default();
    ours.compare(
        &Mem {
            bytes: after,
            base: at,
        },
        at,
        what,
        &mut d,
    );
    assert!(d.0.is_empty(), "{what}: {:?}", d.0);
}

#[test]
fn hanyu_select_entry() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    // The card data's pointer, as the boot sets it.
    let ptr = sym("gmMainLib_804D3EE0");
    if ctx.read_u32(ptr) == 0 {
        ctx.write_u32(ptr, sym("gmMainLib_8045A6C0"));
    }
    // The select's block (`gm_8049BEE8`) is the mode state's enter data.
    let css_at = SCRATCH;
    ctx.register(sym("gm_GetGameModeStateEnterData"), |ctx| {
        ctx.regs.set_r(3, SCRATCH)
    });
    let vs_at = Handle::addr(fns::gmVsMelee_GetVsData(ctx));
    let unlocks_at = Handle::addr(fns::gmMainLib_GetUnlockedCharactersBitmaskPtr(ctx));
    let data = Archives::default();
    let mut rand = Rand(0x4A4E_5955_0001_u64);
    for n in 0..1000 {
        let vs = vs_bytes(&mut rand);
        let css = {
            let mut b = vec![
                rand.bits() as u8,
                rand.bits() as u8,
                rand.below(12) as u8 * 2,
                rand.bits() as u8,
                0,
                0,
                0,
                0,
            ];
            b.extend(vs_bytes(&mut rand));
            b
        };
        let unlocks = rand.bits() as u16;
        let mem = |b: &[u8], at: u32| -> VsModeData {
            VsModeData::import(&Mem { bytes: b, base: at }, at, &mut Diffs::default())
        };
        let mut game = Game::default();
        game.game_data.modes.table[VS_MODE_MELEE] = mem(&vs, vs_at);
        game.game_data.thing.save_data.unlocked_characters = unlocks;
        let h = &mut game.debug_mode.hanyu_css;
        h.unk_0x0 = u16::from_be_bytes([css[0], css[1]]);
        h.match_type = css[2];
        h.pending_scene_change = css[3];
        h.vs = mem(&css[8..], css_at + 8);
        let mut r = Running {
            game: &mut game,
            data: &data,
        };
        r.hanyu_css_enter();
        ctx.write_bytes(vs_at, &vs);
        ctx.write_bytes(css_at, &css);
        ctx.write_u16(unlocks_at, unlocks);
        // `gm_801BED3C` is local to its unit, so it's called by its address (the state is
        // read only through the enter data's getter, stood in above).
        let _: () = ctx.call(sym("gm_801BED3C"), (0x8000_0000_u32,));
        let after = read(ctx, css_at, 8 + VsModeData::SIZE);
        let h = &game.debug_mode.hanyu_css;
        assert_eq!(
            (h.unk_0x0, h.match_type, h.pending_scene_change),
            (u16::from_be_bytes([after[0], after[1]]), after[2], after[3]),
            "gm_801BED3C {n}: the select's header"
        );
        compare(
            &h.vs,
            &after[8..],
            css_at + 8,
            &format!("gm_801BED3C {n}: the select's VS data"),
        );
        let table = read(ctx, vs_at, VsModeData::SIZE);
        compare(
            &game.game_data.modes.table[VS_MODE_MELEE],
            &table,
            vs_at,
            &format!("gm_801BED3C {n}: VS mode's data"),
        );
        assert_eq!(
            game.game_data.thing.save_data.unlocked_characters,
            ctx.read_u16(unlocks_at),
            "gm_801BED3C {n}: the unlocked characters"
        );
    }
}

//! Fountain of Dreams' state outside its objects' variables: the reflection camera's user data
//! (`IzumiReflection`, which the original allocates with `HSD_MemAlloc` and frees with
//! `HSD_Free`), and the stage archive's image descriptor it rewrites.

use melee_sim::game::gr::izumi::{RENDER_REFLECTION, Reflection};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::UserData;
use melee_sim::game::structs::HSD_ImageDesc;
use melee_sim::mtx::Mtx34;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::Importer;

fn w(mem: &[u8], a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(mem[i..i + 4].try_into().unwrap())
}

fn matrix(mem: &[u8], a: u32) -> Mtx34 {
    let mut m = Mtx34::default();
    for r in 0..3 {
        for c in 0..4 {
            m.m[r][c] = f32::from_bits(w(mem, a + 4 * (4 * r + c) as u32));
        }
    }
    m
}

/// Whether a GObj's render callback makes it the reflection camera.
pub fn is_reflection(render: u32) -> bool {
    render == RENDER_REFLECTION
}

/// Imports the reflection camera's user data at `ud`. A capture before its first render holds
/// the block's leftover matrix, which no code reads before the render writes it, so the import
/// takes the matrix as written.
pub(crate) fn import_reflection(imp: &mut Importer, ud: u32) -> UserData {
    let m: &[u8] = &imp.cap.call.mem1;
    let texture_matrix = Some(matrix(m, ud));
    let p = w(m, ud + 0x30);
    let image = imp.data_or_unhandled(p);
    let desc = if p != 0 {
        let mem = Mem {
            bytes: &imp.cap.call.mem1,
            base: MEM1,
        };
        HSD_ImageDesc::import(&mem, p, imp)
    } else {
        HSD_ImageDesc::default()
    };
    imp.game.gr.izumi.reflection = Some(Reflection {
        texture_matrix,
        image,
        desc,
        addr: ud,
        buffer: 0,
        buffer_len: 0,
    });
    UserData::IzumiReflection
}

/// Compares the reflection camera's user data with memory (the GObj at `t`): its release
/// function, its texture matrix once written, its image's descriptor and what the stage wrote
/// there.
pub(crate) fn compare_reflection(cmp: &mut Comparer, path: &str, t: u32) {
    let mem: &[u8] = cmp.mem;
    let ud = w(mem, t + 0x2C);
    cmp.check(
        path,
        "reflection's release",
        crate::stadium::HSD_FREE,
        w(mem, t + 0x30),
    );
    let Some(v) = cmp.game.gr.izumi.reflection.as_ref() else {
        cmp.diff(path, "reflection user data", "none", format!("{ud:#x}"));
        return;
    };
    let (tm, image, desc) = (v.texture_matrix, v.image, v.desc.clone());
    if let Some(m) = tm {
        let theirs = matrix(mem, ud);
        let bits = |m: &Mtx34| m.m.map(|r| r.map(f32::to_bits));
        cmp.check(path, "reflection's texture matrix", bits(&m), bits(&theirs));
    }
    let p = w(mem, ud + 0x30);
    cmp.compare_data(&format!("{path} reflection image"), image, p);
    if p != 0 {
        let m = Mem {
            bytes: cmp.mem,
            base: MEM1,
        };
        desc.compare(&m, p, &format!("{path} reflection image desc"), cmp);
    }
}

//! Dispatch of the original's function pointers. A [`CodeRef`] is the original function's
//! address; each table here maps the addresses a kind of pointer can hold to their ports.

use super::handles::CodeRef;
use super::state::Running;
use crate::hsd::gobj::GObjId;

/// Runs a GObj process (`HSD_GObjEvent`).
pub fn run_proc(game: &mut Running, gobj: GObjId, proc: CodeRef) {
    let _ = (&game, gobj);
    #[allow(clippy::match_single_binding)]
    match proc.0 {
        other => unported("process", other),
    }
}

/// The original reaches a function no port covers yet: stopping beats diverging.
fn unported(what: &str, addr: u32) -> ! {
    panic!("{what} {addr:#010x} isn't ported")
}

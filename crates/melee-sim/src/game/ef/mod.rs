//! The game's effects (`melee/ef`): models that play an animation on a GObj of their own for a
//! while, following a joint or standing where they were spawned (efLib's effects), and
//! particle generators spawned from the effect banks, with the glue between the particle system
//! and the game (efLib's particle callbacks and processes).
//!
//! Effects are spawned by id: `efSync_Spawn` ([`spawn`]) picks how from the id's range, through
//! efAsync's and efAlt's tables of per-effect cases; `efAsync_Spawn` ([`queue`]) defers a spawn
//! made during a fighter's or item's process to when its owner flushes its queue. An id's
//! thousands are its effect bank: bank 0 is `EfCoData.dat`, the common effects, and each
//! character's file has its own (`efAsync_DatEntries`).
//!
//! Every effect is an [`Effect`] in a pool (`efLib_AllocData`), the user data of its GObj
//! (class 8, process link 11 or 12, render link 7). The effect files' tables are loaded data;
//! which are loaded is state ([`Globals::tables`]).

mod alt;
mod dispatch;
pub mod eflib;
mod sync;

use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::handles::FighterId;
use super::state::{Game, Running};
use crate::hsd::generator::GeneratorId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;
use crate::vec::Vec3;

pub use alt::spawn_alt;
pub use dispatch::{dispatch, flush, process_deferred, queue, queue_clear, queue_with};
pub use eflib::*;
pub use sync::{spawn, spawn_with};

pub use super::handles::EffectId;

/// A deferred spawn's slot (`EF_QueuedEffect*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueuedId(pub u32);

/// The GObj class of effects (`HSD_GOBJ_CLASS_EFFECT`).
pub const CLASS: u16 = 8;
/// Effects' process links: most effects', and the one ids 25 and 26 use.
pub const P_LINK: u8 = 11;
pub const P_LINK_AUX: u8 = 12;
/// The user data kind of an effect.
pub const USER_DATA_KIND: u8 = 8;

/// `expire_flags`: the effect's owner is going away.
pub const EXPIRE: i8 = 1;
/// `scale_flags`: the effect takes its joint's scale each frame.
pub const SCALE_INHERIT: u8 = 1;
/// `state_flags`: pausing (the next update pauses), paused, and a mark efAsync's table sets
/// on some effects, which pausing and resuming keep.
pub const STATE_ENTERING_PAUSE: u8 = 1;
pub const STATE_PAUSED: u8 = 2;
pub const STATE_ASYNC: u8 = 0x80;

/// `efLib_LoadKind`: whether the effect being made counts towards the 64 efLib keeps at
/// most (0, which the original names async) or not (1).
pub const LOAD_COUNTED: u32 = 0;
pub const LOAD_UNCOUNTED: u32 = 1;

/// The most counted effects at once before the oldest are destroyed.
pub const MAX_COUNTED: u32 = 64;

/// What an effect's update runs each frame after its animation (`EF_UpdateFn`), by the
/// original function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Update {
    SetOffsetYFromParamY,
    SetScaleFromParamX,
    SetRotYAndTransition,
    SetJObjOffsetZ,
    SetRotYFromFighterDir,
    SetRotYZFromFighter,
    FallFromParamY,
    SetOffsetFromParams,
    LifetimeEndSpawn,
    SetScaleRotYFromFighter,
    SetRotYZFromParamZFighterDir,
    MarioSpecialLw,
    LuigiSpecialLw,
    KoopaSpecialHi,
    Bury,
    ApplyStoredAlpha,
    AccumOffsetFromParams,
}

impl Update {
    pub const ALL: [Update; 17] = [
        Update::SetOffsetYFromParamY,
        Update::SetScaleFromParamX,
        Update::SetRotYAndTransition,
        Update::SetJObjOffsetZ,
        Update::SetRotYFromFighterDir,
        Update::SetRotYZFromFighter,
        Update::FallFromParamY,
        Update::SetOffsetFromParams,
        Update::LifetimeEndSpawn,
        Update::SetScaleRotYFromFighter,
        Update::SetRotYZFromParamZFighterDir,
        Update::MarioSpecialLw,
        Update::LuigiSpecialLw,
        Update::KoopaSpecialHi,
        Update::Bury,
        Update::ApplyStoredAlpha,
        Update::AccumOffsetFromParams,
    ];

    /// The original function's address.
    pub fn addr(self) -> u32 {
        match self {
            Update::SetOffsetYFromParamY => 0x8005_DDB8,
            Update::SetScaleFromParamX => 0x8005_DE94,
            Update::SetRotYAndTransition => 0x8005_E090,
            Update::SetJObjOffsetZ => 0x8005_E1D8,
            Update::SetRotYFromFighterDir => 0x8005_E2B4,
            Update::SetRotYZFromFighter => 0x8005_E3A0,
            Update::FallFromParamY => 0x8005_E648,
            Update::SetOffsetFromParams => 0x8005_E950,
            Update::LifetimeEndSpawn => 0x8005_EB70,
            Update::SetScaleRotYFromFighter => 0x8005_EBC8,
            Update::SetRotYZFromParamZFighterDir => 0x8005_EDDC,
            Update::MarioSpecialLw => 0x8005_F08C,
            Update::LuigiSpecialLw => 0x8005_F270,
            Update::KoopaSpecialHi => 0x8005_F454,
            Update::Bury => 0x8005_F748,
            Update::ApplyStoredAlpha => 0x8005_FAB8,
            Update::AccumOffsetFromParams => 0x8005_FBE4,
        }
    }

    pub fn from_addr(addr: u32) -> Option<Update> {
        Update::ALL.into_iter().find(|u| u.addr() == addr)
    }

    /// Which of the effect's `params` this update reads: x, y and z.
    pub fn reads_params(self) -> [bool; 3] {
        match self {
            Update::SetOffsetYFromParamY | Update::FallFromParamY => [false, true, false],
            Update::SetScaleFromParamX => [true, false, false],
            Update::SetOffsetFromParams | Update::AccumOffsetFromParams => [true; 3],
            Update::SetRotYZFromParamZFighterDir => [false, false, true],
            _ => [false; 3],
        }
    }
}

/// `EF_Effect`. `efLib_Create` leaves `params` and `user_data` as the pool's slot had them;
/// each spawn sets what its update reads before the update runs, and nothing else reads them,
/// so the slot's leftover words are no state: a new effect starts them at zero, and the oracle
/// compares the parts its update reads (`x2A` and `x2B`, which nothing reads, aren't kept).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effect {
    /// The next effect of a spawn that made several (`next`).
    pub next: Option<EffectId>,
    pub gobj: Option<GObjId>,
    /// The GObj that spawned it (`parent_gobj`): pausing, expiring and destroying go by it.
    pub parent: Option<GObjId>,
    pub attach_jobj: Option<JObjId>,
    pub update: Option<Update>,
    /// The fighter whose facing `efLib_Cb_SetRotYAndTransition` reads (`user_data`, which it
    /// reads as a joint's x scale: a fighter's facing is at that offset).
    pub user_data: Option<FighterId>,
    pub params: Vec3,
    /// Frames left, counting down to 0 (`lifetime`); 0 lives until destroyed.
    pub lifetime: u16,
    pub expire_flags: i8,
    pub scale_flags: u8,
    pub state_flags: u8,
    /// `efLib_LoadKind` when it was made: 0 counts towards the 64 (`is_async`).
    pub is_async: u8,
}

/// How a deferred spawn finds where to spawn (`EF_SpawnKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    /// On the joint.
    Attach = 0,
    /// At the joint's position.
    Pos = 1,
    /// At an offset in the joint's space.
    PosOffset = 2,
    /// On the joint, with a float.
    AttachParam = 3,
    /// At the joint's position, with a float.
    PosParam = 4,
    /// At an offset, with a float.
    PosOffsetParam = 5,
    /// At an offset, with two floats.
    PosOffsetParam2 = 6,
    /// On the joint, with a vector.
    AttachOffset = 7,
    /// A camera quake at an offset in the joint's space (the id is the quake's kind).
    CameraShake = 8,
}

impl SpawnKind {
    pub fn from_u8(k: u8) -> Option<SpawnKind> {
        use SpawnKind::*;
        Some(match k {
            0 => Attach,
            1 => Pos,
            2 => PosOffset,
            3 => AttachParam,
            4 => PosParam,
            5 => PosOffsetParam,
            6 => PosOffsetParam2,
            7 => AttachOffset,
            8 => CameraShake,
            _ => return None,
        })
    }
}

/// `EF_QueuedEffect`: a spawn `efAsync_Spawn` deferred. Its words the kind doesn't use are
/// never read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queued {
    pub next: Option<QueuedId>,
    pub kind: u8,
    pub gfx_id: i32,
    pub jobj: Option<JObjId>,
    pub params: Vec3,
    pub extra1: f32,
    pub extra2: f32,
}

/// `EF_ParamEntry` in `efLib_ParamTable`: a GObj's stored alpha and effect id, which
/// `efLib_Cb_ApplyStoredAlpha` applies to its effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParamEntry {
    pub gobj: Option<GObjId>,
    pub gfx_id: u16,
    pub alpha: u16,
}

/// The effect files (`efAsync_DatEntries`): file name and data table name, by bank.
pub const FILES: [Option<(&str, &str)>; 51] = {
    let mut f: [Option<(&str, &str)>; 51] = [None; 51];
    f[0] = Some(("EfCoData.dat", "effCommonDataTable"));
    f[1] = Some(("EfMrData.dat", "effMarioDataTable"));
    f[2] = Some(("EfSsData.dat", "effSamusDataTable"));
    f[3] = Some(("EfFxData.dat", "effFoxDataTable"));
    f[4] = Some(("EfCaData.dat", "effCaptainDataTable"));
    f[5] = Some(("EfKbData.dat", "effKirbyDataTable"));
    f[6] = Some(("EfLkData.dat", "effLinkDataTable"));
    f[7] = Some(("EfPkData.dat", "effPikachuDataTable"));
    f[8] = Some(("EfDkData.dat", "effDonkeyDataTable"));
    f[9] = Some(("EfYsData.dat", "effYoshiDataTable"));
    f[10] = Some(("EfNsData.dat", "effNessDataTable"));
    f[11] = Some(("EfPrData.dat", "effPurinDataTable"));
    f[12] = Some(("EfKpData.dat", "effKoopaDataTable"));
    f[13] = Some(("EfMtData.dat", "effMewtwoDataTable"));
    f[14] = Some(("EfIcData.dat", "effIceclimberDataTable"));
    f[15] = Some(("EfPeData.dat", "effPeachDataTable"));
    f[16] = Some(("EfMsData.dat", "effMarsDataTable"));
    f[17] = Some(("EfZdData.dat", "effZeldaDataTable"));
    f[18] = Some(("EfLgData.dat", "effLuigiDataTable"));
    f[19] = Some(("EfGnData.dat", "effGanonDataTable"));
    f[20] = Some(("EfKbMs.dat", "effKirbyMarsDataTable"));
    f[21] = Some(("EfKbZd.dat", "effKirbyZeldaDataTable"));
    f[31] = Some(("EfMnData.dat", "effMenuDataTable"));
    f[32] = Some(("EfKbMr.dat", "effKirbyMarioDataTable"));
    f[33] = Some(("EfKbFx.dat", "effKirbyFoxDataTable"));
    f[34] = Some(("EfKbSs.dat", "effKirbySamusDataTable"));
    f[36] = Some(("EfKbPk.dat", "effKirbyPikachuDataTable"));
    f[37] = Some(("EfKbLg.dat", "effKirbyLuigiDataTable"));
    f[38] = Some(("EfKbCa.dat", "effKirbyCaptainDataTable"));
    f[39] = Some(("EfKbDk.dat", "effKirbyDonkeyDataTable"));
    f[41] = Some(("EfKbKp.dat", "effKirbyKoopaDataTable"));
    f[46] = Some(("EfKbIc.dat", "effKirbyIceDataTable"));
    f[47] = Some(("EfKbGn.dat", "effKirbyGanonDataTable"));
    f[48] = Some(("EfKbFe.dat", "effKirbyEmblemDataTable"));
    f[49] = Some(("EfFeData.dat", "effEmblemDataTable"));
    f
};

/// efLib's and efAsync's globals.
#[derive(Clone, Debug)]
pub struct Globals {
    /// The effects (`efLib_AllocData`).
    pub effects: Pool<Effect>,
    /// Deferred spawns (`efAsync_AllocData`).
    pub queued: Pool<Queued>,
    /// Counted effects alive (`efLib_EffectCount`).
    pub count: u32,
    /// `efLib_LoadKind`: [`LOAD_COUNTED`] or [`LOAD_UNCOUNTED`].
    pub load_kind: u32,
    /// Joints made with animations since the last spawn began, which it animates once before
    /// it returns (`efLib_AnimQueue` as 32 joint pointers, and `efLib_AnimCount`). Entries past
    /// the count are leftovers nothing reads.
    pub anim_queue: [Option<JObjId>; 32],
    pub anim_count: i32,
    /// GObjs' stored alpha and effect ids (`efLib_ParamTable`).
    pub params: [ParamEntry; 8],
    /// Each loaded effect file's effect descriptors (`efAsync_DatEntries[i].data`: the data
    /// table's descriptors, after its two bank pointers).
    pub tables: [Option<DataRef>; 51],
    /// The effect files the frontend loaded, by bank: disc data, not state. A sync load
    /// (`efAsync_LoadSync`) takes its file from here.
    pub files: [Option<ArchiveId>; 51],
}

impl Default for Globals {
    fn default() -> Self {
        Globals {
            effects: Pool::default(),
            queued: Pool::default(),
            count: 0,
            load_kind: 0,
            anim_queue: [None; 32],
            anim_count: 0,
            params: [ParamEntry::default(); 8],
            tables: [None; 51],
            files: [None; 51],
        }
    }
}

impl Game {
    /// The effect a GObj's user data holds.
    pub fn effect_id(&self, gobj: GObjId) -> EffectId {
        match self.world.gobjs[gobj.0].user_data {
            Some(super::state::UserData::Effect(e)) => e,
            other => panic!("GObj {gobj:?} holds {other:?}, not an effect"),
        }
    }

    pub fn effect(&self, e: EffectId) -> &Effect {
        &self.ef.effects[e.0]
    }

    pub fn effect_mut(&mut self, e: EffectId) -> &mut Effect {
        &mut self.ef.effects[e.0]
    }
}

/// What a spawn returns (`efSync_Spawn`'s `void*`): an effect, a generator, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spawned {
    None,
    Effect(EffectId),
    Generator(GeneratorId),
}

impl Spawned {
    pub fn from_effect(e: Option<EffectId>) -> Spawned {
        e.map_or(Spawned::None, Spawned::Effect)
    }

    pub fn from_generator(g: Option<GeneratorId>) -> Spawned {
        g.map_or(Spawned::None, Spawned::Generator)
    }

    pub fn effect(self) -> Option<EffectId> {
        match self {
            Spawned::Effect(e) => Some(e),
            Spawned::None => None,
            Spawned::Generator(g) => panic!("expected an effect, got generator {g:?}"),
        }
    }

    pub fn generator(self) -> Option<GeneratorId> {
        match self {
            Spawned::Generator(g) => Some(g),
            Spawned::None => None,
            Spawned::Effect(e) => panic!("expected a generator, got effect {e:?}"),
        }
    }
}

/// One of a spawn's variadic arguments: a joint, a vector or a float (pointers, given as the
/// pointee, which the spawn reads once), a word (a color), or a deferred spawn's `params`,
/// which the spawn reads as a vector or as the float its first word is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg {
    JObj(Option<JObjId>),
    Vec(Vec3),
    Float(f32),
    Int(u32),
    Params(Vec3),
}

/// What a spawn reads its next variadic argument as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    JObj,
    Vec,
    Float,
    Int,
}

/// Where a spawn's arguments come from: a list, or a function that gives the `i`th as the
/// spawn asks for it (a caller holding the original's registers decodes them only then, as
/// `va_arg` does).
enum Source<'a> {
    List(&'a [Arg]),
    Lazy(&'a mut dyn FnMut(usize, ArgKind) -> Arg),
}

/// A spawn's variadic arguments (`va_list`), read in order as the cases ask for them.
pub struct Args<'a> {
    src: Source<'a>,
    next: usize,
}

impl<'a> Args<'a> {
    pub fn new(args: &'a [Arg]) -> Self {
        Args {
            src: Source::List(args),
            next: 0,
        }
    }

    /// Arguments `get` gives as they're read.
    pub fn lazy(get: &'a mut dyn FnMut(usize, ArgKind) -> Arg) -> Self {
        Args {
            src: Source::Lazy(get),
            next: 0,
        }
    }

    fn take(&mut self, kind: ArgKind) -> Arg {
        let i = self.next;
        self.next += 1;
        match &mut self.src {
            Source::List(args) => *args
                .get(i)
                .unwrap_or_else(|| panic!("va_arg past the {} arguments passed", args.len())),
            Source::Lazy(get) => get(i, kind),
        }
    }

    /// `va_arg(vlist, HSD_JObj*)`.
    pub fn jobj(&mut self) -> Option<JObjId> {
        match self.take(ArgKind::JObj) {
            Arg::JObj(j) => j,
            other => panic!("va_arg wants a joint, the caller passed {other:?}"),
        }
    }

    /// `*va_arg(vlist, Vec3*)`.
    pub fn vec(&mut self) -> Vec3 {
        match self.take(ArgKind::Vec) {
            Arg::Vec(v) | Arg::Params(v) => v,
            other => panic!("va_arg wants a vector, the caller passed {other:?}"),
        }
    }

    /// `*va_arg(vlist, f32*)`.
    pub fn float(&mut self) -> f32 {
        match self.take(ArgKind::Float) {
            Arg::Float(f) => f,
            Arg::Params(v) => v.x,
            other => panic!("va_arg wants a float, the caller passed {other:?}"),
        }
    }

    /// `va_arg(vlist, u32)`.
    pub fn int(&mut self) -> u32 {
        match self.take(ArgKind::Int) {
            Arg::Int(i) => i,
            other => panic!("va_arg wants a word, the caller passed {other:?}"),
        }
    }

    /// How many arguments the spawn read.
    pub fn read(&self) -> usize {
        self.next
    }
}

/// Loads effect file `index` if it isn't, `efAsync_LoadSync`: registers its two banks with
/// the particle system and keeps its effect descriptors. The file comes from the frontend's
/// set ([`Globals::files`]); whether the original had preloaded it only decides whether
/// `psInitDataBankLoad` or `psInitDataBank` registers the banks, and both do the same here
/// (banks are located as they're read).
#[doc(alias = "efAsync_LoadSync")]
pub fn load_sync(r: &mut Running, index: i32) {
    if !(0..50).contains(&index) {
        return;
    }
    let i = index as usize;
    let Some((file, table)) = FILES[i] else {
        return;
    };
    if r.game.ef.tables[i].is_some() {
        return;
    }
    let archive = r.game.ef.files[i]
        .unwrap_or_else(|| panic!("efAsync_LoadSync: {file} wasn't loaded by the frontend"));
    let offset = r
        .data
        .get(archive)
        .public(table)
        .unwrap_or_else(|| panic!("{file} has no {table}"));
    let t = DataRef { archive, offset };
    let cmd = r.data.ptr(t);
    let tex = r.data.ptr(t.at(4));
    if cmd.is_some() || tex.is_some() {
        let (cmd, tex) = (
            cmd.expect("an effect table's command bank"),
            tex.expect("an effect table's texture bank"),
        );
        crate::hsd::particle::init_data_bank(&mut r.game.objs, r.data, i, cmd, tex, None, None);
    }
    r.game.ef.tables[i] = Some(t.at(8));
}

/// An effect's descriptor (`EF_EffectDesc`) for `gfx_id`: its file's table, at the id's
/// remainder.
pub fn desc(game: &Game, gfx_id: i32) -> DataRef {
    let bank = gfx_id / 1000;
    let t = usize::try_from(bank)
        .ok()
        .and_then(|b| game.ef.tables.get(b).copied().flatten())
        .unwrap_or_else(|| {
            panic!("effect {gfx_id}: its file (bank {bank}) isn't loaded; the original reads low memory")
        });
    t.at(0x14 * (gfx_id % 1000) as u32)
}

/// Archives' read helpers for a descriptor.
pub(crate) fn desc_fields(data: &Archives, d: DataRef) -> (f32, [Option<DataRef>; 4]) {
    (
        data.f32(d),
        [
            data.ptr(d.at(4)),
            data.ptr(d.at(8)),
            data.ptr(d.at(0xC)),
            data.ptr(d.at(0x10)),
        ],
    )
}

/// efLib's and efAsync's functions other code reaches through [`code::call`]
/// (super::super::code::call), by address: `None` for any other.
pub fn call(r: &mut Running, f: u32, args: &[super::code::Val]) -> Option<super::code::Val> {
    use super::code::Val;
    let gobj = |i: usize| args[i].gobj();
    Some(match f {
        0x8005_B4B8 => {
            init(r);
            Val::None
        }
        0x8006_737C => {
            load_sync(r, args[0].int());
            Val::None
        }
        // `efAsync_LoadAsync` only asks the disc for a file, which a later sync load takes.
        0x8006_729C => Val::None,
        0x8005_FDDC => {
            let rest: Vec<Arg> = args[2..]
                .iter()
                .map(|v| match *v {
                    Val::JObj(j) => Arg::JObj(j),
                    Val::Vec(p) => Arg::Vec(p),
                    Val::Float(x) => Arg::Float(crate::fpu::mem(x)),
                    Val::Int(i) => Arg::Int(i as u32),
                    other => panic!("efSync_Spawn: argument {other:?}"),
                })
                .collect();
            Val::Spawned(spawn(r, args[0].int(), gobj(1), &rest))
        }
        0x8005_B880 => {
            destroy_all(r, gobj(0).expect("efLib_DestroyAll: gobj"));
            Val::None
        }
        0x8005_B780 => {
            destroy(r, gobj(0).expect("efLib_Destroy: gobj"));
            Val::None
        }
        0x8005_BA40 => {
            pause_all(r.game, gobj(0).expect("efLib_PauseAll: gobj"));
            Val::None
        }
        0x8005_BAC4 => {
            resume_all(r.game, gobj(0).expect("efLib_ResumeAll: gobj"));
            Val::None
        }
        0x8005_B704 => {
            set_flags(
                r.game,
                gobj(0).expect("efLib_SetFlags: gobj"),
                args[1].int(),
            );
            Val::None
        }
        _ => return None,
    })
}

//! The display pass's HSD side: the joint traversal (`HSD_JObjDispAll`, `displayfunc.c`), its
//! display objects and shapes (`HSD_DObjDisp`, `HSD_PObjDisp`), cameras (`HSD_CObjSetCurrent`)
//! and lights (`HSD_LObjSetupInit`). Drawing changes the game's state: setup brings joint and
//! envelope matrices up to date, materials make their texture matrices, a joint's particle
//! list spawns its particles the first time it is shown, and lights take GX slots. All of that
//! runs here as the original runs it; where the original issues GX commands, the pass appends
//! a [`Draw`] to the frame's list for the presentation layer instead.
//!
//! Only the screen render pass (`HSD_RP_SCREEN`, the scene loop's) is modeled. The original's
//! depth-sorted lists (`HSD_ZList`) are never filled in Melee (`zsort_listing` stays 0), so
//! every transparency pass draws in traversal order.

use melee_data::archive::Archives;

use super::Objects;
use super::cobj::CObjId;
use super::dobj::{self, DObjId};
use super::fog::FogId;
use super::jobj::{self, Contents, JObjId};
use super::lobj::{self, LObjId};
use super::mobj::{self, MObjClass, MObjId};
use super::particle::{Hooks, ParticleId};
use super::pobj::{self, Binding, PObjId};
use crate::mtx::Mtx34;

/// A transparency pass (`HSD_TrspMask`).
pub const TRSP_OPA: u32 = 1;
pub const TRSP_XLU: u32 = 2;
pub const TRSP_TEXEDGE: u32 = 4;

/// Where a joint's transparency flags start (`JOBJ_TRSP_SHIFT`): `JOBJ_OPA` is bit 18.
const JOBJ_TRSP_SHIFT: u32 = 18;
/// Where a joint's root transparency flags start: `JOBJ_ROOT_OPA` is bit 28.
const JOBJ_ROOT_SHIFT: u32 = 28;
/// Where a display object's transparency flags start (`DOBJ_TRSP_SHIFT`).
const DOBJ_TRSP_SHIFT: u32 = 1;
/// `DOBJ_HIDDEN`.
const DOBJ_HIDDEN: u32 = 1;
/// A render mode that skips materials, as shadows draw (`RENDER_SHADOW`).
pub const RENDER_SHADOW: u32 = 1 << 26;
/// A material with a specular highlight (`RENDER_SPECULAR`).
const RENDER_SPECULAR: u32 = 1 << 3;
/// A joint's billboard kind (`JOBJ_BILLBOARD_FIELD`): plain, vertical, horizontal, rotating.
const BILLBOARD_FIELD: u32 = 0xE00;
const BILLBOARDS: [u32; 5] = [0, 0x200, 0x400, 0x600, 0x800];
/// `POBJ_CULLFRONT | POBJ_CULLBACK`: a shape culled both ways isn't drawn at all.
const CULL_BOTH: u16 = 3 << 14;
/// A light's GX object needs loading (`LOBJ_DIFF_DIRTY`, `LOBJ_SPEC_DIRTY`).
const LOBJ_DIFF_DIRTY: u16 = 1 << 7;
const LOBJ_SPEC_DIRTY: u16 = 1 << 8;
/// Envelopes a shape's matrices can take (the GX position matrices).
const ENVELOPE_MAX: usize = 10;

/// What the presentation layer draws, in the order the original issues the GX commands each
/// stands for. Objects are named by handle: the presentation reads their state as the pass
/// left it.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A camera starts drawing: its viewport, scissor, projection and view
    /// (`HSD_CObjSetCurrent`).
    Camera(CObjId),
    /// The current camera's drawing ends (`HSD_CObjEndCurrent`).
    EndCamera,
    /// The active lights (`HSD_LObjSetupInit`; [`lobj::LObjs::active`]) light what follows.
    Lights,
    /// Fog for what follows, or none (`HSD_FogSet`).
    Fog(Option<FogId>),
    /// A shape of a display object of a joint, its matrices set up, in transparency pass
    /// `trsp` with `rendermode`. `vmtx` is the view matrix when it isn't the current
    /// camera's (inside an instance joint).
    Shape {
        jobj: JObjId,
        dobj: DObjId,
        pobj: PObjId,
        trsp: u32,
        rendermode: u32,
        vmtx: Option<Mtx34>,
    },
    /// The screen copied into refraction's source texture (`lbRefract_80022560`).
    RefractCopy,
    /// Debug text: a screen's rectangles and glyphs (`DevText_Draw`).
    Text(Box<crate::game::hud::devtext::Draw>),
    /// A particle link, from its first particle as the last sort left it (`psDispParticles`).
    Particles {
        link: usize,
        first: Option<ParticleId>,
    },
}

/// The display pass's output for the frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    pub draws: Vec<Draw>,
}

/// The transparency pass a GObj render pass draws, `HSD_GObj_80390EB8` (`HSD_GObj_804085F0`).
#[doc(alias = "HSD_GObj_80390EB8")]
pub fn pass_trsp(pass: i32) -> u32 {
    [TRSP_OPA, TRSP_TEXEDGE, TRSP_XLU, 0][usize::try_from(pass).expect("a render pass")]
}

/// Makes a camera current for drawing, `HSD_CObjSetCurrent` in the screen pass: its view
/// matrix is brought up to date. False without a camera.
#[doc(alias = "HSD_CObjSetCurrent")]
pub fn camera_begin(objs: &mut Objects, data: &Archives, c: Option<CObjId>) -> bool {
    let Some(c) = c else {
        return false;
    };
    objs.cobjs.current = Some(c);
    objs.display.draws.push(Draw::Camera(c));
    super::cobj::setup_viewing_mtx(objs, data, c);
    true
}

/// `HSD_CObjEndCurrent`: draws the depth-sorted lists, which Melee never fills.
#[doc(alias = "HSD_CObjEndCurrent")]
pub fn camera_end(objs: &mut Objects) {
    objs.display.draws.push(Draw::EndCamera);
}

/// `HSD_FogSet`: fog for what follows, or none. Fog needs a current camera.
#[doc(alias = "HSD_FogSet")]
pub fn fog_set(objs: &mut Objects, fog: Option<FogId>) {
    if fog.is_some() {
        assert!(objs.cobjs.current.is_some(), "You must specify CObj first.");
    }
    objs.display.draws.push(Draw::Fog(fog));
}

/// Draws a joint tree for transparency pass `trsp`, `HSD_JObjDispAll`: an instance joint
/// draws the tree it instances in its own place; otherwise the joint draws if its flags take
/// the pass, then its children if its root flags do.
#[doc(alias = "HSD_JObjDispAll")]
pub fn jobj_disp_all(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    j: Option<JObjId>,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    let Some(j) = j else { return };
    let flags = objs.jobjs[j].flags;
    if flags & jobj::INSTANCE != 0 {
        if flags & jobj::HIDDEN != 0 {
            return;
        }
        jobj::setup_matrix(objs, data, Some(j));
        let child = objs.jobjs[j].child;
        jobj::setup_matrix(objs, data, child);
        let child = child.expect("an instance joint without the tree it instances");
        let inv = objs.jobjs[child]
            .mtx
            .inverse()
            .expect("PSMTXInverse of a singular matrix leaves the stack's matrix");
        let m = objs.jobjs[j].mtx.concat(&inv);
        let cobj = objs
            .cobjs
            .current
            .expect("HSD_JObjDispAll: no current camera");
        let m = objs.cobjs[cobj].view_mtx.concat(&m);
        jobj_disp_all(objs, data, hooks, Some(child), Some(m), trsp, rendermode);
        return;
    }
    if flags & (trsp << JOBJ_TRSP_SHIFT) != 0 {
        jobj_disp(objs, data, hooks, j, vmtx, trsp, rendermode);
    }
    if flags & (trsp << JOBJ_ROOT_SHIFT) != 0 {
        let mut c = objs.jobjs[j].child;
        while let Some(x) = c {
            jobj_disp_all(objs, data, hooks, Some(x), vmtx, trsp, rendermode);
            c = objs.jobjs[x].next;
        }
    }
}

/// Draws one joint, `HSD_JObjDisp`: its display objects, or the first time a joint with a
/// particle list is shown (once efLib has set `sptcl_callback`), its particles.
#[doc(alias = "HSD_JObjDisp")]
pub fn jobj_disp(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    j: JObjId,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    match objs.jobjs[j].u {
        Contents::DObjs(_) => jobj_disp_dobj(objs, data, j, vmtx, trsp, rendermode),
        Contents::Particles(list) if objs.jobj.sptcl => {
            let mut entry = list;
            while let Some(e) = entry {
                if objs.jobj.pending_ptcl.contains(&e) {
                    let w = data.u32(e.at(4));
                    let (bank, idx) = ((w & 0x3F) as i32, (w >> 6 & 0xFF_FFFF) as i32);
                    hooks.joint_particles(objs, data, false, bank, idx, j);
                }
                objs.jobj.pending_ptcl.remove(&e);
                entry = data.ptr(e);
            }
        }
        _ => {}
    }
}

/// Draws a joint's display objects for each pass its flags take of `trsp`,
/// `HSD_JObjDispDObj`, its matrix brought up to date first.
#[doc(alias = "HSD_JObjDispDObj")]
pub fn jobj_disp_dobj(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    let flags = objs.jobjs[j].flags;
    if flags & jobj::HIDDEN != 0 {
        return;
    }
    let bits = flags & (trsp << JOBJ_TRSP_SHIFT);
    if bits == 0 {
        return;
    }
    jobj::setup_matrix(objs, data, Some(j));
    if vmtx.is_none() {
        assert!(
            objs.cobjs.current.is_some(),
            "HSD_CObjGetViewingMtxPtrDirect: no current camera"
        );
    }
    // The position matrix (`HSD_JObjMakePositionMtx`) is the presentation's, but an unknown
    // billboard kind panics there.
    assert!(
        BILLBOARDS.contains(&(flags & BILLBOARD_FIELD)),
        "unkown type of billboard."
    );
    for pass in [TRSP_OPA, TRSP_TEXEDGE, TRSP_XLU] {
        if bits & (pass << JOBJ_TRSP_SHIFT) != 0 {
            jobj_disp_sub(objs, data, j, vmtx, pass, rendermode);
        }
    }
}

/// Draws the display objects that take pass `trsp`, `HSD_JObjDispSub`, with the joint current.
#[doc(alias = "HSD_JObjDispSub")]
pub fn jobj_disp_sub(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    jobj::set_current(objs, Some(j));
    if rendermode & RENDER_SHADOW == 0 && objs.jobjs[j].flags & jobj::SPECULAR != 0 {
        lobj_setup_specular_init(objs);
    }
    let dtrsp = trsp << DOBJ_TRSP_SHIFT;
    let mut d = match objs.jobjs[j].u {
        Contents::DObjs(d) => d,
        _ => None,
    };
    while let Some(x) = d {
        let flags = objs.dobjs[x].flags;
        if flags & DOBJ_HIDDEN == 0 && flags & dtrsp != 0 {
            dobj::set_current(&mut objs.dobjs, Some(x));
            dobj_disp(objs, data, j, x, vmtx, trsp, rendermode);
        }
        d = objs.dobjs[x].next;
    }
    dobj::set_current(&mut objs.dobjs, None);
    jobj::set_current(objs, None);
}

/// Draws a display object's shapes in its material, `HSD_DObjDisp`.
#[doc(alias = "HSD_DObjDisp")]
pub fn dobj_disp(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    d: DObjId,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    let m = objs.dobjs[d].mobj;
    mobj::set_current(&mut objs.mobjs, m);
    let m = m.expect("a display object without a material");
    if rendermode & RENDER_SHADOW == 0 {
        material_setup(objs, m);
    }
    let mut p = objs.dobjs[d].pobj;
    while let Some(x) = p {
        pobj_disp(objs, data, j, d, x, vmtx, trsp, rendermode);
        p = objs.pobjs[x].next;
    }
    if rendermode & RENDER_SHADOW == 0 {
        mobj::unset(&mut objs.tobjs);
    }
    mobj::set_current(&mut objs.mobjs, None);
}

/// A material's display setup by its class (`setup`): HAL's, or the classes no port
/// covers yet.
fn material_setup(objs: &mut Objects, m: MObjId) {
    match objs.mobjs[m].class {
        MObjClass::Hsd => {
            mobj::setup(objs, m);
            setup_channel_mode(objs, m);
        }
        // grMaterial_803E0A20's setup.
        MObjClass::Ground => panic!("fn_801C8EF8 isn't ported"),
        // it_mobj's setup.
        MObjClass::Item => panic!("fn_80277D8C isn't ported"),
        // ftMObj's setup.
        MObjClass::Fighter => panic!("ftMaterial_800BF2B8 isn't ported"),
    }
}

/// The simulation side of the material's render mode setup (`HSD_SetupChannelMode`, which
/// `HSD_MObjSetup` reaches): a specular material loads every active light with its own
/// shininess.
#[doc(alias = "HSD_SetupChannelMode")]
fn setup_channel_mode(objs: &mut Objects, m: MObjId) {
    let o = &objs.mobjs[m];
    if o.rendermode & RENDER_SPECULAR == 0 {
        return;
    }
    let shininess = o.mat.expect("a material without its colors").shininess;
    for i in 0..lobj::nb_active(&objs.lobjs) {
        if let Some(l) = lobj::active_by_index(&objs.lobjs, i) {
            let color = objs.lobjs[l].color;
            lobj_setup(objs, l, color, shininess);
        }
    }
}

/// Draws a shape, `HSD_PObjDisp`: a shape culled both ways is skipped; a skinned shape's
/// second joint and an enveloped shape's joints have their matrices brought up to date
/// (`SetupSharedVtxModelMtx`, `SetupEnvelopeModelMtx`).
#[doc(alias = "HSD_PObjDisp")]
#[allow(clippy::too_many_arguments)]
pub fn pobj_disp(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    d: DObjId,
    p: PObjId,
    vmtx: Option<Mtx34>,
    trsp: u32,
    rendermode: u32,
) {
    if objs.pobjs[p].flags & CULL_BOTH == CULL_BOTH {
        return;
    }
    match objs.pobjs[p].ty() {
        pobj::SKIN => {
            // `HSD_PObjSetMtxMark` never stores a mark, so the shared joint's setup always
            // runs.
            if let Binding::Skin(Some(s)) = objs.pobjs[p].u {
                jobj::setup_matrix(objs, data, Some(s));
            }
        }
        pobj::ENVELOPE => {
            let Binding::Envelopes(envs) = &objs.pobjs[p].u else {
                panic!("an enveloped shape without envelopes");
            };
            let envs = envs.clone();
            setup_envelopes(objs, data, j, &envs);
        }
        _ => {}
    }
    objs.display.draws.push(Draw::Shape {
        jobj: j,
        dobj: d,
        pobj: p,
        trsp,
        rendermode,
        vmtx,
    });
}

/// The joint a skinned joint's envelopes are relative to, the inline `HSD_JObjFindSkeleton`.
fn find_skeleton(objs: &Objects, j: JObjId) -> Option<JObjId> {
    std::iter::successors(Some(j), |&x| objs.jobjs[x].parent)
        .find(|&x| objs.jobjs[x].flags & (jobj::SKELETON | jobj::SKELETON_ROOT) != 0)
}

/// The simulation side of `SetupEnvelopeModelMtx`: each of the first ten envelopes' joints
/// brought up to date, one joint for a full weight, every joint of a blend. The matrices
/// themselves go to GX; reading a missing envelope matrix fails as the original's null read
/// would.
fn setup_envelopes(objs: &mut Objects, data: &Archives, j: JObjId, envs: &[Vec<pobj::Envelope>]) {
    // `_HSD_mkEnvelopeModelNodeMtx`.
    let relative = if objs.jobjs[j].flags & jobj::SKELETON_ROOT != 0 {
        false
    } else {
        let x = find_skeleton(objs, j).expect("an enveloped shape's joint without a skeleton");
        if x == j || objs.jobjs[x].flags & jobj::SKELETON_ROOT == 0 {
            assert!(
                objs.jobjs[x].envelopemtx.is_some(),
                "a skeleton without an envelope matrix"
            );
        }
        true
    };
    for env in envs.iter().take(ENVELOPE_MAX) {
        let first = env.first().expect("an empty envelope");
        if first.weight >= 1.0 {
            jobj::setup_matrix(objs, data, first.jobj);
            if relative {
                let e = first.jobj.expect("an envelope without its joint");
                assert!(objs.jobjs[e].envelopemtx.is_some(), "no envelope matrix");
            }
        } else {
            for e in env {
                let jp = e.jobj.expect("an envelope without its joint");
                jobj::setup_matrix(objs, data, Some(jp));
                assert!(objs.jobjs[jp].envelopemtx.is_some(), "no envelope matrix");
            }
        }
    }
}

/// The simulation side of `HSD_LObjSetupSpecularInit`: each active light with a specular slot
/// needs its GX object loaded again.
#[doc(alias = "HSD_LObjSetupSpecularInit")]
fn lobj_setup_specular_init(objs: &mut Objects) {
    for i in 0..lobj::nb_active(&objs.lobjs) {
        let l = lobj::active_by_index(&objs.lobjs, i).expect("an active light");
        if objs.lobjs[l].spec_id == 0 {
            continue;
        }
        assert!(
            lobj::light_type(&objs.lobjs, l) != lobj::AMBIENT,
            "an ambient light with a specular slot"
        );
        objs.lobjs[l].flags |= LOBJ_SPEC_DIRTY;
    }
}

/// Gives the current lights GX slots for camera `c`'s view, `HSD_LObjSetupInit`: the
/// diffuse and specular lights in order (at most eight specular ones), then an ambient light
/// if none took the ambient slot, then specular parts for lights that are both; each light's
/// GX object state (its color, specular position and shininess) as it would load.
#[doc(alias = "HSD_LObjSetupInit")]
pub fn lobj_setup_init(objs: &mut Objects, data: &Archives, c: CObjId) {
    let vmtx = objs.cobjs[c].view_mtx;
    let l = &mut objs.lobjs;
    l.lightmask_diffuse = 0;
    l.lightmask_specular = 0;
    l.lightmask_attnfunc = 0;
    l.lightmask_alpha = 0;
    lobj::clear_active(l);
    let max = lobj::MAX_GXLIGHT as i32 - 1;
    let (mut idx, mut num) = (0, 0);
    let mut list = objs.lobjs.current;
    while idx < max
        && let Some(node) = list
    {
        list = objs.lobjs.nodes[node.0].next;
        let l = objs.lobjs.nodes[node.0].data;
        let flags = objs.lobjs[l].flags;
        if flags & lobj::HIDDEN != 0 {
            continue;
        }
        let ty = lobj::light_type(&objs.lobjs, l);
        let reflect = flags & (lobj::DIFFUSE | lobj::SPECULAR);
        if ty != lobj::AMBIENT {
            match reflect {
                lobj::SPECULAR if num >= 8 => continue,
                lobj::SPECULAR | lobj::DIFFUSE => {}
                r if r == lobj::DIFFUSE | lobj::SPECULAR => {}
                _ => continue,
            }
        }
        lobj::set_active(&mut objs.lobjs, l);
        idx = lobj::nb_active(&objs.lobjs);
        objs.lobjs[l].spec_id = 0;
        match ty {
            lobj::POINT => objs.lobjs[l].hw_color = objs.lobjs[l].color,
            lobj::AMBIENT => continue,
            // Infinite and spot lights only set their GX objects.
            _ => {}
        }
        if reflect & lobj::DIFFUSE != 0 {
            setup_diffuse(objs, l);
        } else if reflect == lobj::SPECULAR {
            let id = objs.lobjs[l].id;
            setup_spec(objs, data, l, &vmtx, id);
            num += 1;
        }
    }
    if lobj::active_by_id(&objs.lobjs, 0x100).is_none() {
        while let Some(node) = list {
            let l = objs.lobjs.nodes[node.0].data;
            let o = &objs.lobjs[l];
            if lobj::light_type(&objs.lobjs, l) == lobj::AMBIENT && o.flags & lobj::HIDDEN == 0 {
                lobj::set_active(&mut objs.lobjs, l);
                break;
            }
            list = objs.lobjs.nodes[node.0].next;
        }
    }
    let mut list = objs.lobjs.current;
    while num < max
        && idx < max
        && let Some(node) = list
    {
        list = objs.lobjs.nodes[node.0].next;
        let l = objs.lobjs.nodes[node.0].data;
        let both = lobj::DIFFUSE | lobj::SPECULAR;
        if objs.lobjs[l].flags & (lobj::HIDDEN | both) != both {
            continue;
        }
        let id = lobj::index_to_light_id(idx as u32);
        idx += 1;
        setup_spec(objs, data, l, &vmtx, id);
        num += 1;
    }
    for i in 0..max {
        let Some(l) = lobj::active_by_index(&objs.lobjs, i) else {
            return;
        };
        let (color, shininess) = (objs.lobjs[l].color, objs.lobjs[l].shininess);
        lobj_setup(objs, l, color, shininess);
    }
}

/// `setup_diffuse_lightobj`.
fn setup_diffuse(objs: &mut Objects, l: LObjId) {
    let ty = lobj::light_type(&objs.lobjs, l);
    let o = &mut objs.lobjs[l];
    o.hw_color = o.color;
    o.flags |= LOBJ_DIFF_DIRTY;
    let (id, flags) = (o.id, o.flags);
    let ls = &mut objs.lobjs;
    ls.lightmask_diffuse |= id;
    match ty {
        lobj::SPOT | lobj::POINT => ls.lightmask_attnfunc |= id,
        lobj::INFINITE => {}
        _ => panic!("a diffuse ambient light"),
    }
    if flags & lobj::ALPHA != 0 {
        ls.lightmask_alpha |= id;
    }
}

/// `setup_spec_lightobj`: the light's specular slot, and with one, its shininess and its
/// position (or direction) in view space.
fn setup_spec(objs: &mut Objects, data: &Archives, l: LObjId, vmtx: &Mtx34, spec_id: u32) {
    objs.lobjs[l].spec_id = spec_id;
    if spec_id == 0 {
        return;
    }
    objs.lobjs[l].shininess = 50.0;
    let lvec = match lobj::light_type(&objs.lobjs, l) {
        lobj::POINT | lobj::SPOT => {
            // Without a position WObj the original transforms the vector as it was.
            let p = lobj::position(objs, data, Some(l)).unwrap_or(objs.lobjs[l].lvec);
            vmtx.transform(p)
        }
        lobj::INFINITE => {
            let v = lobj::light_vector(objs, data, Some(l)).expect("a light");
            vmtx.rotate(v).sdk_normalize()
        }
        _ => panic!("a specular ambient light"),
    };
    let o = &mut objs.lobjs[l];
    o.lvec = lvec;
    o.flags |= LOBJ_SPEC_DIRTY;
    objs.lobjs.lightmask_specular |= spec_id;
}

/// Loads a light's GX objects with `color` and `shininess`, `HSD_LObjSetup`: what changed is
/// kept, and what loads is no longer dirty.
#[doc(alias = "HSD_LObjSetup")]
fn lobj_setup(objs: &mut Objects, l: LObjId, color: [u8; 4], shininess: f32) {
    let o = &mut objs.lobjs[l];
    if o.flags & lobj::HIDDEN != 0 || o.flags & lobj::TYPE_MASK == lobj::AMBIENT {
        return;
    }
    if o.flags & lobj::DIFFUSE != 0 {
        if o.hw_color != color {
            o.hw_color = color;
            o.flags |= LOBJ_DIFF_DIRTY;
        }
        o.flags &= !LOBJ_DIFF_DIRTY;
    }
    if o.spec_id != 0 {
        if crate::fpu::reg(o.shininess) != crate::fpu::reg(shininess) {
            o.shininess = shininess;
            o.flags |= LOBJ_SPEC_DIRTY;
        }
        o.flags &= !LOBJ_SPEC_DIRTY;
    }
}

/// A light GObj's render callback, `HSD_GObj_LObjCallback`: its light list becomes the
/// current lights and takes GX slots for the current camera.
#[doc(alias = "HSD_GObj_LObjCallback")]
pub fn lobj_callback(objs: &mut Objects, data: &Archives, l: Option<LObjId>) {
    lobj::replace_current(objs, l);
    let c = objs
        .cobjs
        .current
        .expect("HSD_LObjSetupInit: no current camera");
    lobj_setup_init(objs, data, c);
    objs.display.draws.push(Draw::Lights);
}

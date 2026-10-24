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
use super::tobj::{self, TObjId};
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
/// A translucent material (`RENDER_XLU`).
const RENDER_XLU: u32 = 1 << 30;
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
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum Draw {
    /// A camera starts drawing: its viewport, scissor, projection and view
    /// (`HSD_CObjSetCurrent`), as [`Display::cameras`] holds them at this index (a camera can
    /// draw more than once a frame, moved in between).
    Camera(CObjId, u16),
    /// The current camera's drawing ends (`HSD_CObjEndCurrent`).
    EndCamera,
    /// The lights `HSD_LObjSetupInit` gave GX slots light what follows, as
    /// [`Display::light_sets`] holds them at this index.
    Lights(u16),
    /// As [`Draw::Lights`], but set up after the next material's render mode: a fighter's
    /// material that set its light up without alpha sets its channels from those lights
    /// (`HSD_SetupRenderModeWithCustomPE`) before it sets them up again with alpha
    /// (`ftMaterial_800BF2B8`'s `ftCo_8009F75C(fp, true)`).
    LightsAfterRenderMode(u16),
    /// The GObj whose render callback draws what follows (`HSD_GObj_804D7814`), whose user
    /// data materials read as they set up (a fighter's, a stage object's, an item's).
    Rendering(Option<super::gobj::GObjId>),
    /// The shadow textures shadowed materials take after their own for what follows
    /// (`tobj_shadows`), as [`Display::shadow_tobjs`] holds them from `start`: the stage
    /// switches each fighter's shadow joint by joint (`grDisplay_801C5B90`), so the list can
    /// change between draws.
    Shadows { start: u16, len: u8 },
    /// Fog for what follows, or none (`HSD_FogSet`).
    Fog(Option<FogId>),
    /// HAL's state caches out of date for the parts `mask` names (`HSD_StateInvalidate`), as
    /// a render callback makes them before it draws, so the next draws send their state again.
    Invalidate(u32),
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
    /// A specular joint's half vectors toward it for each specular light
    /// (`HSD_LObjSetupSpecularInit`), as `HSD_JObjDispSub` makes them for every pass it draws
    /// the joint in, whether or not any of its shapes draw in that pass: the last joint's
    /// vectors stay in the lights for what follows. `vmtx` as for [`Draw::Shape`].
    SpecularInit { jobj: JObjId, vmtx: Option<Mtx34> },
    /// A fighter's model's root joint draws its display objects (`HSD_JObjDispDObj` on a joint
    /// of the fighters' class, `ftJObj`, whose position matrix sets the fighters' depth scale
    /// for the shapes after, `ftParts_JObjMakePositionMtx`), whether or not any of them draws.
    /// `vmtx` as for [`Draw::Shape`].
    FighterJoint { jobj: JObjId, vmtx: Option<Mtx34> },
    /// The current camera's viewport erased to the erase color (`HSD_CObjEraseScreen`).
    Erase,
    /// The current camera's viewport erased, color and depth, to the erase color set before
    /// it (`HSD_CObjEraseScreen(cobj, 1, 0, 1)`, the title's erase camera).
    EraseWithDepth,
    /// The sprites' camera's setup (`HSD_SObjLib_803A54EC`): the position matrix from its
    /// view, two texture coordinates from the first, and a white register channel.
    SpriteSetup,
    /// A sprite (`HSD_SObjLib_803A4A68`).
    SObj(super::sobj::SObjId),
    /// The movie's planes loaded as texture maps 0 to 2 (`lbMthp_8001F67C`), with the frame
    /// they hold (none before the first decode: buffers never written).
    MoviePlanes(Option<crate::lb::mthp::MovieFrame>),
    /// The sprites' camera's end (`HSD_SObjLib_803A54EC` after its links): HAL's state caches
    /// out of date and depth tested, compared less-or-equal, and written.
    SpriteEnd,
    /// The magnifier camera's viewport erased, color and depth, for player `n`'s bubble
    /// (`HSD_CObjEraseScreen(cobj, 1, 0, 1)`), in the color `ifMagnify_802FBBDC` blends from
    /// the stage's colors by where the fighter is, which the presentation works out.
    MagnifierErase(u8),
    /// The screen copied into refraction's source texture (`lbRefract_80022560`).
    RefractCopy,
    /// A refracted fighter's tree drawn: the TEV back to direct texturing and HAL's state
    /// caches out of date (`ftCo_800C8AF0`'s `lbRefract_800225D4`).
    RefractEnd,
    /// The render copied into a shadow's texture (`HSD_ShadowEndRender`).
    ShadowCopy(super::shadow::ShadowId),
    /// Later draws render offscreen (`true`, into a texture) or to the screen
    /// (`HSD_StartRender`).
    Offscreen(bool),
    /// A shadow camera's volume drawn into its texture (`lbShadow_8000F214`).
    ShadowVolume(super::shadow::ShadowId),
    /// Debug text: a screen's rectangles and glyphs (`DevText_Draw`).
    Text(Box<crate::game::hud::devtext::Draw>),
    /// An app-SRT's view matrix and screen scales, as drawing a particle of it in the current
    /// camera brings them up to date (`psDispSubAppSRT`, `psDispSubAPPSRTPoint`) for the
    /// particles drawn after: each camera that draws them in a display frame makes them
    /// again, so the frame's last are only the last camera's.
    AppSrt {
        srt: super::appsrt::AppSrtId,
        view: Mtx34,
        scale: [f32; 2],
    },
    /// A particle link, from its first particle as the last sort left it (`psDispParticles`).
    Particles {
        link: usize,
        first: Option<ParticleId>,
    },
    /// A text's background and glyphs (`HSD_SisLib_803A84BC`).
    SisText(Box<super::sis::TextDraw>),
    /// The render copied into a player's magnifier bubble (`lb_800122C8`).
    MagnifierCopy(u8),
    /// The render copied into Fountain of Dreams' reflection image, the frame cleared after it
    /// (`lb_800122C8`).
    ReflectionCopy,
    /// The point size and its texture offset for later point draws (`HSD_StateSetPointSize`).
    PointSize(u8, u8),
    /// The light vector highlight textures take for what follows, or none without an infinite
    /// light: the first current infinite light's (`HSD_LObjGetCurrentByType`,
    /// `HSD_LObjGetLightVector`) as a material with one sets up (`TObjSetupMtx`). Render
    /// callbacks change the current lights between draws, so the frame's last list isn't it.
    Hilight(Option<crate::vec::Vec3>),
    /// The toon texture's image for what follows (`tobj_toon->imagedesc`), as a toon material
    /// sets up (`HSD_MObjSetup`): Poké Floats' render callback sets it around each float's
    /// draw and clears it after (`fn_802130D0`), so the frame's end holds none.
    Toon(Option<tobj::ImageRef>),
    /// The draw flags of the fighter whose materials set up next, as a fighter's material
    /// sets up (`ftMaterial_800BF2B8`): its render callback sets them before each tree it
    /// draws (`ftDrawCommon_80080E18`: an outline, the fighter, then its silhouette, in one
    /// pass), so the frame's end holds only the last tree's.
    FighterFlags(FighterFlags),
    /// The erase color for later erases (`HSD_SetEraseColor`).
    EraseColor([u8; 4]),
    /// The render from (`x`, `y`) copied into one of Pokémon Stadium's screens' images, the
    /// frame cleared after it with `clear` (`lb_800122C8`).
    ScreenCopy {
        screen: u8,
        x: u16,
        y: u16,
        clear: bool,
    },
    /// A flat-colored quad in the current camera's view at depth 0 (`hsd_80391A04`'s debug
    /// drawing: the background flash's).
    Quad(Quad),
    /// A fighter's sword afterimage (`ftCo_800C2600`): a triangle strip blended over the
    /// current camera's view, which the presentation builds from the fighter's afterimage
    /// ring (`x20B0`, `x2100`, `x2101`), its two widths (`x20F8`, `x20FC`) and the sword
    /// attributes' colors, as the pass left them.
    Afterimage(crate::game::handles::FighterId),
}

/// What [`Draw::FighterFlags`] names: the flags a fighter's render callback sets for each
/// tree it draws, which its materials read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct FighterFlags {
    /// An outline or silhouette tree (`x2223_b2`): its color, no depth update.
    pub x2223_b2: bool,
    /// The silhouette (`x2223_b3`): no tint, its own pixel-engine mode.
    pub x2223_b3: bool,
    /// The low models (`x2228_b0`): the shared material only with `x2224_b0` clear.
    pub x2228_b0: bool,
}

/// A quad of [`Draw::Quad`]: two corners and its color.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Quad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub color: [u8; 4],
}

/// A camera as [`Draw::Camera`] draws with it: what `HSD_CObjSetCurrent` sends GX.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CameraView {
    /// `xmin`, `xmax`, `ymin`, `ymax`.
    pub viewport: [f32; 4],
    /// `left`, `right`, `top`, `bottom`.
    pub scissor: [u16; 4],
    pub projection_type: u8,
    pub projection: [f32; 4],
    pub near: f32,
    pub far: f32,
    pub view_mtx: Mtx34,
}

/// A light with a GX slot, as `HSD_LObjSetupInit` left it.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct ActiveLight {
    pub lobj: LObjId,
    pub id: u32,
    pub spec_id: u32,
    pub shininess: f32,
    /// A specular light's position or direction in view space.
    pub lvec: crate::vec::Vec3,
}

/// The lights of a [`Draw::Lights`]: the active table, the light masks and the view they
/// were set up for.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct LightSet {
    pub active: [Option<ActiveLight>; lobj::MAX_GXLIGHT],
    pub nb_active: i32,
    pub mask_diffuse: u32,
    pub mask_specular: u32,
    pub mask_attnfunc: u32,
    pub mask_alpha: u32,
    pub view_mtx: Mtx34,
}

/// The display pass's output for the frame.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Display {
    pub draws: Vec<Draw>,
    /// The cameras [`Draw::Camera`]s name, and the light setups [`Draw::Lights`] name, for
    /// the frame: the presentation reads them, and the screen pass starts them afresh.
    pub cameras: Vec<CameraView>,
    pub light_sets: Vec<LightSet>,
    /// The shadow texture lists [`Draw::Shadows`] name, one after another, and the last one
    /// named this frame.
    pub shadow_tobjs: Vec<TObjId>,
    pub shadows_named: Option<(u16, u8)>,
    /// The highlight light vector last named this frame ([`Draw::Hilight`]).
    pub hilight_named: Option<Option<crate::vec::Vec3>>,
    /// The toon texture's image last named this frame ([`Draw::Toon`]).
    pub toon_named: Option<Option<tobj::ImageRef>>,
    /// The fighter draw flags last named this frame ([`Draw::FighterFlags`]).
    pub fighter_flags_named: Option<FighterFlags>,
    /// The render pass (`current_render_pass`): offscreen, into a texture, or the screen.
    pub offscreen: bool,
    /// The fighter whose trees draw, for its materials.
    pub fighter: Option<FighterMaterial>,
    /// The debug text's drawing as its screens hand it on, reused frame to frame.
    pub devtext: Vec<crate::game::hud::devtext::Draw>,
}

/// Starts a render pass, `HSD_StartRender`: offscreen (a shadow's texture) or the screen.
#[doc(alias = "HSD_StartRender")]
pub fn set_offscreen(objs: &mut Objects, offscreen: bool) {
    objs.display.offscreen = offscreen;
    objs.display.draws.push(Draw::Offscreen(offscreen));
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
    super::cobj::setup_viewing_mtx(objs, data, c);
    let o = &objs.cobjs[c];
    let view = CameraView {
        viewport: o.viewport,
        scissor: o.scissor,
        projection_type: o.projection_type,
        projection: o.projection,
        near: o.near,
        far: o.far,
        view_mtx: o.view_mtx,
    };
    let d = &mut objs.display;
    d.draws.push(Draw::Camera(c, d.cameras.len() as u16));
    d.cameras.push(view);
    true
}

/// Starts the frame's draws: the screen pass forgets the last frame's cameras and lights.
pub fn start_frame(objs: &mut Objects) {
    objs.display.cameras.clear();
    objs.display.light_sets.clear();
    objs.display.shadow_tobjs.clear();
    objs.display.shadows_named = None;
    objs.display.hilight_named = None;
    objs.display.toon_named = None;
    objs.display.fighter_flags_named = None;
}

/// Names the toon texture's image a toon material is about to take ([`Draw::Toon`]), when it
/// differs from the last named this frame.
fn name_toon(objs: &mut Objects) {
    let image = objs.mobjs.toon.and_then(|t| objs.tobjs[t].imagedesc);
    let d = &mut objs.display;
    if d.toon_named != Some(image) {
        d.toon_named = Some(image);
        d.draws.push(Draw::Toon(image));
    }
}

/// Names the shadow textures a shadowed material is about to take ([`Draw::Shadows`]), when
/// they differ from the last named this frame.
fn name_shadows(objs: &mut Objects) {
    let d = &mut objs.display;
    let current = || objs.tobjs.chain(objs.mobjs.shadows);
    if let Some((start, len)) = d.shadows_named {
        let (start, len) = (usize::from(start), usize::from(len));
        if current().eq(d.shadow_tobjs[start..start + len].iter().copied()) {
            return;
        }
    }
    let start = d.shadow_tobjs.len();
    d.shadow_tobjs.extend(current());
    let named = (
        u16::try_from(start).expect("too many shadow lists"),
        u8::try_from(d.shadow_tobjs.len() - start).expect("too many shadows"),
    );
    d.shadows_named = Some(named);
    d.draws.push(Draw::Shadows {
        start: named.0,
        len: named.1,
    });
}

/// The GObj whose render callback draws what follows, for the presentation
/// ([`Draw::Rendering`]).
pub fn rendering(objs: &mut Objects, g: Option<super::gobj::GObjId>) {
    objs.display.draws.push(Draw::Rendering(g));
}

/// `HSD_CObjEndCurrent`: draws the depth-sorted lists, which Melee never fills.
#[doc(alias = "HSD_CObjEndCurrent")]
pub fn camera_end(objs: &mut Objects) {
    objs.display.draws.push(Draw::EndCamera);
}

/// `HSD_StateInvalidate`: HAL's state caches out of date for the parts `mask` names
/// (`HSD_STATE_*`; -1 for all), for the presentation ([`Draw::Invalidate`]).
#[doc(alias = "HSD_StateInvalidate")]
pub fn state_invalidate(objs: &mut Objects, mask: u32) {
    objs.display.draws.push(Draw::Invalidate(mask));
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
    if objs.jobjs[j].class == super::heap::Class::FtJObj {
        objs.display
            .draws
            .push(Draw::FighterJoint { jobj: j, vmtx });
    }
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
        objs.display
            .draws
            .push(Draw::SpecularInit { jobj: j, vmtx });
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
        material_setup(objs, data, m);
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
fn material_setup(objs: &mut Objects, data: &Archives, m: MObjId) {
    if objs.mobjs[m].rendermode & mobj::RENDER_SHADOW != 0 {
        name_shadows(objs);
    }
    if objs.mobjs[m].rendermode & mobj::RENDER_TOON != 0 {
        name_toon(objs);
    }
    match objs.mobjs[m].class {
        // The stages' (`fn_801C8EF8`) and items' (`fn_80277D8C`) setups differ from HAL's
        // only in the TEV stages and blending they add for their color overlays, which are
        // GX's.
        MObjClass::Hsd | MObjClass::Ground | MObjClass::Item => {
            mobj::setup(objs, m);
            texture_matrices(objs, data, m);
            setup_channel_mode(objs, m);
        }
        MObjClass::Fighter => fighter_material(objs, data, m),
    }
}

/// What a fighter's material setup reads of the fighter that draws (the GObj being drawn,
/// `HSD_GObj_804D7814`), set by the fighter's render callback while its trees draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FighterMaterial {
    /// The fighter draws refracted (`x2226_b5`): refraction's chain for its vibration frame.
    pub refract: Option<TObjId>,
    /// The shared material its materials are swapped for: the metal one, or the other
    /// (`ft_804D6580`, `ft_804D6588`).
    pub swapped: Option<MObjId>,
    /// It draws translucent (`x61D` set).
    pub translucent: bool,
    /// The draw flags its render callback set for the tree.
    pub flags: FighterFlags,
    /// Its color overlay lights it (`x7C_flag2` and `x7C_light_enable`).
    pub overlay_light: bool,
    /// Its own light, and the fighters' lights (`lobj0`, and `lobj1`, the first ambient).
    pub light: Option<LObjId>,
    pub lights: Option<LObjId>,
    pub ambient: Option<LObjId>,
}

/// A fighter's material setup, `ftMaterial_800BF2B8`: a refracted fighter's refraction
/// texture; otherwise HAL's setup of the material, or of the shared one it's swapped for,
/// and when the overlay lights the fighter but the material leaves alpha to the lights
/// (translucent, or `x2223_b2`), the fighter's light set up without alpha, the render mode's
/// channels set from those lights, then the light set up with alpha
/// (`ftMaterial_800BF534`'s `ftCo_8009F75C(fp, false)`, `HSD_SetupRenderModeWithCustomPE`,
/// then `ftCo_8009F75C(fp, true)`).
#[doc(alias = "ftMaterial_800BF2B8")]
fn fighter_material(objs: &mut Objects, data: &Archives, m: MObjId) {
    let fm = objs
        .display
        .fighter
        .expect("a fighter's material drawn outside its fighter");
    let d = &mut objs.display;
    if d.fighter_flags_named != Some(fm.flags) {
        d.fighter_flags_named = Some(fm.flags);
        d.draws.push(Draw::FighterFlags(fm.flags));
    }
    if let Some(t) = fm.refract {
        // `lbRefract_80022998`: the chain's screen copy is a shadow-coordinate texture, so its
        // setup brings the current camera's inverse view matrix up to date.
        tobj::setup(&mut objs.tobjs, Some(t));
        let mut next = Some(t);
        while let Some(t) = next {
            next = objs.tobjs[t].next;
            texture_matrix(objs, data, t);
        }
        return;
    }
    let m = fm.swapped.unwrap_or(m);
    mobj::setup(objs, m);
    texture_matrices(objs, data, m);
    let rendermode = objs.mobjs[m].rendermode | if fm.translucent { RENDER_XLU } else { 0 };
    let texp = fm.overlay_light && rendermode & RENDER_XLU == 0 && !fm.flags.x2223_b2;
    if texp || !fm.overlay_light {
        setup_channel_mode(objs, m);
        return;
    }
    let c = objs
        .cobjs
        .current
        .expect("HSD_LObjSetupInit: no current camera");
    // `ftCo_8009F75C(fp, false)`, before the render mode, whose channels take these lights.
    lobj::clear_flags(&mut objs.lobjs, fm.light, 1 << 4);
    lobj::set_current_all(objs, fm.ambient);
    lobj::add_current(objs, fm.light);
    lobj_setup_init(objs, data, c);
    setup_channel_mode(objs, m);
    // `ftCo_8009F75C(fp, true)`, after it.
    lobj::set_flags(&mut objs.lobjs, fm.light, 1 << 4);
    lobj::replace_current(objs, fm.lights);
    lobj::add_current(objs, fm.light);
    setup_lights(objs, data, c);
    push_light_set(objs, c, true);
}

/// The rest of what `TObjSetupMtx` changes as it loads a material's texture matrices for
/// GX, after [`mobj::setup`]: a shadow texture's matrix takes the current camera's inverse
/// view matrix, made if it's out of date, and a highlight texture's the current infinite
/// light's vector.
fn texture_matrices(objs: &mut Objects, data: &Archives, m: MObjId) {
    let o = &objs.mobjs[m];
    let shadows = (o.rendermode & mobj::RENDER_SHADOW != 0)
        .then_some(objs.mobjs.shadows)
        .flatten();
    // The material's textures, then the shadows' (no setup here changes either chain).
    for first in [o.tobj, shadows] {
        let mut next = first;
        while let Some(t) = next {
            next = objs.tobjs[t].next;
            texture_matrix(objs, data, t);
        }
    }
}

/// One texture's part of [`texture_matrices`].
fn texture_matrix(objs: &mut Objects, data: &Archives, t: TObjId) {
    let o = &objs.tobjs[t];
    if o.id == tobj::GX_TEXMAP_NULL {
        return;
    }
    match o.flags & tobj::COORD_MASK {
        tobj::COORD_SHADOW => {
            let c = objs.cobjs.current.expect("TObjSetupMtx: no current camera");
            super::cobj::inv_viewing_mtx_direct(&mut objs.cobjs, c);
        }
        tobj::COORD_HILIGHT => {
            let vector = lobj::current_by_type(&objs.lobjs, lobj::INFINITE).and_then(|l| {
                assert!(objs.cobjs.current.is_some(), "cobj");
                lobj::light_vector(objs, data, Some(l))
            });
            let d = &mut objs.display;
            if d.hilight_named != Some(vector) {
                d.hilight_named = Some(vector);
                d.draws.push(Draw::Hilight(vector));
            }
        }
        _ => {}
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
            // Lent out while the joints are set up, which reads no shape.
            let Binding::Envelopes(envs) = &mut objs.pobjs[p].u else {
                panic!("an enveloped shape without envelopes");
            };
            let envs = std::mem::take(envs);
            setup_envelopes(objs, data, j, &envs);
            objs.pobjs[p].u = Binding::Envelopes(envs);
        }
        pobj::SHAPEANIM => shape_anim_buffers(objs, p),
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

/// `HSD_DEFAULT_MAX_SHAPE_VERTICES`, `HSD_DEFAULT_MAX_SHAPE_NORMALS`.
const MAX_SHAPE_VERTICES: u32 = 2000;
const MAX_SHAPE_NORMALS: u32 = 2000;

/// The simulation side of `drawShapeAnim`: the scene's first shape animation drawn takes the
/// blend buffers from the heap (the normals' only with a normal description); the blend
/// itself is the renderer's.
fn shape_anim_buffers(objs: &mut Objects, p: PObjId) {
    let Binding::ShapeSet(set) = &objs.pobjs[p].u else {
        panic!("a shape animation without its shape set");
    };
    let (n_vtx, has_normals) = (set.nb_vertex_index, set.normal_desc.is_some());
    let o = &mut objs.pobjs;
    if o.vertex_buffer == 0 {
        o.vertex_buffer = crate::hsd::heap::mem_alloc(MAX_SHAPE_VERTICES * 12);
    }
    assert!(
        n_vtx <= MAX_SHAPE_VERTICES as i32,
        "the original's assertion failed: vertex_buffer_size >= shape_set->nb_vertex_index"
    );
    if has_normals && o.normal_buffer == 0 {
        o.normal_buffer = crate::hsd::heap::mem_alloc(MAX_SHAPE_NORMALS * 12);
    }
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
    setup_lights(objs, data, c);
    push_light_set(objs, c, false);
}

/// Names the lights [`setup_lights`] just set up for what follows ([`Draw::Lights`]), or
/// for what follows the next material's render mode ([`Draw::LightsAfterRenderMode`]).
fn push_light_set(objs: &mut Objects, c: CObjId, after_render_mode: bool) {
    let l = &objs.lobjs;
    let mut set = LightSet {
        nb_active: l.nb_active,
        mask_diffuse: l.lightmask_diffuse,
        mask_specular: l.lightmask_specular,
        mask_attnfunc: l.lightmask_attnfunc,
        mask_alpha: l.lightmask_alpha,
        view_mtx: objs.cobjs[c].view_mtx,
        ..LightSet::default()
    };
    for (slot, a) in set.active.iter_mut().zip(&l.active) {
        *slot = a.map(|lobj| {
            let o = &l[lobj];
            ActiveLight {
                lobj,
                id: o.id,
                spec_id: o.spec_id,
                shininess: o.shininess,
                lvec: o.lvec,
            }
        });
    }
    let d = &mut objs.display;
    let n = d.light_sets.len() as u16;
    d.draws.push(if after_render_mode {
        Draw::LightsAfterRenderMode(n)
    } else {
        Draw::Lights(n)
    });
    d.light_sets.push(set);
}

/// [`lobj_setup_init`]'s light setup.
fn setup_lights(objs: &mut Objects, data: &Archives, c: CObjId) {
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
        // `setup_infinite_lightobj`, `setup_point_lightobj` and `setup_spot_lightobj` load
        // their GX objects from the light's position (a spot's also from its vector), which
        // reading brings a path-animated position into world space (`HSD_WObjGetPosition`).
        match ty {
            lobj::INFINITE => {
                lobj::position(objs, data, Some(l));
            }
            lobj::POINT => {
                objs.lobjs[l].hw_color = objs.lobjs[l].color;
                lobj::position(objs, data, Some(l));
            }
            lobj::SPOT => {
                lobj::position(objs, data, Some(l));
                lobj::light_vector(objs, data, Some(l));
            }
            lobj::AMBIENT => continue,
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
}

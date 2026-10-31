//! Lights' GX side (`lobj.c`): the GX light objects each light keeps, made when
//! `HSD_LObjSetupInit` gives the lights GX slots and loaded by `HSD_LObjSetup`.

use std::collections::HashMap;

use melee_sim::hsd::Objects;
use melee_sim::hsd::display::LightSet;
use melee_sim::hsd::lobj::{self, LObjId};
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;

use super::state::Lights;
use crate::gx::{Color, Gx, LightObj};

const LOBJ_DIFF_DIRTY: u16 = 1 << 7;
const LOBJ_SPEC_DIRTY: u16 = 1 << 8;
/// Word indexes into `LObj::params` (see melee-sim's lobj.rs).
const CUTOFF: usize = 0;
const SPOT_FUNC: usize = 1;
const REF_BR: usize = 2;
const REF_DIST: usize = 3;
const DIST_FUNC: usize = 4;

/// A light's GX objects and what HSD keeps of them (`lightobj`, `spec_lightobj`, `hw_color`,
/// the dirty flags and the shininess its specular attenuation was made for).
#[derive(Clone, Debug, Default)]
pub struct LightGx {
    pub light: LightObj,
    pub spec: LightObj,
    pub hw_color: Color,
    pub dirty: u16,
    pub shininess: f32,
    pub spec_id: u32,
    pub id: u32,
    /// The specular light's position or direction in view space (`lvec`).
    pub lvec: Vec3,
}

/// The lights GX has: each light's objects, and the active set of the last
/// `HSD_LObjSetupInit`.
#[derive(Default)]
pub struct LightState {
    pub lights: HashMap<LObjId, LightGx>,
    pub set: LightSet,
}

fn color(c: [u8; 4]) -> Color {
    Color::new(c[0], c[1], c[2], c[3])
}

/// A light's GX object as melee-sim keeps it (`__GXLightObjInt_struct`, reserved words zero).
fn light_obj(g: &lobj::GxLight) -> LightObj {
    LightObj {
        reserved: [0; 3],
        color: u32::from_be_bytes(g.color),
        a: g.a,
        k: g.k,
        lpos: g.lpos,
        ldir: g.ldir,
    }
}

fn param(o: &lobj::LObj, i: usize) -> f32 {
    f32::from_bits(o.params[i])
}

fn wobj_pos(objs: &Objects, w: Option<melee_sim::hsd::wobj::WObjId>) -> Option<Vec3> {
    w.map(|w| objs.wobjs[w.0].pos)
}

/// `HSD_LObjGetPosition` as the pass left the position.
fn position(objs: &Objects, l: LObjId) -> Vec3 {
    wobj_pos(objs, objs.lobjs[l].position).unwrap_or(Vec3::ZERO)
}

/// `HSD_LObjGetLightVector`: from the position to the interest, normalized.
fn light_vector(objs: &Objects, l: LObjId) -> Vec3 {
    let p = wobj_pos(objs, objs.lobjs[l].position).unwrap_or(Vec3::ZERO);
    let i = wobj_pos(objs, objs.lobjs[l].interest).unwrap_or(Vec3::ZERO);
    i.sdk_sub(p).sdk_normalize()
}

impl LightState {
    /// `HSD_LObjSetupInit`'s GX side for the set a [`Draw::Lights`] names: each active light's
    /// objects made for the view, then loaded.
    ///
    /// [`Draw::Lights`]: melee_sim::hsd::display::Draw::Lights
    pub fn setup_init(&mut self, gx: &mut Gx, objs: &Objects, set: &LightSet) {
        self.set = *set;
        let vmtx = set.view_mtx;
        for a in set.active.iter().flatten() {
            let o = &objs.lobjs[a.lobj];
            let ty = o.flags & lobj::TYPE_MASK;
            // The objects start as the light's memory has them, not as the renderer last
            // left this slot: a slot a scene freed and another took is a new light.
            let g = self.lights.entry(a.lobj).or_default();
            *g = LightGx {
                light: light_obj(&a.light),
                spec: light_obj(&a.spec),
                hw_color: color(a.light.color),
                dirty: 0,
                shininess: a.shininess,
                spec_id: a.spec_id,
                id: a.id,
                lvec: a.lvec,
            };
            match ty {
                lobj::INFINITE => setup_infinite(gx, objs, a.lobj, &vmtx, g),
                lobj::POINT => setup_point(gx, objs, a.lobj, &vmtx, g),
                lobj::SPOT => setup_spot(gx, objs, a.lobj, &vmtx, g),
                _ => continue,
            }
            let reflect = o.flags & (lobj::DIFFUSE | lobj::SPECULAR);
            if reflect & lobj::DIFFUSE != 0 {
                gx.init_light_color(&mut g.light, color(o.color));
                g.hw_color = color(o.color);
                g.dirty |= LOBJ_DIFF_DIRTY;
            }
            if a.spec_id != 0 {
                // `setup_spec_lightobj`.
                gx.init_light_color(&mut g.spec, color(o.color));
                g.shininess = 50.0;
                let x = 50.0f32 * 0.5;
                gx.init_light_attn(&mut g.spec, 0.0, 0.0, 1.0, x, 0.0, 1.0 - x);
                g.dirty |= LOBJ_SPEC_DIRTY;
            }
        }
        for a in set.active.iter().take(lobj::MAX_GXLIGHT - 1) {
            let Some(a) = a else { break };
            let c = color(objs.lobjs[a.lobj].color);
            let shininess = self
                .lights
                .get(&a.lobj)
                .map_or(a.shininess, |g| g.shininess);
            self.setup(gx, objs, a.lobj, c, shininess);
        }
    }

    /// `HSD_LObjSetup`: loads what changed of a light's objects.
    pub fn setup(&mut self, gx: &mut Gx, objs: &Objects, l: LObjId, c: Color, shininess: f32) {
        let o = &objs.lobjs[l];
        if o.flags & lobj::HIDDEN != 0 || o.flags & lobj::TYPE_MASK == lobj::AMBIENT {
            return;
        }
        let g = self.lights.entry(l).or_default();
        if o.flags & lobj::DIFFUSE != 0 {
            if g.hw_color != c {
                gx.init_light_color(&mut g.light, c);
                g.hw_color = c;
                g.dirty |= LOBJ_DIFF_DIRTY;
            }
            if g.dirty & LOBJ_DIFF_DIRTY != 0 {
                gx.load_light_obj_imm(&g.light, g.id);
                g.dirty &= !LOBJ_DIFF_DIRTY;
            }
        }
        if g.spec_id != 0 {
            if g.shininess.to_bits() != shininess.to_bits() {
                g.shininess = shininess;
                let k0 = shininess * 0.5;
                gx.init_light_attn(&mut g.spec, 0.0, 0.0, 1.0, k0, 0.0, 1.0 - k0);
                g.dirty |= LOBJ_SPEC_DIRTY;
            }
            if g.dirty & LOBJ_SPEC_DIRTY != 0 {
                gx.load_light_obj_imm(&g.spec, g.spec_id);
                g.dirty &= !LOBJ_SPEC_DIRTY;
            }
        }
    }

    /// `HSD_LObjSetupSpecularInit`: each specular light's half vector toward the joint.
    pub fn setup_specular_init(&mut self, gx: &mut Gx, objs: &Objects, pmtx: &Mtx34) {
        let jpos = Vec3::new(pmtx.m[0][3], pmtx.m[1][3], pmtx.m[2][3]);
        let cdir = jpos.sdk_normalize();
        for a in self
            .set
            .active
            .iter()
            .take(self.set.nb_active.max(0) as usize)
            .flatten()
        {
            let Some(g) = self.lights.get_mut(&a.lobj) else {
                continue;
            };
            if g.spec_id == 0 {
                continue;
            }
            let half = match objs.lobjs[a.lobj].flags & lobj::TYPE_MASK {
                lobj::POINT | lobj::SPOT => {
                    let ldir = jpos.sdk_sub(g.lvec).sdk_normalize();
                    ldir.sdk_add(cdir)
                }
                lobj::INFINITE => g.lvec.sdk_add(cdir),
                _ => panic!("a specular ambient light"),
            }
            .sdk_normalize();
            gx.init_light_dir(&mut g.spec, half.x, half.y, half.z);
            g.dirty |= LOBJ_SPEC_DIRTY;
        }
    }
}

/// `setup_infinite_lightobj`.
fn setup_infinite(gx: &mut Gx, objs: &Objects, l: LObjId, vmtx: &Mtx34, g: &mut LightGx) {
    let p = position(objs, l);
    let s = 1_048_576.0f32;
    let lpos = vmtx.transform(Vec3::new(p.x * s, p.y * s, p.z * s));
    let flags = objs.lobjs[l].flags;
    if flags & lobj::DIFFUSE != 0 {
        gx.init_light_pos(&mut g.light, lpos.x, lpos.y, lpos.z);
        gx.init_light_attn(&mut g.light, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    }
    if flags & lobj::SPECULAR != 0 {
        gx.init_light_pos(&mut g.spec, lpos.x, lpos.y, lpos.z);
    }
}

/// `setup_point_lightobj`.
fn setup_point(gx: &mut Gx, objs: &Objects, l: LObjId, vmtx: &Mtx34, g: &mut LightGx) {
    let o = &objs.lobjs[l];
    gx.init_light_color(&mut g.light, color(o.color));
    g.hw_color = color(o.color);
    let lpos = vmtx.transform(position(objs, l));
    gx.init_light_pos(&mut g.light, lpos.x, lpos.y, lpos.z);
    gx.init_light_pos(&mut g.spec, lpos.x, lpos.y, lpos.z);
    if o.flags & lobj::RAW_PARAM != 0 {
        let (k0, k1, k2) = (param(o, 3), param(o, 4), param(o, 5));
        gx.init_light_attn(&mut g.light, 1.0, 0.0, 0.0, k0, k1, k2);
    } else {
        let (br, dist, func) = (param(o, REF_BR), param(o, REF_DIST), o.params[DIST_FUNC]);
        gx.init_light_dist_attn(&mut g.light, dist, br, func);
        gx.init_light_spot(&mut g.light, 0.0, 0);
        gx.init_light_dist_attn(&mut g.spec, dist, br, func);
    }
}

/// `setup_spot_lightobj`.
fn setup_spot(gx: &mut Gx, objs: &Objects, l: LObjId, vmtx: &Mtx34, g: &mut LightGx) {
    let o = &objs.lobjs[l];
    let lpos = vmtx.transform(position(objs, l));
    let ldir = vmtx.rotate(light_vector(objs, l)).sdk_normalize();
    gx.init_light_pos(&mut g.light, lpos.x, lpos.y, lpos.z);
    gx.init_light_pos(&mut g.spec, lpos.x, lpos.y, lpos.z);
    gx.init_light_dir(&mut g.light, ldir.x, ldir.y, ldir.z);
    if o.flags & lobj::RAW_PARAM != 0 {
        let p = |i| param(o, i);
        gx.init_light_attn(&mut g.light, p(0), p(1), p(2), p(3), p(4), p(5));
    } else {
        let (br, dist, cutoff) = (param(o, REF_BR), param(o, REF_DIST), param(o, CUTOFF));
        let (spot, dist_func) = (o.params[SPOT_FUNC], o.params[DIST_FUNC]);
        gx.init_light_dist_attn(&mut g.light, dist, br, dist_func);
        gx.init_light_dist_attn(&mut g.spec, dist, br, dist_func);
        gx.init_light_spot(&mut g.light, cutoff, spot);
    }
}

/// The lights as `HSD_SetupChannelMode` sees them.
pub struct ChannelLights<'a> {
    pub state: &'a mut LightState,
    pub objs: &'a Objects,
}

impl Lights for ChannelLights<'_> {
    fn set(&self) -> &LightSet {
        &self.state.set
    }

    fn ambient(&self) -> Option<(Color, u16)> {
        let a = self.state.set.active[lobj::MAX_GXLIGHT - 1]?;
        let o = &self.objs.lobjs[a.lobj];
        Some((color(o.color), o.flags))
    }

    fn setup_specular(&mut self, gx: &mut Gx, i: usize, shininess: f32) {
        if let Some(a) = self.state.set.active[i] {
            let c = color(self.objs.lobjs[a.lobj].color);
            self.state.setup(gx, self.objs, a.lobj, c, shininess);
        }
    }
}

#[cfg(test)]
mod tests {
    use melee_data::archive::Archives;
    use melee_sim::hsd::{cobj, display};

    use super::*;

    /// A light to set up: flags, color, position, interest and parameters.
    struct Light {
        flags: u16,
        color: [u8; 4],
        pos: Vec3,
        interest: Option<Vec3>,
        /// A spot's cutoff and `GXSpotFn`, if it has them.
        spot: Option<(f32, i32)>,
        /// Distance attenuation: reference distance and brightness, `GXDistAttnFn`.
        dist: Option<(f32, f32, i32)>,
        /// Raw attenuation (`LOBJ_RAW_PARAM`): a and k.
        raw: Option<([f32; 3], [f32; 3])>,
    }

    const DIFF_SPEC: u16 = lobj::DIFFUSE | lobj::SPECULAR;
    const RAW: u16 = 1 << 6;

    /// Sets `lights` up with melee-sim's `HSD_LObjSetupInit` for one view, then with
    /// melee-render's from the light set it names, its objects cleared first so that the
    /// renderer computes every word the setup writes; each active light's objects must then be
    /// melee-sim's, bit for bit.
    fn check(lights: &[Light]) {
        let data = Archives::default();
        let mut objs = Objects::default();
        let c = cobj::alloc(&mut objs);
        // A view turned about two axes and moved, as a camera's.
        objs.cobjs[c].view_mtx = Mtx34 {
            m: [
                [0.866_025_4, 0.0, -0.5, 12.5],
                [0.171_010_07, 0.939_692_6, 0.296_198_13, -40.25],
                [0.469_846_3, -0.342_020_15, 0.813_798_3, -310.0],
            ],
        };
        for d in lights {
            let l = lobj::alloc(&mut objs.lobjs);
            lobj::set_flags(&mut objs.lobjs, Some(l), u32::from(d.flags));
            lobj::set_color(&mut objs.lobjs, l, d.color);
            lobj::set_position(&mut objs, l, d.pos);
            if let Some(i) = d.interest {
                lobj::set_interest(&mut objs, l, i);
            }
            if let Some((cutoff, func)) = d.spot {
                lobj::set_spot(&mut objs.lobjs, Some(l), cutoff, func);
            }
            if let Some((dist, br, func)) = d.dist {
                lobj::set_dist_attn(&mut objs.lobjs, Some(l), dist, br, func);
            }
            if let Some((a, k)) = d.raw {
                lobj::set_attn_a(&mut objs.lobjs, Some(l), a);
                lobj::set_attn_k(&mut objs.lobjs, Some(l), k);
            }
            lobj::add_current(&mut objs, Some(l));
        }
        display::lobj_setup_init(&mut objs, &data, c);
        let mut set = *objs.display.light_sets.last().expect("a light set");
        for a in set.active.iter_mut().flatten() {
            a.light = lobj::GxLight::default();
            a.spec = lobj::GxLight::default();
        }
        let (mut state, mut gx) = (LightState::default(), Gx::new());
        state.setup_init(&mut gx, &objs, &set);
        let mut checked = 0;
        for a in set.active.iter().flatten() {
            let o = &objs.lobjs[a.lobj];
            if o.flags & lobj::TYPE_MASK == lobj::AMBIENT {
                continue;
            }
            let g = &state.lights[&a.lobj];
            let ty = o.flags & lobj::TYPE_MASK;
            assert_eq!(g.light, light_obj(&o.light), "type {ty}'s light object");
            assert_eq!(g.spec, light_obj(&o.spec), "type {ty}'s specular object");
            checked += 1;
        }
        assert_eq!(
            checked,
            lights.len() - 1,
            "every light but the ambient one checked"
        );
    }

    fn ambient() -> Light {
        Light {
            flags: lobj::AMBIENT | lobj::DIFFUSE,
            color: [0x40, 0x40, 0xB3, 0xFF],
            pos: Vec3::ZERO,
            interest: None,
            spot: None,
            dist: None,
            raw: None,
        }
    }

    /// melee-render recomputes each light's objects (with the blended view when high refresh
    /// blends) rather than taking melee-sim's. No scene milestone 5 reaches has a spot light
    /// (`hsd::lobj`'s disc audit), so no checked frame holds the renderer's spot path, and
    /// this does: every light type with each attenuation, the spot with every `GXSpotFn` and
    /// `GXDistAttnFn` (the disc's spot lights' values among them: a 30 or 45 degree cosine
    /// squared cutoff, steep distance attenuation at 10,000 or 600).
    #[test]
    fn setup_matches_melee_sim() {
        let spot = |flags, cutoff, spot_func, dist, dist_func| Light {
            flags: lobj::SPOT | flags,
            color: [0xFF, 0xFF, 0xCC, 0xFF],
            pos: Vec3::new(0.0, -30.0, 50.0),
            interest: Some(Vec3::new(4.0, 680.0, -20.0)),
            spot: Some((cutoff, spot_func)),
            dist: Some((dist, 0.99, dist_func)),
            raw: None,
        };
        for spot_func in 0..=6 {
            for dist_func in 0..=3 {
                check(&[
                    spot(DIFF_SPEC, 30.0, spot_func, 10_000.0, dist_func),
                    spot(lobj::DIFFUSE, 45.0, spot_func, 600.0, dist_func),
                    spot(lobj::SPECULAR, 12.5, spot_func, 75.0, dist_func),
                    ambient(),
                ]);
            }
        }
        check(&[
            Light {
                flags: lobj::INFINITE | DIFF_SPEC,
                color: [0xFF; 4],
                pos: Vec3::new(0.57, 0.57, 0.57),
                interest: None,
                spot: None,
                dist: None,
                raw: None,
            },
            Light {
                flags: lobj::POINT | DIFF_SPEC,
                color: [0xB3, 0xB3, 0xB3, 0xFF],
                pos: Vec3::new(0.0, -58.6, -71.1),
                interest: None,
                spot: None,
                dist: Some((30.0, 0.99, 3)),
                raw: None,
            },
            Light {
                flags: lobj::POINT | lobj::DIFFUSE | RAW,
                color: [0xCC, 0xCC, 0xCC, 0xFF],
                pos: Vec3::new(-6.39, 18.75, 65.25),
                interest: None,
                spot: None,
                dist: None,
                raw: Some(([1.0, 0.0, 0.0], [0.5, 0.02, 0.001])),
            },
            Light {
                flags: lobj::SPOT | DIFF_SPEC | RAW,
                color: [0x80, 0xFF, 0x40, 0xFF],
                pos: Vec3::new(10.0, 79.4, 104.21),
                interest: Some(Vec3::ZERO),
                spot: None,
                dist: None,
                raw: Some(([-3.2, 0.0, 4.2], [1.0, 0.003, 0.0001])),
            },
            ambient(),
        ]);
    }
}

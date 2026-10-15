//! Drawing joints' display objects (`displayfunc.c`, `dobj.c`, `mobj.c`, `tobj.c`,
//! `pobj.c`, and the fighters' material class, `ftmaterial.c`): the material set up in GX,
//! then each shape's matrices loaded and its display list called.

use melee_data::archive::{Archives, DataRef};
use melee_sim::game::state::Game;
use melee_sim::hsd::Objects;
use melee_sim::hsd::display::{Draw, TRSP_OPA, TRSP_TEXEDGE, TRSP_XLU};
use melee_sim::hsd::dobj::DObjId;
use melee_sim::hsd::jobj::{self, JObjId};
use melee_sim::hsd::mobj::{self, MObjClass, MObjId};
use melee_sim::hsd::pobj::{self, Binding, PObjId};
use melee_sim::hsd::tobj::{self, ImageRef, TObjId};
use melee_sim::mtx::Mtx34;

use super::Hsd;
use super::lobj::ChannelLights;
use super::state::States;
use crate::gx::enums::*;
use crate::gx::{Color, Gx, TexObj, TlutObj};
use crate::mem::{Dynamic, Space};

/// What drawing reads: the game as the display pass left it, its data, and the address map.
pub struct Ctx<'a> {
    pub gx: &'a mut Gx,
    pub hsd: &'a mut Hsd,
    pub game: &'a Game,
    pub data: &'a Archives,
    pub space: &'a Space,
}

const RENDER_SHADOW: u32 = 1 << 26;
const RENDER_TOON: u32 = 1 << 12;
const RENDER_SPECULAR: u32 = 1 << 3;
const TEX_BUMP: u32 = 1 << 24;
const JOBJ_BILLBOARD_FIELD: u32 = 0xE00;
const JOBJ_BILLBOARD: u32 = 0x200;
const JOBJ_VBILLBOARD: u32 = 0x400;
const JOBJ_HBILLBOARD: u32 = 0x600;
const JOBJ_RBILLBOARD: u32 = 0x800;
const JOBJ_PBILLBOARD: u32 = 1 << 13;
/// `FLT_EPSILON` as pobj.c compares envelope weights with.
const FLT_EPSILON: f32 = 1.192_092_9e-7;

fn mtx(m: &Mtx34) -> [[f32; 4]; 3] {
    m.m
}

fn color(c: melee_sim::hsd::tobj::Color) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

/// The column `c` of a matrix (`HSD_MtxColVec`).
fn col(m: &Mtx34, c: usize) -> melee_sim::vec::Vec3 {
    melee_sim::vec::Vec3::new(m.m[0][c], m.m[1][c], m.m[2][c])
}

fn set_col(m: &mut Mtx34, c: usize, v: melee_sim::vec::Vec3) {
    m.m[0][c] = v.x;
    m.m[1][c] = v.y;
    m.m[2][c] = v.z;
}

/// `HSD_MtxColMag`.
fn col_mag(m: &Mtx34, c: usize) -> f32 {
    col(m, c).sdk_length()
}

/// `HSD_JObjMakePositionMtx`: the joint's matrix in view space, turned toward the camera for
/// a billboard.
pub fn position_mtx(objs: &Objects, j: JObjId, vmtx: &Mtx34) -> Mtx34 {
    use melee_sim::vec::Vec3;
    let o = &objs.jobjs[j];
    let src = vmtx.concat(&o.mtx);
    let field = o.flags & JOBJ_BILLBOARD_FIELD;
    if field == 0 {
        return src;
    }
    let pb = o.flags & JOBJ_PBILLBOARD != 0;
    let mut dst = Mtx34::default();
    match field {
        JOBJ_BILLBOARD => {
            let sx = col_mag(&src, 0);
            let mut sz = col_mag(&src, 2);
            let mut ay = col(&src, 1);
            let mut sy = ay.sdk_length();
            let pos = col(&src, 3);
            let (ax, az) = if pb {
                let ax = pos.sdk_cross(ay);
                ay = ax.sdk_cross(pos);
                sz /= -pos.sdk_length();
                (ax, pos)
            } else {
                let z = Vec3::new(0.0, 0.0, 1.0);
                let ax = ay.sdk_cross(z);
                ay = z.sdk_cross(ax);
                sz /= z.sdk_length();
                (ax, z)
            };
            let sx = sx / ax.sdk_length();
            sy /= ay.sdk_length();
            set_col(&mut dst, 0, Vec3::new(sx * ax.x, sx * ax.y, sx * ax.z));
            set_col(&mut dst, 1, Vec3::new(sy * ay.x, sy * ay.y, sy * ay.z));
            set_col(&mut dst, 2, Vec3::new(sz * az.x, sz * az.y, sz * az.z));
            set_col(&mut dst, 3, pos);
        }
        JOBJ_VBILLBOARD => {
            let pos = col(&src, 3);
            let ay = col(&src, 1);
            let mut sx = col_mag(&src, 0);
            let mut sz = col_mag(&src, 2);
            let ax = if pb {
                pos.sdk_cross(ay)
            } else {
                ay.sdk_cross(Vec3::new(0.0, 0.0, 1.0))
            };
            let az = ax.sdk_cross(ay);
            sx /= ax.sdk_length();
            sz /= az.sdk_length();
            set_col(&mut dst, 0, Vec3::new(sx * ax.x, sx * ax.y, sx * ax.z));
            set_col(&mut dst, 1, ay);
            set_col(&mut dst, 2, Vec3::new(sz * az.x, sz * az.y, sz * az.z));
            set_col(&mut dst, 3, pos);
        }
        JOBJ_HBILLBOARD => {
            let pos = col(&src, 3);
            let ax = col(&src, 0);
            let mut sy = col_mag(&src, 1);
            let mut sz = col_mag(&src, 2);
            let az = if pb {
                let y = melee_sim::math::sqrt(pos.x * pos.x + pos.z * pos.z);
                let uy = Vec3::new(-pos.y / y * pos.x, y, -pos.y / y * pos.z);
                ax.sdk_cross(uy)
            } else {
                ax.sdk_cross(Vec3::new(0.0, 1.0, 0.0))
            };
            let ay = az.sdk_cross(ax);
            sy /= ay.sdk_length();
            sz /= az.sdk_length();
            set_col(&mut dst, 0, ax);
            set_col(&mut dst, 1, Vec3::new(sy * ay.x, sy * ay.y, sy * ay.z));
            set_col(&mut dst, 2, Vec3::new(sz * az.x, sz * az.y, sz * az.z));
            set_col(&mut dst, 3, pos);
        }
        JOBJ_RBILLBOARD => {
            let scl = Mtx34::scale(col_mag(&src, 0), col_mag(&src, 1), col_mag(&src, 2));
            let mut rot = Mtx34::rotation(melee_sim::mtx::Axis::Z, o.rotate.z);
            for r in 0..3 {
                rot.m[r][3] = src.m[r][3];
            }
            dst = rot.concat(&scl);
        }
        _ => panic!("unkown type of billboard."),
    }
    dst
}

/// The texture chain a material draws with: the toon texture first for a toon material, the
/// shadow textures (`shadows`, as the display pass had them at the draw) after its own for a
/// shadowed one (`HSD_MObjSetup`).
pub fn material_chain(
    objs: &Objects,
    m: MObjId,
    rendermode: u32,
    shadows: &[TObjId],
) -> Vec<TObjId> {
    let mut chain: Vec<TObjId> = objs.tobjs.chain(objs.mobjs[m].tobj).collect();
    if rendermode & RENDER_SHADOW != 0 {
        chain.extend_from_slice(shadows);
    }
    if rendermode & RENDER_TOON != 0
        && let Some(toon) = objs.mobjs.toon
        && objs.tobjs[toon].imagedesc.is_some()
    {
        chain.insert(0, toon);
    }
    chain
}

/// An image a texture shows: where its texels are, its size, format, mipmapping and levels
/// (`HSD_ImageDesc`).
#[derive(Clone, Copy, Debug)]
pub struct Image {
    pub ptr: u32,
    pub width: u16,
    pub height: u16,
    pub format: u32,
    pub mipmap: bool,
    pub min_lod: f32,
    pub max_lod: f32,
}

impl Ctx<'_> {
    fn objs(&self) -> &Objects {
        &self.game.objs
    }

    /// The image a texture shows, with its texels' address.
    fn image(&mut self, t: TObjId) -> Option<Image> {
        let objs = self.objs();
        match objs.tobjs[t].imagedesc {
            Some(ImageRef::Data(d)) => {
                if let Some(img) = self.reflection_image(d) {
                    return Some(img);
                }
                let data = self.data;
                Some(Image {
                    ptr: self.space.addr(data, data.ptr(d)?),
                    width: data.u16(d.at(4)),
                    height: data.u16(d.at(6)),
                    format: data.u32(d.at(8)),
                    mipmap: data.u32(d.at(0xC)) != 0,
                    min_lod: data.f32(d.at(0x10)),
                    max_lod: data.f32(d.at(0x14)),
                })
            }
            Some(ImageRef::Shadow(s)) => {
                // A shadow's texture keeps its image in the shadow.
                let img = objs.shadows[s.0].image;
                let size = crate::gx::get_tex_buffer_size(img.width, img.height, img.format, 0, 0);
                Some(Image {
                    ptr: self.space.dynamic(Dynamic::Shadow(s.0), size),
                    width: img.width,
                    height: img.height,
                    format: img.format,
                    mipmap: false,
                    min_lod: 0.0,
                    max_lod: 0.0,
                })
            }
            Some(ImageRef::Static(at)) => self.static_image(at),
            Some(ImageRef::Screen(n)) => self.screen_image(n),
            // Listed one by one, so a new kind of image fails to build here until it's drawn.
            Some(other @ (ImageRef::Refract(_) | ImageRef::RefractFrame)) => {
                self.hsd.note(format!("image {other:?}"));
                None
            }
            None => None,
        }
    }

    /// `TObjSetupMtx`'s loads.
    fn tobj_setup_mtx(&mut self, t: TObjId) {
        let objs = &self.game.objs;
        let o = &objs.tobjs[t];
        let coord = o.flags & tobj::COORD_MASK;
        if coord == tobj::COORD_TOON {
            return;
        }
        let m = o.mtx;
        let mtxid = self.hsd.assigned.get(objs, t).mtxid;
        match coord {
            tobj::COORD_REFLECTION => {
                let mut r = [[0f32; 4]; 3];
                for (i, row) in r.iter_mut().enumerate() {
                    let src = m.m[i];
                    row[0] = 0.5 * src[0];
                    row[1] = -0.5 * src[1];
                    row[2] = 0.0;
                    row[3] = 0.5 * src[0] + 0.5 * src[1] + src[2] + src[3];
                }
                self.gx.load_tex_mtx_imm(&r, mtxid, GX_MTX3X4);
            }
            tobj::COORD_HILIGHT => {
                let light = melee_sim::hsd::lobj::current_by_type(&objs.lobjs, 1);
                let vmtx = self.hsd.view_mtx();
                match light {
                    Some(l) => {
                        let lo = &objs.lobjs[l];
                        let p = lo.position.map(|w| objs.wobjs[w.0].pos).unwrap_or_default();
                        let i = lo.interest.map(|w| objs.wobjs[w.0].pos).unwrap_or_default();
                        let mut ldir = vmtx.rotate(i.sdk_sub(p).sdk_normalize());
                        ldir.z += -1.0;
                        let h = ldir.sdk_normalize();
                        let half = [h.x * -0.5, h.y * -0.5, h.z * -0.5];
                        let r = [
                            [
                                m.m[0][0] * half[0],
                                m.m[0][0] * half[1],
                                m.m[0][0] * half[2],
                                m.m[0][0] * 0.5 + m.m[0][3],
                            ],
                            [
                                m.m[1][0] * half[0],
                                m.m[1][0] * half[1],
                                m.m[1][0] * half[2],
                                m.m[1][0] * 0.5 + m.m[1][3],
                            ],
                            [0.0, 0.0, 0.0, 1.0],
                        ];
                        self.gx.load_tex_mtx_imm(&r, mtxid, GX_MTX3X4);
                    }
                    None => {
                        let zero = [[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], [0.0; 4]];
                        self.gx.load_tex_mtx_imm(&zero, mtxid, GX_MTX3X4);
                    }
                }
            }
            tobj::COORD_SHADOW => {
                let inv = self.hsd.inv_view_mtx();
                let r = m.concat(&inv);
                self.gx.load_tex_mtx_imm(&r.m, mtxid, GX_MTX3X4);
            }
            _ => {
                if o.flags & TEX_BUMP != 0 {
                    self.gx.load_tex_mtx_imm(&m.m[..2], mtxid, GX_MTX2X4);
                } else {
                    self.gx.load_tex_mtx_imm(&m.m, mtxid, GX_MTX3X4);
                }
            }
        }
    }

    /// `HSD_TObjSetup`'s GX side: each texture's matrix and image loaded, palettes shared
    /// between textures whose entry counts match (as the original's comparison does).
    pub fn tobj_setup(&mut self, chain: &[TObjId]) {
        if chain.is_empty() {
            self.hsd.assigned = Default::default();
            return;
        }
        let num = self.hsd.assigned.assign(&self.game.objs, chain);
        if num > 0 {
            self.hsd
                .tev
                .register_tex_gen(tobj::index_to_texcoord(num - 1));
        }
        let mut tluts: Vec<(u16, u32)> = Vec::new();
        let (mut tlut_name, mut big_tlut_name) = (GX_TLUT0, GX_BIGTLUT0);
        for &t in chain {
            let id = self.hsd.assigned.get(&self.game.objs, t).id;
            if id == tobj::GX_TEXMAP_NULL {
                continue;
            }
            self.tobj_setup_mtx(t);
            let Some(img) = self.image(t) else { continue };
            let o = &self.game.objs.tobjs[t];
            let data = self.data;
            let (mut min_filter, lod_bias, bias_clamp, edge_lod, aniso) = match o.lod {
                Some(l) => (
                    data.u32(l),
                    data.f32(l.at(4)),
                    data.u8(l.at(8)),
                    data.u8(l.at(9)),
                    data.u32(l.at(0xC)),
                ),
                None => (GX_LIN_MIP_LIN, 0.0, GX_FALSE, GX_FALSE, GX_ANISO_1),
            };
            let mut texobj = TexObj::default();
            let mipmap = u8::from(img.mipmap);
            match img.format {
                GX_TF_C4 | GX_TF_C8 | GX_TF_C14X2 => {
                    let tlut = if o.tlut_no != 0xFF {
                        o.tluttbl
                            .and_then(|tbl| data.ptr(tbl.table.at(4 * u32::from(o.tlut_no))))
                    } else {
                        o.tlut
                    };
                    let Some(tlut) = tlut else {
                        self.hsd
                            .note("a color-indexed texture without a palette".into());
                        continue;
                    };
                    let n_entries = data.u16(tlut.at(0xC));
                    let name = if let Some(&(_, name)) = tluts.iter().find(|x| x.0 == n_entries) {
                        name
                    } else if tluts.len() < 8 {
                        let name = if n_entries >= 256 {
                            big_tlut_name += 1;
                            big_tlut_name - 1
                        } else {
                            tlut_name += 1;
                            tlut_name - 1
                        };
                        let mut tlutobj = TlutObj::default();
                        let lut = data.ptr(tlut).map_or(0, |p| self.space.addr(data, p));
                        self.gx
                            .init_tlut_obj(&mut tlutobj, lut, data.u32(tlut.at(4)), n_entries);
                        self.gx.load_tlut(&tlutobj, name);
                        tluts.push((n_entries, name));
                        name
                    } else {
                        GX_TLUT0
                    };
                    self.gx.init_tex_obj_ci(
                        &mut texobj,
                        img.ptr,
                        img.width,
                        img.height,
                        img.format,
                        o.wrap_s,
                        o.wrap_t,
                        mipmap,
                        name,
                    );
                    if min_filter == GX_LIN_MIP_LIN {
                        min_filter = GX_LIN_MIP_NEAR;
                    }
                }
                _ => {
                    self.gx.init_tex_obj(
                        &mut texobj,
                        img.ptr,
                        img.width,
                        img.height,
                        img.format,
                        o.wrap_s,
                        o.wrap_t,
                        mipmap,
                    );
                }
            }
            if !img.mipmap {
                min_filter &= 1;
            }
            self.gx.init_tex_obj_lod(
                &mut texobj,
                min_filter,
                o.mag_filt,
                img.min_lod,
                img.max_lod,
                lod_bias,
                bias_clamp,
                edge_lod,
                aniso,
            );
            self.gx.load_tex_obj(&texobj, id);
        }
    }

    /// `HSD_TObjSetupTextureCoordGen`.
    pub fn tobj_setup_coord_gen(&mut self, chain: &[TObjId]) {
        for &t in chain {
            let o = &self.game.objs.tobjs[t];
            let r = self.hsd.assigned.get(&self.game.objs, t);
            if r.id == tobj::GX_TEXMAP_NULL {
                continue;
            }
            let coord = o.flags & tobj::COORD_MASK;
            if o.flags & TEX_BUMP != 0 {
                self.coord_gen(t);
                // `setupTextureCoordGenBump`.
                let mask = self.hsd.lights.set.mask_diffuse;
                let i = (0..8).find(|i| mask & (1 << i) != 0).unwrap_or(0);
                self.gx.set_tex_coord_gen(
                    r.coord + 1,
                    GX_TG_BUMP0 + i,
                    tobj::texcoord_to_texgen_src(r.coord),
                    GX_IDENTITY,
                );
            } else if coord == tobj::COORD_TOON {
                self.gx
                    .set_tex_coord_gen(r.coord, GX_TG_SRTG, o.src, GX_IDENTITY);
            } else {
                self.coord_gen(t);
            }
        }
    }

    /// `setupTextureCoordGen`.
    fn coord_gen(&mut self, t: TObjId) {
        let o = &self.game.objs.tobjs[t];
        let r = self.hsd.assigned.get(&self.game.objs, t);
        match o.flags & tobj::COORD_MASK {
            tobj::COORD_SHADOW => self.gx.set_tex_coord_gen2(
                r.coord,
                GX_TG_MTX3X4,
                GX_TG_POS,
                GX_PNMTX0,
                GX_DISABLE,
                r.mtxid,
            ),
            tobj::COORD_REFLECTION | tobj::COORD_HILIGHT => self.gx.set_tex_coord_gen2(
                r.coord,
                GX_TG_MTX3X4,
                GX_TG_NRM,
                GX_TEXMTX0,
                GX_ENABLE,
                r.mtxid,
            ),
            _ if o.flags & TEX_BUMP != 0 => {
                self.gx
                    .set_tex_coord_gen(r.coord, GX_TG_MTX2X4, o.src, r.mtxid)
            }
            _ => self.gx.set_tex_coord_gen2(
                r.coord,
                GX_TG_MTX2X4,
                o.src,
                GX_IDENTITY,
                GX_DISABLE,
                r.mtxid,
            ),
        }
    }

    /// `HSD_MObjSetup` for HAL's material class: colors, textures, TEV and render mode, with
    /// `rendermode_extra` and the pixel-engine settings `pe_override` for callers that change
    /// them.
    pub fn mobj_setup(
        &mut self,
        m: MObjId,
        rendermode_extra: u32,
        pe_override: Option<mobj::PeDesc>,
    ) {
        let (_, rendermode) = self.mobj_setup_tev_part(m);
        let pe = pe_override.or(self.game.objs.mobjs[m].pe);
        self.mobj_setup_render_mode(rendermode | rendermode_extra, pe);
    }

    /// `HSD_MObjSetup` up to the material's TEV (`setup_tev`), which every class's setup
    /// shares: the material's colors, its textures and their coordinates, then its compiled
    /// and volatile stages. Returns the compiled stages (`mobj->tevdesc`, `mobj->texp`) and
    /// the material's render mode.
    pub fn mobj_setup_tev_part(&mut self, m: MObjId) -> (super::texp::TevDesc, u32) {
        self.hsd.tev.init_tev();
        let o = &self.game.objs.mobjs[m];
        let rendermode = o.rendermode;
        let mat = o.mat.expect("a material without its colors");
        let c = |c: melee_sim::hsd::tobj::Color| color(c);
        self.hsd.state.set_material_color(
            c(mat.ambient),
            c(mat.diffuse),
            c(mat.specular),
            mat.alpha,
        );
        if rendermode & RENDER_SPECULAR != 0 {
            self.hsd.state.set_material_shininess(mat.shininess);
        }
        let chain = material_chain(self.objs(), m, rendermode, &self.hsd.shadows);
        self.tobj_setup(&chain);
        self.tobj_setup_coord_gen(&chain);
        let desc = self.setup_tev(m, &chain, rendermode);
        (desc, rendermode)
    }

    /// The end of `HSD_MObjSetup`: `HSD_SetupRenderModeWithCustomPE`.
    pub fn mobj_setup_render_mode(&mut self, rendermode: u32, pe: Option<mobj::PeDesc>) {
        let objs = &self.game.objs;
        let mut lights = ChannelLights {
            state: &mut self.hsd.lights,
            objs,
        };
        States {
            state: &mut self.hsd.state,
            tev: &mut self.hsd.tev,
        }
        .setup_render_mode(self.gx, rendermode, pe.as_ref(), &mut lights);
    }

    /// `MObjSetupTev`: the material's compiled stages (`mobj->tevdesc`, as its last compile
    /// made them from its textures then), then the volatile ones from its textures now.
    fn setup_tev(&mut self, m: MObjId, chain: &[TObjId], rendermode: u32) -> super::texp::TevDesc {
        let objs = &self.game.objs;
        let desc = match &objs.mobjs[m].tev {
            Some(d) => d.clone(),
            None => {
                self.hsd.note_once("a material never compiled");
                super::texp_make::compile_material(objs, m, chain)
            }
        };
        super::texp_make::mobj_setup_tev(
            self.gx,
            &mut self.hsd.tev,
            objs,
            &self.hsd.assigned,
            &desc,
            chain,
            rendermode,
        );
        desc
    }

    /// `HSD_PObjDisp`: culling, the shape's matrices, then its display list.
    fn pobj_disp(&mut self, j: JObjId, p: PObjId, vmtx: &Mtx34, pmtx: &Mtx34, rendermode: u32) {
        let objs = &self.game.objs;
        let po = &objs.pobjs[p];
        let cull = match po.flags & (3 << 14) {
            0 => GX_CULL_NONE,
            0x4000 => GX_CULL_FRONT,
            0x8000 => GX_CULL_BACK,
            _ => return,
        };
        self.hsd.state.set_cull_mode(self.gx, cull);
        match po.ty() {
            pobj::SKIN => match po.u {
                Binding::Skin(Some(s)) => self.setup_shared_vtx_mtx(j, s, vmtx, pmtx, rendermode),
                _ => self.setup_rigid_mtx(j, pmtx, rendermode),
            },
            pobj::SHAPEANIM => self.setup_rigid_mtx(j, pmtx, rendermode),
            pobj::ENVELOPE => self.setup_envelope_mtx(j, p, vmtx, rendermode),
            _ => {}
        }
        let objs = &self.game.objs;
        let po = &objs.pobjs[p];
        if po.ty() == pobj::SHAPEANIM {
            self.hsd.note("a shape animation (drawShapeAnim)".into());
            return;
        }
        let (Some(verts), Some(display)) = (po.verts, po.display) else {
            return;
        };
        let n_display = po.n_display;
        self.setup_array_desc(verts);
        self.setup_vtx_desc(verts);
        let addr = self.space.addr(self.data, display);
        if self.hsd.debug_dl {
            let mut b = vec![0u8; (usize::from(n_display) << 5).min(4096)];
            self.space.read(self.data, addr, &mut b);
            let a = self.data.get(display.archive);
            let relocs: Vec<u32> = a
                .relocs()
                .iter()
                .copied()
                .filter(|&r| r >= display.offset && r < display.offset + b.len() as u32)
                .collect();
            eprintln!("DL {display:?} n {n_display} relocs {relocs:x?} {:02x?}", b);
        }
        self.gx.call_display_list(addr, u32::from(n_display) << 5);
    }

    /// `GetSetupFlags`: (normals, normal projection).
    fn setup_flags(&self, j: JObjId, rendermode: u32) -> (bool, bool) {
        if rendermode & RENDER_SHADOW != 0 {
            return (false, false);
        }
        let objs = self.objs();
        let mut normal = objs.jobjs[j].flags & jobj::LIGHTING != 0;
        let chain = &self.hsd.tobj_chain;
        let by = |ty: u32| {
            chain
                .iter()
                .any(|&t| objs.tobjs[t].flags & tobj::COORD_MASK == ty)
        };
        let proj = by(tobj::COORD_REFLECTION) || by(tobj::COORD_HILIGHT);
        normal |= proj;
        (normal, proj)
    }

    /// `SetupRigidModelMtx` (its matrix mark never matches: `HSD_PObjSetMtxMark` stores
    /// nothing).
    fn setup_rigid_mtx(&mut self, j: JObjId, pmtx: &Mtx34, rendermode: u32) {
        self.gx.set_current_mtx(GX_PNMTX0);
        self.gx.load_pos_mtx_imm(&mtx(pmtx), GX_PNMTX0);
        let (normal, proj) = self.setup_flags(j, rendermode);
        if normal {
            let n = pmtx.hsd_inverse_transpose();
            if self.objs().jobjs[j].flags & jobj::LIGHTING != 0 {
                self.gx.load_nrm_mtx_imm(&mtx(&n), GX_PNMTX0);
            }
            if proj {
                self.gx.load_tex_mtx_imm(&n.m, GX_TEXMTX0, GX_MTX3X4);
            }
        }
    }

    /// `SetupSharedVtxModelMtx`.
    fn setup_shared_vtx_mtx(
        &mut self,
        j: JObjId,
        s: JObjId,
        vmtx: &Mtx34,
        pmtx: &Mtx34,
        rendermode: u32,
    ) {
        let (normal, proj) = self.setup_flags(j, rendermode);
        let lighting = self.objs().jobjs[j].flags & jobj::LIGHTING != 0;
        self.gx.set_current_mtx(GX_PNMTX0);
        self.gx.load_pos_mtx_imm(&mtx(pmtx), GX_PNMTX0);
        if normal {
            let n0 = pmtx.hsd_inverse_transpose();
            if lighting {
                self.gx.load_nrm_mtx_imm(&mtx(&n0), GX_PNMTX0);
            }
            if proj {
                self.gx.load_tex_mtx_imm(&n0.m, GX_TEXMTX0, GX_MTX3X4);
            }
        }
        let m = vmtx.concat(&self.objs().jobjs[s].mtx);
        self.gx.load_pos_mtx_imm(&mtx(&m), GX_PNMTX1);
        if normal {
            let n1 = m.hsd_inverse_transpose();
            if lighting {
                self.gx.load_nrm_mtx_imm(&mtx(&n1), GX_PNMTX1);
            }
            if proj {
                self.gx.load_tex_mtx_imm(&n1.m, GX_TEXMTX1, GX_MTX3X4);
            }
        }
    }

    /// `SetupEnvelopeModelMtx`.
    fn setup_envelope_mtx(&mut self, j: JObjId, p: PObjId, vmtx: &Mtx34, rendermode: u32) {
        let (normal, proj) = self.setup_flags(j, rendermode);
        let objs = &self.game.objs;
        let jo = &objs.jobjs[j];
        let lighting = jo.flags & jobj::LIGHTING != 0;
        // `_HSD_mkEnvelopeModelNodeMtx`.
        let right = if jo.flags & jobj::SKELETON_ROOT != 0 {
            None
        } else {
            let x = std::iter::successors(Some(j), |&x| objs.jobjs[x].parent)
                .find(|&x| objs.jobjs[x].flags & (jobj::SKELETON | jobj::SKELETON_ROOT) != 0)
                .expect("an enveloped shape's joint without a skeleton");
            let xo = &objs.jobjs[x];
            Some(if x == j {
                xo.envelopemtx
                    .expect("a skeleton without an envelope matrix")
                    .inverse()
                    .unwrap_or_default()
            } else if xo.flags & jobj::SKELETON_ROOT != 0 {
                xo.mtx.hsd_inverse_concat(&jo.mtx)
            } else {
                let n = xo.mtx.concat(&xo.envelopemtx.expect("no envelope matrix"));
                n.hsd_inverse_concat(&jo.mtx)
            })
        };
        let Binding::Envelopes(envs) = &objs.pobjs[p].u else {
            return;
        };
        let mut loads = Vec::with_capacity(envs.len().min(10));
        for (i, env) in envs.iter().take(10).enumerate() {
            let first = env.first().expect("an empty envelope");
            let m = if first.weight >= 1.0 - FLT_EPSILON {
                let e = &objs.jobjs[first.jobj.expect("an envelope without its joint")];
                match right {
                    Some(_) => e.mtx.concat(&e.envelopemtx.unwrap_or(Mtx34::IDENTITY)),
                    None => e.mtx,
                }
            } else {
                let mut acc = Mtx34::default();
                for e in env {
                    let jp = &objs.jobjs[e.jobj.expect("an envelope without its joint")];
                    let t = jp.mtx.concat(&jp.envelopemtx.unwrap_or(Mtx34::IDENTITY));
                    acc = t.scaled_add(&acc, e.weight);
                }
                acc
            };
            let m = match &right {
                Some(r) => m.concat(r),
                None => m,
            };
            loads.push((i as u32, vmtx.concat(&m)));
        }
        for (i, tmp) in loads {
            let id = i * 3;
            self.gx.load_pos_mtx_imm(&mtx(&tmp), id);
            if normal {
                let n = tmp.hsd_inverse_transpose();
                if lighting {
                    self.gx.load_nrm_mtx_imm(&mtx(&n), id);
                }
                if proj {
                    self.gx
                        .load_tex_mtx_imm(&n.m, tobj::index_to_texmtx(i), GX_MTX3X4);
                }
            }
        }
    }

    /// `setupArrayDesc`: the arrays of a vertex description, unless it was the last one set.
    fn setup_array_desc(&mut self, verts: DataRef) {
        if self.hsd.prev_array == Some(verts) {
            return;
        }
        let data = self.data;
        let mut d = verts;
        loop {
            let attr = data.u32(d);
            if attr == GX_VA_NULL {
                break;
            }
            if data.u32(d.at(4)) != GX_DIRECT {
                let base = data.ptr(d.at(0x14)).map_or(0, |p| self.space.addr(data, p));
                self.gx.set_array(attr, base, data.u16(d.at(0x12)) as u8);
            }
            d = d.at(0x18);
        }
        self.hsd.prev_array = Some(verts);
    }

    /// `setupVtxDesc`.
    fn setup_vtx_desc(&mut self, verts: DataRef) {
        if self.hsd.prev_vtxdesc == Some(verts) {
            return;
        }
        let data = self.data;
        self.gx.clear_vtx_desc();
        let mut d = verts;
        loop {
            let attr = data.u32(d);
            if attr == GX_VA_NULL {
                break;
            }
            self.gx.set_vtx_desc(attr, data.u32(d.at(4)));
            if !(GX_VA_PNMTXIDX..=GX_VA_TEX7MTXIDX).contains(&attr) {
                self.gx.set_vtx_attr_fmt(
                    GX_VTXFMT0,
                    attr,
                    data.u32(d.at(8)),
                    data.u32(d.at(0xC)),
                    data.u8(d.at(0x10)),
                );
            }
            d = d.at(0x18);
        }
        self.hsd.prev_vtxdesc = Some(verts);
    }

    /// `HSD_ClearVtxDesc`.
    pub fn clear_vtx_desc(&mut self) {
        self.gx.clear_vtx_desc();
        self.hsd.prev_array = None;
        self.hsd.prev_vtxdesc = None;
    }

    /// Draws a run of [`Draw::Shape`]s of one joint and pass, `HSD_JObjDispSub` for the
    /// display objects the pass drew, each through `HSD_DObjDisp`.
    pub fn joint_shapes(&mut self, run: &[Draw]) {
        let Some(Draw::Shape {
            jobj: j,
            trsp: _,
            rendermode,
            vmtx,
            ..
        }) = run.first()
        else {
            return;
        };
        let (j, rendermode) = (*j, *rendermode);
        let vmtx = vmtx.unwrap_or_else(|| self.hsd.view_mtx());
        let pmtx = position_mtx(self.objs(), j, &vmtx);
        if rendermode & RENDER_SHADOW == 0 && self.objs().jobjs[j].flags & jobj::SPECULAR != 0 {
            let objs = &self.game.objs;
            self.hsd.lights.setup_specular_init(self.gx, objs, &pmtx);
        }
        let mut i = 0;
        while i < run.len() {
            let Draw::Shape { dobj: d, .. } = run[i] else {
                unreachable!()
            };
            let end = run[i..]
                .iter()
                .position(|x| !matches!(x, Draw::Shape { dobj, .. } if *dobj == d))
                .map_or(run.len(), |n| i + n);
            self.dobj_disp(j, d, &run[i..end], &vmtx, &pmtx, rendermode);
            i = end;
        }
    }

    /// `HSD_DObjDisp` for the shapes the pass drew of a display object.
    fn dobj_disp(
        &mut self,
        j: JObjId,
        d: DObjId,
        shapes: &[Draw],
        vmtx: &Mtx34,
        pmtx: &Mtx34,
        rendermode: u32,
    ) {
        let m = self.objs().dobjs[d].mobj;
        if rendermode & RENDER_SHADOW == 0
            && let Some(m) = m
        {
            self.hsd.tobj_chain = material_chain(
                self.objs(),
                m,
                self.objs().mobjs[m].rendermode,
                &self.hsd.shadows,
            );
            match self.objs().mobjs[m].class {
                MObjClass::Fighter => self.fighter_mobj_setup(m),
                MObjClass::Ground => self.ground_mobj_setup(m),
                MObjClass::Item => self.item_mobj_setup(m),
                MObjClass::Hsd => self.mobj_setup(m, 0, None),
            }
        } else {
            self.hsd.tobj_chain.clear();
        }
        for s in shapes {
            if let Draw::Shape { pobj, .. } = s {
                self.pobj_disp(j, *pobj, vmtx, pmtx, rendermode);
            }
        }
        self.hsd.tobj_chain.clear();
    }

    /// The transparency passes a joint's flags give a pass, for checks.
    pub fn trsp_name(trsp: u32) -> &'static str {
        match trsp {
            TRSP_OPA => "opa",
            TRSP_TEXEDGE => "texedge",
            TRSP_XLU => "xlu",
            _ => "?",
        }
    }
}

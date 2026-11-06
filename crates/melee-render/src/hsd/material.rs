//! The game's own material classes' display setup: fighters' (`ftmaterial.c`), stage objects'
//! (`grmaterial.c`) and items' (`itmaterial.c`), each HAL's setup with a color overlay stage
//! (and a fighter's or item's light overlay stage) after the material's own, the registers
//! they use found free of the material's (`lbGetFreeColorRegister`, lb_00B0.c), and the
//! refracted fighter's setup (`lbRefract_80022998`).

use melee_sim::game::state::UserData;
use melee_sim::game::structs::{_GXColor, ColorOverlay};
use melee_sim::hsd::mobj::{MObjId, PeDesc};
use melee_sim::hsd::tobj::TObjId;

use super::draw::Ctx;
use super::tev::{HsdTevDesc, TevConf, setup_tev_stage};
use super::texp::{CnstValue, TE_A, TE_RGB, TevDesc, set_reg_values};
use crate::gx::Color;
use crate::gx::enums::*;

const RENDER_XLU: u32 = 1 << 30;
const RENDER_NO_ZUPDATE: u32 = 1 << 29;
const RENDER_ZMODE_ALWAYS: u32 = 1 << 27;
/// `RENDER_BLENDING`.
const RENDER_BLENDING: u32 = RENDER_XLU | RENDER_NO_ZUPDATE;

/// `HSD_TECnst::comp` of the classes' alpha constants (`HSD_TE_A`) and their `idx`.
const COMP_A: u8 = TE_A;

/// A class's stage template (`tevdesc_tmpl`): the previous color blended toward input `b` by
/// input `c`, the alpha passed through, unclamped; the color clamped as `clr_clamp` says
/// (fighters' and items' clamp, stages' don't).
fn tevdesc_tmpl(clr_clamp: u8) -> HsdTevDesc {
    HsdTevDesc {
        flags: 1,
        stage: 0,
        coord: GX_TEXCOORD_NULL,
        map: GX_TEXMAP_NULL,
        color: GX_COLOR_NULL,
        conf: TevConf {
            clr_op: GX_TEV_ADD,
            clr_a: GX_CC_CPREV,
            clr_b: GX_CC_ZERO,
            clr_c: GX_CC_ZERO,
            clr_d: GX_CC_ZERO,
            clr_scale: GX_CS_SCALE_1,
            clr_bias: GX_TB_ZERO,
            clr_clamp,
            clr_out_reg: GX_TEVPREV,
            alpha_op: GX_TEV_ADD,
            alpha_a: GX_CA_ZERO,
            alpha_b: GX_CA_ZERO,
            alpha_c: GX_CA_ZERO,
            alpha_d: GX_CA_APREV,
            alpha_scale: GX_CS_SCALE_1,
            alpha_bias: GX_TB_ZERO,
            alpha_clamp: GX_DISABLE,
            alpha_out_reg: GX_TEVPREV,
            mode: GX_TC_LINEAR,
            ras_swap: GX_TEV_SWAP0,
            tex_swap: GX_TEV_SWAP0,
            kcsel: GX_TEV_KCSEL_1,
            kasel: GX_TEV_KASEL_1,
        },
    }
}

/// `lb_803B9FF0` and `lb_803BA030`: the register index a stage's output register is.
const OUT_REG_INDEX: [usize; 4] = [7, 4, 5, 6];

/// The first register from `i0` (konst colors 0 to 3, TEV registers 0 to 2 as 4 to 6) that
/// neither the material's constants, its stages' color outputs nor `extra` use,
/// `lbGetFreeColorRegister`.
#[doc(alias = "lbGetFreeColorRegister")]
#[doc(alias = "lbGetFreeColorRegImpl")]
pub fn free_color_register(i0: i32, desc: &TevDesc, extra: &[u8]) -> i32 {
    let mut used = [false; 8];
    for c in &desc.cnsts {
        if c.reg < 8 {
            used[usize::from(c.reg)] = true;
        }
    }
    for s in &desc.stages {
        used[OUT_REG_INDEX[s.desc.conf.clr_out_reg as usize]] = true;
    }
    for &r in extra {
        if r < 8 {
            used[usize::from(r)] = true;
        }
    }
    (i0.max(0)..7).find(|&i| !used[i as usize]).unwrap_or(-1)
}

/// `lbGetFreeAlphaRegister` (`lbGetFreeAlphaRegImpl`): as [`free_color_register`] but
/// counting only the stages' alpha outputs (the original passes the constants and ignores
/// them).
pub fn free_alpha_register(i0: i32, desc: &TevDesc) -> i32 {
    let mut used = [false; 8];
    for s in &desc.stages {
        used[OUT_REG_INDEX[s.desc.conf.alpha_out_reg as usize]] = true;
    }
    (i0.max(0)..7).find(|&i| !used[i as usize]).unwrap_or(-1)
}

/// The color input reading register `i`, `lb_8000CC8C` (`lb_803B9FC0`).
#[doc(alias = "lb_8000CC8C")]
pub fn color_arg(i: i32) -> u32 {
    [14, 14, 14, 14, 2, 4, 6, 0][i as usize]
}

/// The konst color selection of konst register `i`, `lb_8000CCA4` (`lb_803B9FE0`).
#[doc(alias = "lb_8000CCA4")]
pub fn kcsel(i: i32) -> u32 {
    [12, 13, 14, 15][i as usize]
}

/// The alpha input reading register `i`, `lb_8000CD90` (`lb_803BA000`).
pub fn alpha_arg(i: i32) -> u32 {
    [6, 6, 6, 6, 1, 2, 3, 0][i as usize]
}

/// The konst alpha selection of konst register `i`, `lb_8000CDA8` (`lb_803BA020`).
pub fn kasel(i: i32) -> u32 {
    [28, 29, 30, 31][i as usize]
}

fn color(c: &_GXColor) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

/// The overlay color blended over the object's own tint (`ftMaterial_800BF6BC`'s and
/// `it_80278108`'s): its alpha is how much of the blend replaces the material's color.
fn blend_tint(tint: &_GXColor, hex: &_GXColor) -> Color {
    let temp_alpha = ((0xFF - i32::from(tint.a)) * (0xFF - i32::from(hex.a))) / 255;
    if temp_alpha == 0xFF {
        return color(hex);
    }
    let inv_alpha = 0xFF - temp_alpha;
    let ha = i32::from(hex.a);
    let mut out = Color::default();
    let r = i32::from(tint.r);
    let r = r + (ha * (i32::from(hex.r) - r)) / 255;
    let r7 = r * 0xFF;
    out.r = (r7 / inv_alpha) as u8;
    out.a = if out.r != 0 {
        (r7 / i32::from(out.r)) as u8
    } else {
        (((inv_alpha - r) * 0xFF) / 255) as u8
    };
    let g = i32::from(tint.g);
    out.g = (((g + (ha * (i32::from(hex.g) - g)) / 255) * 0xFF) / inv_alpha) as u8;
    let b = i32::from(tint.b);
    out.b = (((b + (ha * (i32::from(hex.b) - b)) / 255) * 0xFF) / inv_alpha) as u8;
    out
}

/// The pixel-engine settings a fighter or item drawn as a silhouette uses (`x2223_b3`,
/// `xDCF` bit 5): blending off, depth compared and written, no alpha test.
const SILHOUETTE_PE: PeDesc = PeDesc {
    flags: (1 << 3) | (1 << 4) | (1 << 5),
    ref0: 0,
    ref1: 0,
    dst_alpha: 0,
    ty: 0,
    src_factor: 4,
    dst_factor: 5,
    logic_op: 15,
    z_comp: 3,
    alpha_comp0: 7,
    alpha_op: 0,
    alpha_comp1: 7,
};

impl Ctx<'_> {
    /// The light overlay's stage (`ftMaterial_800BF534`, `it_80277F90`): the light color
    /// times the lit alpha added to the color. Returns the register it took.
    fn light_overlay_stage(&mut self, desc: &TevDesc, light: &_GXColor, clamp: u8) -> u8 {
        let reg = free_color_register(0, desc, &[]);
        assert!(reg != -1, "can't find free color register!");
        set_reg_values(
            self.gx,
            &mut self.hsd.tev,
            &[(CnstValue::Color(color(light)), TE_RGB, reg as u8, 0xFF)],
        );
        let mut d = tevdesc_tmpl(clamp);
        d.stage = self.hsd.tev.assign_tev();
        d.color = GX_ALPHA0;
        d.conf.clr_a = GX_CC_ZERO;
        d.conf.clr_b = color_arg(reg);
        d.conf.clr_c = GX_CC_RASA;
        d.conf.clr_d = GX_CC_CPREV;
        if reg < 4 {
            d.conf.kcsel = kcsel(reg);
        }
        setup_tev_stage(self.gx, &d);
        reg as u8
    }

    /// The color overlay's stage (`ftMaterial_800BF6BC`, `it_80278108`): the color blended
    /// toward `tint` by its alpha, and with `alpha` set, the alpha replaced by it.
    fn color_overlay_stage(
        &mut self,
        desc: &TevDesc,
        light_reg: Option<u8>,
        tint: Color,
        alpha: Option<u8>,
        clamp: u8,
    ) {
        let reg1 = free_color_register(0, desc, &light_reg.into_iter().collect::<Vec<_>>());
        assert!(reg1 != -1, "can't find free color register!");
        set_reg_values(
            self.gx,
            &mut self.hsd.tev,
            &[(CnstValue::Color(tint), TE_RGB, reg1 as u8, 0xFF)],
        );
        let start = if reg1 < 4 { 4 } else { 0 };
        let mut extra = vec![reg1 as u8];
        extra.extend(light_reg);
        let reg2 = free_color_register(start, desc, &extra);
        assert!(reg2 != -1, "can't find free color ratio register!");
        let ratio = Color::new(tint.a, tint.a, tint.a, 0);
        let mut cnsts = vec![(CnstValue::Color(ratio), TE_RGB, reg2 as u8, 0xFF)];
        if let Some(a) = alpha {
            cnsts.push((CnstValue::U8(a), COMP_A, reg2 as u8, 3));
        }
        set_reg_values(self.gx, &mut self.hsd.tev, &cnsts);
        let mut d = tevdesc_tmpl(clamp);
        d.stage = self.hsd.tev.assign_tev();
        d.conf.clr_b = color_arg(reg1);
        d.conf.clr_c = color_arg(reg2);
        if reg1 < 4 {
            d.conf.kcsel = kcsel(reg1);
        } else if reg2 < 4 {
            d.conf.kcsel = kcsel(reg2);
        }
        if alpha.is_some() {
            d.conf.alpha_d = alpha_arg(reg2);
            if reg2 < 4 {
                d.conf.kasel = kasel(reg2);
            }
        }
        setup_tev_stage(self.gx, &d);
    }

    /// The fighters' material setup, `ftMaterial_800BF2B8`, for the fighter being drawn.
    #[doc(alias = "ftMaterial_800BF2B8")]
    pub fn fighter_mobj_setup(&mut self, m: MObjId) {
        let Some(f) = self.hsd.fighter() else {
            self.hsd.note("a fighter material outside a fighter".into());
            return self.mobj_setup(m, 0, None);
        };
        let game = self.game;
        let fp = &game.fighters[f.0];
        if fp.x2226_b5 {
            let rendermode = game.objs.mobjs[m].rendermode;
            return self.refract_setup(rendermode, usize::from(fp.smash_attrs.x2134_vibrateFrame));
        }
        // The flags the fighter's render callback set for this tree, as melee-sim named them.
        let flags = match self.hsd.fighter_flags {
            Some(flags) => flags,
            None => {
                self.hsd
                    .note_once("a fighter material before its draw flags were named");
                melee_sim::hsd::display::FighterFlags {
                    x2223_b2: fp.x2223_b2,
                    x2223_b3: fp.x2223_b3,
                    x2228_b0: fp.x2228_b0,
                }
            }
        };
        let mut m = m;
        if !flags.x2223_b2 && (!flags.x2228_b0 || !fp.x2224_b0) {
            if fp.is_metal {
                match game.ft.metal_mobj {
                    Some(metal) => m = metal,
                    None => self
                        .hsd
                        .note("a metal fighter without the metal material".into()),
                }
            } else if fp.x2227_b3 {
                match game.ft.clear_mobj {
                    Some(clear) => m = clear,
                    None => self
                        .hsd
                        .note("a clear fighter without the clear material".into()),
                }
            }
        }
        let (desc, mut rendermode) = self.mobj_setup_tev_part(m);
        if fp.x61D != 0xFF {
            rendermode |= RENDER_NO_ZUPDATE | RENDER_XLU;
        }
        let overlay = active_overlay_ft(fp);

        // `ftMaterial_800BF534`.
        let mut light_reg = None;
        // `ftCo_8009F75C(fp, false)` otherwise: melee-sim named those lights before the
        // material (`Draw::Lights`), and the render mode's channels take them.
        let without_alpha = overlay.x7C_flag2
            && overlay.x7C_light_enable
            && (rendermode & RENDER_XLU != 0 || flags.x2223_b2);
        if overlay.x7C_flag2 && overlay.x7C_light_enable && !without_alpha {
            light_reg = Some(self.light_overlay_stage(&desc, &overlay.x50_light_color, GX_ENABLE));
        }

        // `ftMaterial_800BF6BC`.
        if !flags.x2223_b3 {
            let mut sub = fp.sub_color;
            if flags.x2228_b0 && fp.x2224_b0 {
                if fp.is_metal {
                    sub = 4;
                } else if fp.x2227_b3 {
                    sub = 5;
                }
            }
            let tint = if flags.x2223_b2 {
                Some(color(&fp.x610_color_rgba[1]))
            } else if sub != 0 {
                if overlay.x7C_color_enable {
                    Some(blend_tint(&fp.x610_color_rgba[0], &overlay.x2C_hex))
                } else {
                    Some(self.sub_color(sub))
                }
            } else if overlay.x7C_color_enable {
                Some(color(&overlay.x2C_hex))
            } else {
                None
            };
            if let Some(tint) = tint {
                let alpha = (fp.x61D != 0xFF).then_some(fp.x61D);
                self.color_overlay_stage(&desc, light_reg, tint, alpha, GX_ENABLE);
            }
        }

        if flags.x2223_b2 && !flags.x2223_b3 {
            rendermode |= RENDER_NO_ZUPDATE;
        }
        let pe = if flags.x2223_b3 && fp.x61D == 0xFF {
            Some(SILHOUETTE_PE)
        } else {
            game.objs.mobjs[m].pe
        };
        self.mobj_setup_render_mode(rendermode, pe);
        // `ftCo_8009F75C(fp, true)` when no light stage was made: the lights with alpha, after
        // the render mode, as melee-sim named them (`Draw::LightsAfterRenderMode`) when the
        // overlay lit the fighter.
        let after = self.hsd.lights_after_render_mode.take();
        if after.is_some() != without_alpha {
            self.hsd.note_once(
                "a fighter's lights after its render mode not where its material sets them up",
            );
        }
        if let Some(n) = after {
            self.load_lights(n);
        }
    }

    /// The fighters' tint colors (`p_ftCommonData->sub_colors[i - 1]`).
    fn sub_color(&mut self, i: u8) -> Color {
        let Some(base) = self.game.ft.load_common else {
            self.hsd
                .note("fighter sub colors without the common data".into());
            return Color::default();
        };
        let data = self.data;
        let Some(common) = data.ptr(base) else {
            return Color::default();
        };
        let at = common.at(0x6DC + 4 * u32::from(i - 1));
        Color::new(
            data.u8(at),
            data.u8(at.at(1)),
            data.u8(at.at(2)),
            data.u8(at.at(3)),
        )
    }

    /// The stage objects' material setup, `fn_801C8EF8`, for the stage object being drawn:
    /// HAL's, with a stage for the object's color overlay and its alpha (`x10_flags.b6`,
    /// `xC0`).
    #[doc(alias = "fn_801C8EF8")]
    pub fn ground_mobj_setup(&mut self, m: MObjId) {
        let game = self.game;
        let gp = match self
            .hsd
            .rendering
            .and_then(|g| game.world.gobj(g).user_data)
        {
            Some(UserData::Ground(g)) => &game.grounds[g.0],
            _ => {
                self.hsd
                    .note("a stage material outside a stage object".into());
                return self.mobj_setup(m, 0, None);
            }
        };
        let (desc, rendermode) = self.mobj_setup_tev_part(m);
        let overlay = &gp.color_overlay;
        if overlay.x7C_color_enable || gp.x10_flags.b6 {
            let mut d = tevdesc_tmpl(GX_DISABLE);
            d.stage = self.hsd.tev.assign_tev();
            if overlay.x7C_color_enable {
                let reg1 = free_color_register(0, &desc, &[]);
                assert!(reg1 != -1, "can't find free color register!");
                let hex = color(&overlay.x2C_hex);
                set_reg_values(
                    self.gx,
                    &mut self.hsd.tev,
                    &[(CnstValue::Color(hex), TE_RGB, reg1 as u8, 0xFF)],
                );
                let start = if reg1 < 4 { 4 } else { 0 };
                let reg2 = free_color_register(start, &desc, &[reg1 as u8]);
                assert!(reg2 != -1, "can't find free color register!");
                let a = overlay.x2C_hex.a;
                set_reg_values(
                    self.gx,
                    &mut self.hsd.tev,
                    &[(
                        CnstValue::Color(Color::new(a, a, a, 0)),
                        TE_RGB,
                        reg2 as u8,
                        0xFF,
                    )],
                );
                d.conf.clr_b = color_arg(reg1);
                d.conf.clr_c = color_arg(reg2);
                if reg1 < 4 {
                    d.conf.kcsel = kcsel(reg1);
                } else if reg2 < 4 {
                    d.conf.kcsel = kcsel(reg2);
                }
            }
            if gp.x10_flags.b6 {
                let reg = free_alpha_register(0, &desc);
                assert!(reg != -1, "can't find free alpha register!");
                // The template's `idx` (0xFF): a TEV register's color takes the value, a konst
                // register's alpha.
                set_reg_values(
                    self.gx,
                    &mut self.hsd.tev,
                    &[(CnstValue::F32(gp.xC0), COMP_A, reg as u8, 0xFF)],
                );
                d.conf.alpha_b = GX_CA_APREV;
                d.conf.alpha_c = alpha_arg(reg);
                d.conf.alpha_d = GX_CA_ZERO;
            }
            setup_tev_stage(self.gx, &d);
        }
        let pe = game.objs.mobjs[m].pe;
        self.mobj_setup_render_mode(rendermode, pe);
    }

    /// The items' material setup, `fn_80277D8C`, for the item being drawn: HAL's with the
    /// item's light and color overlay stages (`it_80277F90`, `it_80278108`).
    #[doc(alias = "fn_80277D8C")]
    pub fn item_mobj_setup(&mut self, m: MObjId) {
        let game = self.game;
        let ip = match self
            .hsd
            .rendering
            .and_then(|g| game.world.gobj(g).user_data)
        {
            Some(UserData::Item(i)) => &game.items[i.0],
            _ => {
                self.hsd.note("an item material outside an item".into());
                return self.mobj_setup(m, 0, None);
            }
        };
        let flag = |i: u32| ip.xDCF_flag[0] >> (7 - i) & 1 != 0;
        let (b3, b4, b5) = (flag(3), flag(4), flag(5));
        let (desc, mut rendermode) = self.mobj_setup_tev_part(m);

        // `it_80277F90`.
        let own = &ip.x548_colorOverlay;
        let light_reg = (!b4 && own.x7C_flag2 && own.x7C_light_enable)
            .then(|| self.light_overlay_stage(&desc, &own.x50_light_color, GX_ENABLE));

        // `it_80278108`.
        if !b5 {
            let mut overlay = own;
            if b3
                && ip.xDC8_word.flags.x13
                && let Some(o) = ip.owner
                && let Some(UserData::Fighter(f)) = game.world.gobj(o).user_data
            {
                overlay = active_overlay_ft(&game.fighters[f.0]);
            }
            let tint = if b4 {
                Some(color(&ip.xBC8.x0_unk))
            } else if ip.x5C8 != 0 {
                if overlay.x7C_color_enable {
                    Some(blend_tint(&ip.xBC4, &overlay.x2C_hex))
                } else {
                    Some(color(&ip.xBC4))
                }
            } else if overlay.x7C_color_enable {
                Some(color(&overlay.x2C_hex))
            } else {
                None
            };
            if let Some(tint) = tint {
                let alpha = (ip.x5C9 != 0xFF).then_some(ip.x5C9);
                self.color_overlay_stage(&desc, light_reg, tint, alpha, GX_ENABLE);
            }
        }

        if ip.x5C9 != 0xFF {
            rendermode |= RENDER_BLENDING;
        }
        if b4 && !b5 {
            rendermode |= RENDER_NO_ZUPDATE;
        }
        let pe = if b5 && ip.x5C9 == 0xFF {
            Some(SILHOUETTE_PE)
        } else {
            game.objs.mobjs[m].pe
        };
        self.mobj_setup_render_mode(rendermode, pe);
    }

    /// A refracted fighter's material setup, `lbRefract_80022998`: refraction's texture
    /// chain for `frame`, its two coordinates (the normals' through texture matrix 0 and the
    /// positions' projected onto the screen copy), and one stage reading the screen copy
    /// offset by the indirect map, sent to GX directly (not through HAL's state cache). In
    /// widescreen the screen copy's projection widens with the camera
    /// ([`Ctx::refract_texture_mtx`]).
    #[doc(alias = "lbRefract_80022998")]
    fn refract_setup(&mut self, rendermode: u32, frame: usize) {
        const TEXTURE_MTX: [[f32; 4]; 3] = [
            [0.5, 0.0, 0.0, 0.5],
            [0.0, -0.5, 0.0, 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        const TEXTURE_OFFSET: [[f32; 3]; 2] = [[-0.5, 0.0, 0.0], [0.0, -0.5, 0.0]];
        let refract = &self.game.refract;
        let Some(&top) = refract.tobjs.get(frame) else {
            self.hsd.note("refraction without its texture chain".into());
            return;
        };
        let chain: Vec<TObjId> = self.game.objs.tobjs.chain(Some(top)).collect();
        self.hsd.tobj_chain = chain.clone();
        self.tobj_setup(&chain);
        let screen_mtx = self.refract_texture_mtx().m;
        let gx = &mut *self.gx;
        gx.set_num_tex_gens(2);
        gx.set_tex_coord_gen2(
            GX_TEXCOORD0,
            GX_TG_MTX3X4,
            GX_TG_NRM,
            GX_TEXMTX0,
            GX_TRUE,
            GX_PTTEXMTX0,
        );
        gx.set_tex_coord_gen2(
            GX_TEXCOORD1,
            GX_TG_MTX3X4,
            GX_TG_POS,
            0,
            GX_FALSE,
            GX_PTTEXMTX1,
        );
        gx.load_tex_mtx_imm(&TEXTURE_MTX, GX_PTTEXMTX0, GX_MTX3X4);
        gx.load_tex_mtx_imm(&screen_mtx, GX_PTTEXMTX1, GX_MTX3X4);
        gx.set_num_chans(0);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD1, GX_TEXMAP1, GX_COLOR_NULL);
        gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, GX_CC_TEXC);
        gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_TEXA);
        gx.set_tev_color_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_FALSE,
            GX_TEVPREV,
        );
        gx.set_tev_alpha_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_FALSE,
            GX_TEVPREV,
        );
        gx.set_num_ind_stages(1);
        gx.set_ind_tex_order(GX_INDTEXSTAGE0, GX_TEXCOORD0, GX_TEXMAP0);
        gx.set_ind_tex_coord_scale(GX_INDTEXSTAGE0, GX_ITS_1, GX_ITS_1);
        gx.set_ind_tex_mtx(GX_ITM_0, &TEXTURE_OFFSET, 1);
        gx.set_tev_indirect(
            GX_TEVSTAGE0,
            GX_INDTEXSTAGE0,
            GX_ITF_8,
            GX_ITB_ST,
            GX_ITM_0,
            GX_ITW_OFF,
            GX_ITW_OFF,
            0,
            0,
            GX_ITBA_OFF,
        );
        gx.set_color_update(GX_TRUE);
        gx.set_alpha_update(GX_FALSE);
        let write_z = u8::from(rendermode & RENDER_NO_ZUPDATE == 0);
        let compare = if rendermode & RENDER_ZMODE_ALWAYS != 0 {
            GX_ALWAYS
        } else {
            GX_LEQUAL
        };
        gx.set_z_mode(GX_TRUE, compare, write_z);
        gx.set_z_comp_loc(GX_TRUE);
        gx.set_blend_mode(GX_BM_NONE, GX_BL_ONE, GX_BL_ZERO, GX_LO_SET);
        gx.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_OR, GX_ALWAYS, 0);
        gx.set_dither(GX_FALSE);
    }
}

/// The overlay a fighter shows, `ftCo_800C0658`: the first one's while it runs, else the
/// second's.
#[doc(alias = "ftCo_800C0658")]
fn active_overlay_ft(fp: &melee_sim::game::structs::Fighter) -> &ColorOverlay {
    if fp.x408.x28_colanim != [0; 4] {
        &fp.x408
    } else {
        &fp.x488
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_blend() {
        // A full overlay replaces the tint.
        let hex = _GXColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let tint = _GXColor {
            r: 10,
            g: 20,
            b: 30,
            a: 0,
        };
        let c = blend_tint(&tint, &hex);
        assert_eq!((c.r, c.g, c.b), (255, 0, 0));
    }

    #[test]
    fn register_tables() {
        assert_eq!(color_arg(0), GX_CC_KONST);
        assert_eq!(color_arg(5), GX_CC_C1);
        assert_eq!(alpha_arg(4), GX_CA_A0);
        assert_eq!(kcsel(1), GX_TEV_KCSEL_K1);
        assert_eq!(kasel(2), GX_TEV_KASEL_K2_A);
        let desc = TevDesc::default();
        assert_eq!(free_color_register(0, &desc, &[0, 1]), 2);
        assert_eq!(free_color_register(4, &desc, &[]), 4);
    }
}

//! A sword's afterimage (`melee/ft/ftafterimage.c`'s `ftCo_800C2600`): a triangle strip
//! through the sword's last positions in the fighter's ring, widening and fading along the
//! swing, set up in GX directly, after which HSD's state caches are invalidated.

use gekko_fp as fp;
use melee_sim::game::handles::FighterId;
use melee_sim::math;
use melee_sim::vec::Vec3;

use super::draw::Ctx;
use super::state::STATE_ALL;
use crate::gx::enums::*;

/// The angle between the strip's in-between positions, at most (`AFTERIMAGE_ANGLE_STEP`).
const ANGLE_STEP: f32 = 0.087_266_46;

const KIND_LINK: i32 = 6;
const KIND_CLINK: i32 = 20;
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;

/// A float as a register holds it after `lfs`.
fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs`.
fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

fn mul(a: f32, b: f32) -> f32 {
    mem(fp::fmuls(reg(a), reg(b)))
}

fn madd(a: f32, b: f32, c: f32) -> f32 {
    mem(fp::fmadds(reg(a), reg(b), reg(c)))
}

fn div(a: f32, b: f32) -> f32 {
    mem(fp::fdivs(reg(a), reg(b)))
}

/// A float to an integer as `(s32)` converts it (`fctiwz`).
fn int(x: f32) -> i32 {
    fp::fctiwz(reg(x))
}

/// The ring slot before `i`.
fn prev_slot(i: usize) -> usize {
    if i != 0 { i - 1 } else { 2 }
}

/// A sword's afterimage parameters (`itSword_UnkBytes`): where the strip's inner and outer
/// edges sit between the sword's two widths, its alpha at the newest and oldest positions,
/// and its inner and outer edges' colors.
struct Params {
    inner: f32,
    outer: f32,
    alpha_new: u8,
    alpha_old: u8,
    inner_rgb: [u8; 3],
    outer_rgb: [u8; 3],
}

impl Ctx<'_> {
    /// A fighter's sword afterimage, `ftCo_800C2600` (the display pass checked its pass and
    /// that the ring holds two positions).
    #[doc(alias = "ftCo_800C2600")]
    pub fn afterimage(&mut self, f: FighterId) {
        let game = self.game;
        let fp = &game.fighters[f.0];
        let gx = &mut *self.gx;
        gx.set_color_update(GX_TRUE);
        gx.set_alpha_update(GX_FALSE);
        gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_NOOP);
        gx.set_alpha_compare(GX_GREATER, 0, GX_AOP_AND, GX_GREATER, 0);
        gx.set_z_mode(GX_TRUE, GX_LEQUAL, GX_FALSE);
        gx.set_z_comp_loc(GX_FALSE);
        gx.set_num_tex_gens(0);
        gx.set_tev_clamp_mode(GX_TEVSTAGE0, 0);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
        gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
        gx.set_num_chans(1);
        gx.set_chan_ctrl(
            GX_COLOR0A0,
            GX_FALSE,
            GX_SRC_REG,
            GX_SRC_VTX,
            0,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        gx.set_cull_mode(GX_CULL_NONE);

        // `ftCo_800C2600_get_params`: a held Beam Sword's (`it_80285300`), or the sword
        // attributes in the character's (Link's and Young Link's at x64, Marth's and Roy's at
        // x78).
        let held;
        let b: &[u8] = if fp.x2101_bits_8 {
            let item = fp
                .item_gobj
                .expect("a held item's afterimage without an item");
            let kind = melee_sim::game::it::item(game, item).kind;
            assert!(
                kind == melee_sim::game::it::kinds::SWORD,
                "ftCo_800C2600: no afterimage item!"
            );
            let at = melee_sim::game::it::kinds::sword::trail_params(game, self.data, item);
            held = (0..0x14u32)
                .map(|i| self.data.u8(at.at(i)))
                .collect::<Vec<u8>>();
            &held
        } else {
            let base = match fp.kind {
                KIND_LINK | KIND_CLINK => 0x64,
                KIND_MARS | KIND_EMBLEM => 0x78,
                k => panic!("ftCo_800C2600: character {k} has no sword attributes"),
            };
            let attrs = &game.fighter_attrs[fp.dat_attrs.expect("a fighter without attributes").0];
            &attrs[base..base + 0x14]
        };
        let word =
            |o: usize| f32::from_bits(u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]));
        let p = Params {
            inner: word(0),
            outer: word(4),
            alpha_new: b[8],
            alpha_old: b[9],
            inner_rgb: [b[0xA], b[0xB], b[0xC]],
            outer_rgb: [b[0xE], b[0xF], b[0x10]],
        };

        let (w0, w1) = (fp.x20F8, fp.x20FC);
        let n = i32::from(fp.x2100);
        let start = prev_slot(usize::from(fp.x2101_bits_0to6));
        // The distances along the sword's outer end, newest first, summed.
        let mut cum = vec![0.0f32];
        let mut total = 0.0f32;
        let mut prev = Vec3::new(0.0, 0.0, 0.0);
        let mut cur = start;
        for i in (0..n).rev() {
            let e = &fp.x20B0[cur];
            let d = Vec3::new(
                madd(e.xC.x, w1, e.x0.x) - prev.x,
                madd(e.xC.y, w1, e.x0.y) - prev.y,
                madd(e.xC.z, w1, e.x0.z) - prev.z,
            );
            if i != n - 1 {
                let d2 = mul(d.z, d.z) + (mul(d.x, d.x) + mul(d.y, d.y));
                total += math::sqrt(d2);
                cum.push(total);
            }
            prev = d;
            if i != 0 {
                cur = prev_slot(cur);
            }
        }
        if total < 0.00001 {
            total = 1.0;
        }

        let span = w1 - w0;
        let inner0 = madd(p.inner, span, w0);
        let outer0 = madd(p.outer, span, w0);
        let inner_diff = w0 - inner0;
        let outer_diff = w1 - outer0;
        // `(f32) (x8 - x9)` and `(f32) x9`.
        let alpha_span = (i32::from(p.alpha_new) - i32::from(p.alpha_old)) as f32;
        let alpha_old = f32::from(p.alpha_old);
        let mut verts: Vec<(Vec3, [u8; 4])> = Vec::new();
        let mut push = |pos: Vec3, inner: f32, outer: f32, dir: Vec3, alpha: i32| {
            let at = |s: f32| {
                Vec3::new(
                    madd(dir.x, s, pos.x),
                    madd(dir.y, s, pos.y),
                    madd(dir.z, s, pos.z),
                )
            };
            let [r, g, b] = p.inner_rgb;
            verts.push((at(inner), [r, g, b, alpha as u8]));
            let [r, g, b] = p.outer_rgb;
            verts.push((at(outer), [r, g, b, alpha as u8]));
        };
        let mut interp = 1.0f32;
        let mut cur = start;
        let mut dist = 0;
        for remaining in (0..n).rev() {
            let e = &fp.x20B0[cur];
            let mut outer = madd(interp, outer_diff, outer0);
            let mut inner = madd(interp, inner_diff, inner0);
            dist += 1;
            let mut alpha = int(madd(interp, alpha_span, alpha_old));
            push(e.x0, inner, outer, e.xC, alpha);
            if remaining == 0 {
                break;
            }
            let next = prev_slot(cur);
            let ne = &fp.x20B0[next];
            // `lbVector_CrossprodNormalized` never returns null.
            let axis = e.xC.cross_normalized(ne.xC);
            let mut angle = e.xC.angle(ne.xC);
            let steps = int(div(angle, ANGLE_STEP));
            interp = 1.0 - div(cum[dist], total);
            if steps != 0 {
                let frac = div(1.0, (steps + 1) as f32);
                angle = mul(angle, frac);
                let inner_step = mul(frac, madd(interp, inner_diff, inner0) - inner);
                let mut pos = e.x0;
                let pos_step = Vec3::new(
                    mul(frac, ne.x0.x - pos.x),
                    mul(frac, ne.x0.y - pos.y),
                    mul(frac, ne.x0.z - pos.z),
                );
                let alpha_step = int(mul(
                    frac,
                    madd(interp, alpha_span, alpha_old) - alpha as f32,
                ));
                let outer_step = mul(frac, madd(interp, outer_diff, outer0) - outer);
                let mut turned = 0.0f32;
                for _ in 0..steps {
                    turned += angle;
                    pos = Vec3::new(pos.x + pos_step.x, pos.y + pos_step.y, pos.z + pos_step.z);
                    inner += inner_step;
                    outer += outer_step;
                    alpha = alpha.wrapping_add(alpha_step);
                    let mut dir = e.xC;
                    dir.rotate_about(axis, turned);
                    push(pos, inner, outer, dir, alpha);
                }
            }
            cur = next;
        }

        let gx = &mut *self.gx;
        gx.clear_vtx_desc();
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
        gx.load_pos_mtx_imm(&self.hsd.view_mtx().m, GX_PNMTX0);
        gx.set_current_mtx(GX_PNMTX0);
        // The first vertex, the third, then from the second on: the strip's first triangle
        // is the newest position's inner edge alone.
        gx.begin(GX_TRIANGLESTRIP, GX_VTXFMT0, (verts.len() + 1) as u16);
        for (v, [r, g, b, a]) in std::iter::once(verts[0])
            .chain(std::iter::once(verts[2]))
            .chain(verts[1..].iter().copied())
        {
            gx.position3f32(v.x, v.y, v.z);
            gx.color4u8(r, g, b, a);
        }
        gx.end();
        self.invalidate(STATE_ALL);
    }
}

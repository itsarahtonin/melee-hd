//! The SDK's streams through ssbm-gx's parser, and register values worked out by hand from
//! the C.

use super::enums::*;
use super::*;
use ssbm_gx::{Event, Produced, State};
use std::collections::HashMap;

/// Memory for the parser: display lists at their physical addresses, zeros elsewhere.
#[derive(Default)]
struct Mem(HashMap<u32, Vec<u8>>);

impl ssbm_gx::Memory for Mem {
    fn read(&self, phys: u32, out: &mut [u8]) {
        out.fill(0);
        if let Some(bytes) = self.0.get(&phys) {
            let n = bytes.len().min(out.len());
            out[..n].copy_from_slice(&bytes[..n]);
        }
    }
}

/// Runs `bytes` through a parser state; every byte must belong to a whole command.
fn run(state: &mut State, mem: &Mem, bytes: &[u8]) -> Vec<Event> {
    let mut out: Vec<Produced> = Vec::new();
    let used = state.run(mem, bytes, &mut out, false);
    assert_eq!(
        used,
        bytes.len(),
        "the stream ends in the middle of a command"
    );
    out.into_iter().map(|p| p.event).collect()
}

fn initialized() -> (Gx, State) {
    let mut gx = Gx::new();
    gx.init();
    let mut state = State::new();
    let fifo = gx.take_fifo();
    run(&mut state, &Mem::default(), &fifo);
    (gx, state)
}

/// The BP writes (`0x61` and the value) in a stream of nothing but BP writes.
fn ras_writes(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 5, 0);
    bytes
        .chunks(5)
        .map(|c| {
            assert_eq!(c[0], 0x61);
            u32::from_be_bytes([c[1], c[2], c[3], c[4]])
        })
        .collect()
}

#[test]
fn init_stream_parses() {
    let mut gx = Gx::new();
    gx.init();
    assert!(!gx.fifo.is_empty());
    let mut state = State::new();
    let events = run(&mut state, &Mem::default(), &gx.fifo);
    assert!(events.is_empty(), "GXInit draws nothing: {events:?}");
    // The first commands: the texture state flush and the two timing registers.
    assert_eq!(
        &gx.fifo[..15],
        &[
            0x61, 0x0F, 0x00, 0x00, 0xFF, 0x61, 0x69, 0x00, 0x04, 0x9E, 0x61, 0x0F, 0x00, 0x00,
            0xFF
        ]
    );
    // GXInit's TEV setup: stage 0 replaces with the texture's color and alpha.
    assert_eq!(state.bp[0xC0], 0x08_FFF8);
    assert_eq!(state.bp[0xC1], 0x08_FFC0);
    // The scissor covers the 640x480 frame from (342, 342).
    assert_eq!(state.bp[0x20], (342 << 12) | 342);
    assert_eq!(state.bp[0x21], ((342 + 639) << 12) | (342 + 479));
    // Matrix 0 and the texture identity matrix are the identity.
    assert_eq!(state.xf[0], 1.0f32.to_bits());
    assert_eq!(state.xf[5], 1.0f32.to_bits());
    assert_eq!(state.xf[60 * 4 + 10], 1.0f32.to_bits());
    // The viewport: 320 wide, -240 high, centered at 342 + (320, 240).
    assert_eq!(state.xf[0x101A], 320.0f32.to_bits());
    assert_eq!(state.xf[0x101B], (-240.0f32).to_bits());
    assert_eq!(state.xf[0x101D], 662.0f32.to_bits());
    assert_eq!(state.xf[0x101E], 582.0f32.to_bits());
    assert_eq!(gx.data.dirty_state, 0x0F);
    assert!(gx.data.tex_region_callback.is_some());
}

#[test]
fn tev_setup() {
    let (mut gx, mut state) = initialized();
    // tevc[1] is 0xC2000000 after GXInit: only stage 0 got inputs.
    gx.set_tev_color_in(GX_TEVSTAGE1, GX_CC_C0, GX_CC_TEXC, GX_CC_RASC, GX_CC_ZERO);
    // a = 2 << 12, b = 8 << 8, c = 10 << 4, d = 15.
    assert_eq!(gx.fifo, [0x61, 0xC2, 0x00, 0x28, 0xAF]);
    // Scale 2 (1 << 20), clamp (1 << 19), into register 0 (1 << 22).
    gx.set_tev_color_op(
        GX_TEVSTAGE1,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_2,
        GX_TRUE,
        GX_TEVREG0,
    );
    // A comparison: op 8 sets bit 18 to 0, scale to (8 >> 1) & 3 = 0 and bias to 3.
    gx.set_tev_alpha_op(
        GX_TEVSTAGE1,
        GX_TEV_COMP_R8_GT,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        0,
        GX_TEVPREV,
    );
    gx.set_tev_k_color_sel(GX_TEVSTAGE1, GX_TEV_KCSEL_K2);
    gx.set_tev_color(GX_TEVREG0, Color::new(0x12, 0x34, 0x56, 0x78));
    let fifo = gx.take_fifo();
    assert_eq!(
        ras_writes(&fifo),
        [
            0xC200_28AF,
            0xC258_28AF,
            0xC303_0000,
            // tevKsel[0] after GXInit: 0xF6 with swap table 0 (R, G = 0, 1), konst 1/4 (6)
            // and alpha 1 (0) for both stages; stage 1's color now K2 (0xE << 14).
            0xF603_8064,
            0xE207_8012,
            0xE303_4056,
            0xE303_4056,
            0xE303_4056,
        ]
    );
    run(&mut state, &Mem::default(), &fifo);
    assert_eq!(state.bp[0xC2], 0x58_28AF);
    assert_eq!(state.color_ra[1], 0x07_8012);
    assert_eq!(state.color_bg[1], 0x03_4056);
}

#[test]
fn blend_mode() {
    let (mut gx, _) = initialized();
    // cmode0 after GXInit: 0x41, src alpha (4 << 8), inverse src alpha (5 << 5), color and
    // alpha updates (bits 3, 4) and dither (bit 2).
    assert_eq!(gx.data.cmode0, 0x4100_04BC);
    gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
    assert_eq!(gx.take_fifo(), [0x61, 0x41, 0x00, 0x04, 0xBD]);
    // Subtract: blend (bit 0) and subtract (bit 11); one (1 << 8), one (1 << 5).
    gx.set_blend_mode(GX_BM_SUBTRACT, GX_BL_ONE, GX_BL_ONE, GX_LO_CLEAR);
    assert_eq!(ras_writes(&gx.take_fifo()), [0x4100_093D]);
    // Logic op XOR (6 << 12), bit 1.
    gx.set_blend_mode(GX_BM_LOGIC, GX_BL_ONE, GX_BL_ONE, GX_LO_XOR);
    assert_eq!(ras_writes(&gx.take_fifo()), [0x4100_613E]);
}

#[test]
fn texture_load() {
    let (mut gx, mut state) = initialized();
    let mut t = TexObj::default();
    gx.init_tex_obj(
        &mut t,
        0x8012_3460,
        64,
        32,
        GX_TF_RGBA8,
        GX_REPEAT,
        GX_CLAMP,
        0,
    );
    // Wrap S repeat (1), linear magnification (bit 4), min filter linear (4 << 5).
    assert_eq!(t.mode0, 0x91);
    assert_eq!(t.image0, (6 << 20) | (31 << 10) | 63);
    assert_eq!(t.image3, 0x0012_3460 >> 5);
    assert_eq!((t.load_fmt, t.load_cnt, t.flags), (3, 16 * 8, 2));
    assert_eq!(gx.get_tex_obj_width(&t), 64);
    assert_eq!(gx.get_tex_obj_height(&t), 32);
    assert_eq!(gx.get_tex_obj_fmt(&t), GX_TF_RGBA8);
    gx.init_tex_obj_lod(
        &mut t, GX_LINEAR, GX_LINEAR, 0.0, 0.0, 0.0, 0, 0, GX_ANISO_1,
    );
    // No edge LOD (bit 8).
    assert_eq!(t.mode0, 0x191);
    gx.load_tex_obj(&t, GX_TEXMAP0);
    // Region 0: even TMEM at 0, odd at 0x80000 (0x4000 in 32-byte units), both 32K (3 at
    // bits 15 and 18).
    let fifo = gx.take_fifo();
    assert_eq!(
        ras_writes(&fifo),
        [
            0x8000_0191,
            0x8400_0000,
            0x8860_7C3F,
            0x8C0D_8000,
            0x900D_C000,
            0x9400_91A3
        ]
    );
    assert_eq!(gx.data.dirty_state & 1, 1);
    assert_eq!(gx.data.next_tex_rgn, 1);
    run(&mut state, &Mem::default(), &fifo);
    assert_eq!(state.bp[0x94], 0x91A3);

    // A CI texture loads its palette's register too.
    let mut tlut = TlutObj::default();
    gx.init_tlut_obj(&mut tlut, 0x8045_6780, GX_TL_RGB5A3, 16);
    assert_eq!(tlut.load_tlut0, 0x6400_0000 | (0x0045_6780 >> 5));
    gx.load_tlut(&tlut, GX_TLUT1);
    // TLUT 1 is at 0xC2000 in TMEM: (0xC2000 - 0x80000) >> 9 = 0x210, 16 lines (16 << 10).
    assert_eq!(
        ras_writes(&gx.take_fifo()),
        [0x0F00_00FF, 0x6402_2B3C, 0x6500_4210, 0x0F00_00FF]
    );
    let mut ci = TexObj::default();
    gx.init_tex_obj_ci(
        &mut ci,
        0x8001_0000,
        8,
        8,
        GX_TF_C4,
        GX_CLAMP,
        GX_CLAMP,
        0,
        GX_TLUT1,
    );
    gx.load_tex_obj(&ci, GX_TEXMAP1);
    let writes = ras_writes(&gx.take_fifo());
    assert_eq!(writes.len(), 7);
    // The CI region 0: even TMEM at 0x40000, odd at 0x48000.
    assert_eq!(writes[3], 0x8D0D_A000);
    assert_eq!(writes[4], 0x910D_A400);
    // The palette: RGB5A3 (2 << 10) at offset 0x210.
    assert_eq!(writes[6], 0x9900_0A10);
}

#[test]
fn matrix_load() {
    let (mut gx, mut state) = initialized();
    let m = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, -0.5],
    ];
    gx.load_pos_mtx_imm(&m, GX_PNMTX1);
    let fifo = gx.take_fifo();
    // 12 words (count - 1 = 11 at bit 16) at 3 * 4.
    assert_eq!(&fifo[..5], &[0x10, 0x00, 0x0B, 0x00, 0x0C]);
    assert_eq!(&fifo[5..9], &1.0f32.to_be_bytes());
    assert_eq!(&fifo[49..53], &(-0.5f32).to_be_bytes());
    assert_eq!(fifo.len(), 5 + 48);
    gx.load_nrm_mtx_imm(&m, GX_PNMTX1);
    let nrm = gx.take_fifo();
    // 9 words at 0x400 + 3 * 3.
    assert_eq!(&nrm[..5], &[0x10, 0x00, 0x08, 0x04, 0x09]);
    assert_eq!(&nrm[5 + 12..5 + 16], &5.0f32.to_be_bytes());
    gx.load_tex_mtx_imm(&m[..2], GX_TEXMTX0, GX_MTX2X4);
    let tex = gx.take_fifo();
    assert_eq!(&tex[..5], &[0x10, 0x00, 0x07, 0x00, 0x78]);
    assert_eq!(tex.len(), 5 + 32);
    gx.set_current_mtx(GX_PNMTX1);
    // matIdxA: position 3, texture coordinates 0-3 the identity (60).
    let idx = (60 << 24) | (60 << 18) | (60 << 12) | (60 << 6) | 3;
    assert_eq!(gx.data.mat_idx_a, idx);
    let cur = gx.take_fifo();
    run(&mut state, &Mem::default(), &[fifo, nrm, tex, cur].concat());
    let words: Vec<u32> = m.iter().flatten().map(|v: &f32| v.to_bits()).collect();
    assert_eq!(&state.xf[12..24], &words[..]);
    assert_eq!(state.xf[0x400 + 9 + 3], 5.0f32.to_bits());
    assert_eq!(state.xf[0x1018], idx);
    assert_eq!(state.cp[0x30], idx);
}

#[test]
fn draw_parses() {
    let (mut gx, mut state) = initialized();
    gx.clear_vtx_desc();
    gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
    gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
    gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT);
    gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
    gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
    gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_U8, 0);
    gx.begin(GX_QUADS, GX_VTXFMT0, 4);
    for i in 0..4u8 {
        gx.position3f32(f32::from(i), 0.0, -1.0);
        gx.color4u8(255, i, 0, 255);
        gx.tex_coord2u8(i & 1, i >> 1);
    }
    gx.end();
    let fifo = gx.take_fifo();
    let events = run(&mut state, &Mem::default(), &fifo);
    assert_eq!(
        events.last(),
        Some(&Event::Draw {
            primitive: GX_QUADS as u8,
            vat: 0,
            count: 4
        })
    );
    // The descriptor: position (1 << 9), color 0 (1 << 13), texture 0 (1 in vcdHi).
    assert_eq!(state.cp[0x50], (1 << 9) | (1 << 13));
    assert_eq!(state.cp[0x60], 1);
    // XF's counts: one color, one texture coordinate.
    assert_eq!(state.xf[0x1008], 1 | (1 << 4));
}

#[test]
fn display_list_calls() {
    let (mut gx, mut state) = initialized();
    gx.begin_display_list();
    gx.set_tev_k_color(GX_KCOLOR0, Color::new(1, 2, 3, 4));
    gx.set_cull_mode(GX_CULL_FRONT);
    let mut list = gx.end_display_list();
    // GXEndDisplayList's dirty state put the gen mode in the list; the saved context takes
    // the shadow back.
    assert_eq!(list.len(), 15);
    assert_eq!(gx.data.gen_mode & (3 << 14), 1 << 14);
    list.resize(32, 0);
    let mut mem = Mem::default();
    mem.0.insert(0x0010_0000, list);
    gx.call_display_list(0x8010_0000, 32);
    let fifo = gx.take_fifo();
    assert_eq!(
        &fifo[fifo.len() - 9..],
        &[0x40, 0x80, 0x10, 0, 0, 0, 0, 0, 0x20]
    );
    run(&mut state, &mem, &fifo);
    assert_eq!(state.konst_ra[0], (4 << 12) | 1);
    assert_eq!(state.konst_bg[0], (2 << 12) | 3);
    // Cull front is the hardware's back (2 at bit 14).
    assert_eq!(state.bp[0x00] & (3 << 14), 2 << 14);
}

#[test]
fn fog_and_light() {
    let (mut gx, _) = initialized();
    gx.set_fog(GX_FOG_LIN, 0.0, 1.0, 0.1, 1.0, Color::new(1, 2, 3, 0));
    let w = ras_writes(&gx.take_fifo());
    // B = 1 / 0.9 = 1.111: mantissa 0.5556 * 8388638, exponent 1 + 1.
    assert_eq!(w[1] >> 24, 0xEF);
    assert_eq!(w[2], 0xF000_0002);
    // C = 0: zero mantissa and exponent, linear fog (2 << 21).
    assert_eq!(w[3], 0xF140_0000);
    assert_eq!(w[4], 0xF201_0203);
    let mut lt = LightObj::default();
    gx.init_light_spot(&mut lt, 0.0, GX_SP_COS);
    assert_eq!(lt.a, [1.0, 0.0, 0.0]);
    gx.init_light_dist_attn(&mut lt, 100.0, 0.5, GX_DA_GENTLE);
    assert_eq!(lt.k, [1.0, 0.01, 0.0]);
    gx.init_light_color(&mut lt, Color::new(0xAA, 0xBB, 0xCC, 0xDD));
    gx.load_light_obj_imm(&lt, GX_LIGHT2);
    let f = gx.take_fifo();
    assert_eq!(&f[..5], &[0x10, 0x00, 0x0F, 0x06, 0x20]);
    assert_eq!(&f[17..21], &[0xAA, 0xBB, 0xCC, 0xDD]);
    assert_eq!(f.len(), 5 + 64);
}

#[test]
fn project_identity() {
    let mtx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    // Orthographic: pm[0] = 1.
    let pm = [1.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0];
    let vp = [0.0, 0.0, 640.0, 480.0, 0.0, 1.0];
    let (sx, sy, sz) = project(0.5, 0.5, -0.25, &mtx, &pm, &vp);
    assert_eq!((sx, sy, sz), (480.0, 120.0, 1.25));
}

//! Flat-colored quads in the current camera's view (`hsd_80391A04`'s debug drawing setup and
//! the quads the background flash draws with it, `fn_8001FEC4`).

use melee_sim::hsd::display::Quad;

use super::draw::Ctx;
use super::lobj::ChannelLights;
use super::state::States;
use crate::gx::enums::*;

impl Ctx<'_> {
    /// `hsd_80391A04(1, 1, 1)`: positions in the camera's view, a color per vertex, lines and
    /// points one wide, no culling.
    pub fn debug_draw_setup(&mut self) {
        self.clear_vtx_desc();
        self.gx.set_current_mtx(0);
        let view = self.hsd.view_mtx();
        self.gx.load_pos_mtx_imm(&view.m, 0);
        self.hsd.state.set_line_width(self.gx, 1, 5);
        self.hsd.state.set_point_size(self.gx, 2, 5);
        let objs = &self.game.objs;
        let mut lights = ChannelLights {
            state: &mut self.hsd.lights,
            objs,
        };
        States {
            state: &mut self.hsd.state,
            tev: &mut self.hsd.tev,
        }
        .setup_render_mode(self.gx, 0x6800_0002, None, &mut lights);
        self.gx
            .set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_TEX_S, GX_RGBA6, 0);
        self.gx
            .set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_TEX_ST, GX_RGBA8, 0);
        self.gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        self.gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
        self.hsd.state.set_cull_mode(self.gx, 0);
    }

    /// The background flash's quads, after one setup.
    pub fn quads(&mut self, quads: &[Quad]) {
        self.debug_draw_setup();
        for q in quads {
            let [r, g, b, a] = q.color;
            self.gx.begin(GX_QUADS, GX_VTXFMT0, 4);
            for (x, y) in [(q.x0, q.y0), (q.x1, q.y0), (q.x1, q.y1), (q.x0, q.y1)] {
                self.gx.position2f32(x, y);
                self.gx.color4u8(r, g, b, a);
            }
            self.gx.end();
        }
    }
}

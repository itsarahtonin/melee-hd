//! GXDisplayList.c: building and calling display lists.

use super::{Gx, GxData};

/// What `GXBeginDisplayList` puts aside: the FIFO's bytes (the CPU FIFO it switches away
/// from) and, with `dlSaveContext`, the state (`__savedGXdata`).
#[derive(Clone, Debug)]
pub(super) struct Saved {
    fifo: Vec<u8>,
    data: Option<Box<GxData>>,
}

impl Gx {
    /// `GXBeginDisplayList`: later writes build a display list, which `end_display_list`
    /// returns, instead of going to the FIFO. The list's buffer and size aren't modelled.
    #[doc(alias = "GXBeginDisplayList")]
    pub fn begin_display_list(&mut self) {
        if self.data.dirty_state != 0 {
            self.set_dirty_state();
        }
        let data = (self.data.dl_save_context != 0).then(|| Box::new(self.data.clone()));
        self.data.in_disp_list = 1;
        let fifo = std::mem::take(&mut self.fifo);
        self.display_list = Some(Saved { fifo, data });
    }

    /// `GXEndDisplayList`: the list's bytes (the original returns their count). With
    /// `dlSaveContext` the state goes back to what it was at `begin_display_list`, but for
    /// `cpEnable`.
    #[doc(alias = "GXEndDisplayList")]
    pub fn end_display_list(&mut self) -> Vec<u8> {
        if self.data.dirty_state != 0 {
            self.set_dirty_state();
        }
        let list = match self.display_list.take() {
            Some(saved) => {
                let list = std::mem::replace(&mut self.fifo, saved.fifo);
                if let Some(data) = saved.data {
                    let cp_enable = self.data.cp_enable;
                    self.data = *data;
                    self.data.cp_enable = cp_enable;
                }
                list
            }
            None => Vec::new(),
        };
        self.data.in_disp_list = 0;
        list
    }

    /// `GXCallDisplayList`: `list` is the address the game passes, written as it is.
    #[doc(alias = "GXCallDisplayList")]
    pub fn call_display_list(&mut self, list: u32, nbytes: u32) {
        if self.data.dirty_state != 0 {
            self.set_dirty_state();
        }
        // *(u32*) &gx->unk == 0: both unk and bpSent.
        if self.data.unk == 0 && self.data.bp_sent == 0 {
            self.send_flush_prim();
        }
        self.write_u8(0x40);
        self.write_u32(list);
        self.write_u32(nbytes);
    }
}

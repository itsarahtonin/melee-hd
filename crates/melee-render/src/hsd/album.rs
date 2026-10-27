//! The album's images (`mnsnap.c`): the image descriptors the album wrote in the menus'
//! archive (`melee_sim::game::mn::snap::Album::descs`), which its thumbnails and whole view
//! show, and the snapshots decoded into its four buffers.

use melee_data::archive::DataRef;
use melee_sim::game::mn::snap::AlbumTexels;
use melee_sim::lb::snap_read::IMAGE_SIZE;

use super::draw::{Ctx, Image};
use crate::mem::Dynamic;

impl Ctx<'_> {
    /// The image descriptor `d` as the album wrote it, when it wrote it: the blank texels in
    /// the archive, or one of its images, whose bytes the display copies from the state when
    /// a decode changed them.
    pub fn album_image(&mut self, d: DataRef) -> Option<Image> {
        let a = *self.game.data_menus.album.desc(d)?;
        let data = self.data;
        let ptr = match a.texels {
            AlbumTexels::Data(t) => self.space.addr(data, t),
            AlbumTexels::Buffer(b) => {
                let images = &self.game.snap_read.images;
                let Some(i) = images.iter().position(|m| m.addr == b) else {
                    self.hsd
                        .note_once("an album image drawn before its first decode");
                    return None;
                };
                let img = &images[i];
                let at = self.space.dynamic(Dynamic::Album(i as u8), IMAGE_SIZE);
                let key = (img.addr, img.version);
                if self.hsd.album.get(i) != Some(&key) {
                    self.space.write(at, &img.bytes);
                    if self.hsd.album.len() <= i {
                        self.hsd.album.resize(i + 1, (0, 0));
                    }
                    self.hsd.album[i] = key;
                }
                at
            }
        };
        Some(Image {
            ptr,
            width: a.width,
            height: a.height,
            format: data.u32(d.at(8)),
            mipmap: data.u32(d.at(0xC)) != 0,
            min_lod: data.f32(d.at(0x10)),
            max_lod: data.f32(d.at(0x14)),
        })
    }
}

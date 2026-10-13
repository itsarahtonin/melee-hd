//! GXVert.h's inline writers: commands, parameters and vertex attributes straight into the
//! FIFO, big-endian. Each is named after its C function without the `GX` prefix.

use super::Gx;

/// Writers of one value: `name(x)`.
macro_rules! one {
    ($($name:ident: $alias:literal, $t:ty, $w:ident;)*) => {$(
        #[doc(alias = $alias)]
        #[inline]
        pub fn $name(&mut self, x: $t) {
            self.$w(x);
        }
    )*};
}

/// Writers of two values.
macro_rules! two {
    ($($name:ident: $alias:literal, $t:ty, $w:ident;)*) => {$(
        #[doc(alias = $alias)]
        #[inline]
        pub fn $name(&mut self, x: $t, y: $t) {
            self.$w(x);
            self.$w(y);
        }
    )*};
}

/// Writers of three values.
macro_rules! three {
    ($($name:ident: $alias:literal, $t:ty, $w:ident;)*) => {$(
        #[doc(alias = $alias)]
        #[inline]
        pub fn $name(&mut self, x: $t, y: $t, z: $t) {
            self.$w(x);
            self.$w(y);
            self.$w(z);
        }
    )*};
}

/// Writers of four values.
macro_rules! four {
    ($($name:ident: $alias:literal, $t:ty, $w:ident;)*) => {$(
        #[doc(alias = $alias)]
        #[inline]
        pub fn $name(&mut self, x: $t, y: $t, z: $t, w: $t) {
            self.$w(x);
            self.$w(y);
            self.$w(z);
            self.$w(w);
        }
    )*};
}

impl Gx {
    #[inline]
    fn put_u8(&mut self, x: u8) {
        self.write_u8(x);
    }

    #[inline]
    fn put_s8(&mut self, x: i8) {
        self.write_u8(x as u8);
    }

    #[inline]
    fn put_u16(&mut self, x: u16) {
        self.write_u16(x);
    }

    #[inline]
    fn put_s16(&mut self, x: i16) {
        self.write_u16(x as u16);
    }

    #[inline]
    fn put_u32(&mut self, x: u32) {
        self.write_u32(x);
    }

    #[inline]
    fn put_s32(&mut self, x: i32) {
        self.write_u32(x as u32);
    }

    #[inline]
    fn put_f32(&mut self, x: f32) {
        self.write_f32(x);
    }

    one! {
        cmd1u8: "GXCmd1u8", u8, put_u8;
        cmd1u16: "GXCmd1u16", u16, put_u16;
        cmd1u32: "GXCmd1u32", u32, put_u32;
        param1u8: "GXParam1u8", u8, put_u8;
        param1u16: "GXParam1u16", u16, put_u16;
        param1u32: "GXParam1u32", u32, put_u32;
        param1s8: "GXParam1s8", i8, put_s8;
        param1s16: "GXParam1s16", i16, put_s16;
        param1s32: "GXParam1s32", i32, put_s32;
        param1f32: "GXParam1f32", f32, put_f32;
        position1x16: "GXPosition1x16", u16, put_u16;
        position1x8: "GXPosition1x8", u8, put_u8;
        normal1x16: "GXNormal1x16", u16, put_u16;
        normal1x8: "GXNormal1x8", u8, put_u8;
        color1u32: "GXColor1u32", u32, put_u32;
        color1u16: "GXColor1u16", u16, put_u16;
        color1x16: "GXColor1x16", u16, put_u16;
        color1x8: "GXColor1x8", u8, put_u8;
        tex_coord1f32: "GXTexCoord1f32", f32, put_f32;
        tex_coord1s16: "GXTexCoord1s16", i16, put_s16;
        tex_coord1u16: "GXTexCoord1u16", u16, put_u16;
        tex_coord1s8: "GXTexCoord1s8", i8, put_s8;
        tex_coord1u8: "GXTexCoord1u8", u8, put_u8;
        tex_coord1x16: "GXTexCoord1x16", u16, put_u16;
        tex_coord1x8: "GXTexCoord1x8", u8, put_u8;
        matrix_index1u8: "GXMatrixIndex1u8", u8, put_u8;
    }

    two! {
        position2f32: "GXPosition2f32", f32, put_f32;
        position2u8: "GXPosition2u8", u8, put_u8;
        position2s8: "GXPosition2s8", i8, put_s8;
        position2u16: "GXPosition2u16", u16, put_u16;
        position2s16: "GXPosition2s16", i16, put_s16;
        tex_coord2f32: "GXTexCoord2f32", f32, put_f32;
        tex_coord2s16: "GXTexCoord2s16", i16, put_s16;
        tex_coord2u16: "GXTexCoord2u16", u16, put_u16;
        tex_coord2s8: "GXTexCoord2s8", i8, put_s8;
        tex_coord2u8: "GXTexCoord2u8", u8, put_u8;
    }

    three! {
        param3f32: "GXParam3f32", f32, put_f32;
        position3f32: "GXPosition3f32", f32, put_f32;
        position3u8: "GXPosition3u8", u8, put_u8;
        position3s8: "GXPosition3s8", i8, put_s8;
        position3u16: "GXPosition3u16", u16, put_u16;
        position3s16: "GXPosition3s16", i16, put_s16;
        normal3f32: "GXNormal3f32", f32, put_f32;
        normal3s16: "GXNormal3s16", i16, put_s16;
        normal3s8: "GXNormal3s8", i8, put_s8;
        color3u8: "GXColor3u8", u8, put_u8;
    }

    four! {
        param4f32: "GXParam4f32", f32, put_f32;
        color4u8: "GXColor4u8", u8, put_u8;
    }
}

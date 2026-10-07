//! What every HSD object shares (`sysdolphin/baselib/object.h`): a reference count that decides
//! when it is destroyed.

/// `HSD_Obj::ref_count`. A new object holds 0, which counts its first owner; each further
/// owner adds one. `NOREF` (0xFFFF) marks an object destroyed on every release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefCount(pub u16);

/// `HSD_OBJ_NOREF`.
pub const NOREF: u16 = 0xFFFF;

impl RefCount {
    /// `ref_INC`.
    pub fn inc(&mut self) {
        self.0 = self.0.wrapping_add(1);
        assert_ne!(self.0, NOREF, "reference count overflow");
    }

    /// `ref_DEC`: drops one owner; true when it was the last (or the count is `NOREF`), and
    /// the caller destroys the object.
    pub fn dec(&mut self) -> bool {
        if self.0 == NOREF {
            return true;
        }
        let was = self.0;
        self.0 = was.wrapping_sub(1);
        was == 0
    }
}

//! The movie player (`melee/lb/lbmthp.c`): what the boot and each scene's end do with it.
//! Playing a movie is task 5's.

/// `MoviePlayer`, as far as the boot reaches it: whether a movie is open (`power`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MoviePlayer {
    pub power: bool,
}

impl MoviePlayer {
    /// `lbMthp_8001F87C`, at boot: no movie open.
    #[doc(alias = "lbMthp_8001F87C")]
    pub fn init(&mut self) {
        self.power = false;
    }

    /// `lbMthp_8001F800`, after each scene: an open movie is closed (its reads finished,
    /// its alarm cancelled, its buffer freed).
    #[doc(alias = "lbMthp_8001F800")]
    pub fn release(&mut self) {
        assert!(
            !self.power,
            "closing a movie (lbMthp_8001F800) isn't ported"
        );
    }
}

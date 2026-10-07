//! The game's random numbers: HSD's linear congruential generator (`sysdolphin/baselib/random.c`).

/// The generator's state, `seed` in the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    pub seed: u32,
}

impl Default for Rng {
    /// The original's static initial seed.
    fn default() -> Self {
        Rng { seed: 1 }
    }
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng { seed }
    }

    /// Advances the seed and returns its top 16 bits, `HSD_Rand`.
    #[doc(alias = "HSD_Rand")]
    pub fn draw(&mut self) -> i32 {
        self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
        (self.seed >> 16) as i32
    }

    /// A float in [0, 1), `HSD_Randf`.
    #[doc(alias = "HSD_Randf")]
    pub fn draw_f32(&mut self) -> f32 {
        // A 16-bit integer converts exactly, and dividing by 2^16 is exact.
        self.draw() as f32 / 65536.0
    }

    /// An integer in [0, max) for positive `max`, `HSD_Randi`: the product wraps and the
    /// division truncates toward zero, as the original's 32-bit C arithmetic does.
    #[doc(alias = "HSD_Randi")]
    pub fn below(&mut self, max: i32) -> i32 {
        max.wrapping_mul(self.draw()) / 65536
    }
}

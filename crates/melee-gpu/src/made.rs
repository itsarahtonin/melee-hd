//! What the renderers made on first use, counted for the whole process with the time it took:
//! textures decoded and made, meshes decoded and put on the GPU, bind groups, buffers grown and
//! pipelines made at a draw. The benches take the difference over a frame
//! ([`Made::now`], [`Made::since`]) to say what a frame that ran long made (docs/fast-renderer.md,
//! F5's fourth round). Counting costs only where something is made.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// What's made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// A texture decoded from memory and made.
    Texture,
    /// A display list decoded into a mesh (melee-render's mesh cache).
    Mesh,
    /// A mesh put on the GPU (its words and indices written).
    Upload,
    /// The meshes' pools grown, or closed up with every mesh written again.
    Pool,
    /// A texture bind group.
    BindGroup,
    /// A buffer made larger.
    Buffer,
    /// A pipeline made at a draw (the uber shader's, not in the saved list).
    Pipeline,
    /// A copy from the EFB's texture made.
    CopyTexture,
    /// A material's record made (F4).
    Material,
    /// A draw call drawn with the uber shader while its specialized pipeline is made (no time).
    UberDraw,
}

impl What {
    pub const ALL: [What; 10] = [
        What::Texture,
        What::Mesh,
        What::Upload,
        What::Pool,
        What::BindGroup,
        What::Buffer,
        What::Pipeline,
        What::CopyTexture,
        What::Material,
        What::UberDraw,
    ];

    pub fn name(self) -> &'static str {
        match self {
            What::Texture => "textures",
            What::Mesh => "meshes",
            What::Upload => "uploads",
            What::Pool => "pools",
            What::BindGroup => "bind_groups",
            What::Buffer => "buffers",
            What::Pipeline => "pipelines",
            What::CopyTexture => "copy_textures",
            What::Material => "materials",
            What::UberDraw => "uber_draws",
        }
    }
}

const N: usize = What::ALL.len();

static COUNTS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static NANOS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

/// Counts one of `what`, made since `start`.
pub fn add(what: What, start: Instant) {
    COUNTS[what as usize].fetch_add(1, Relaxed);
    NANOS[what as usize].fetch_add(start.elapsed().as_nanos() as u64, Relaxed);
}

/// Counts one of `what`, untimed.
pub fn count(what: What) {
    COUNTS[what as usize].fetch_add(1, Relaxed);
}

/// Counts and times, of everything made so far or over a span.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Made {
    pub counts: [u64; N],
    pub nanos: [u64; N],
}

impl Made {
    /// What's been made so far.
    pub fn now() -> Made {
        Made {
            counts: std::array::from_fn(|i| COUNTS[i].load(Relaxed)),
            nanos: std::array::from_fn(|i| NANOS[i].load(Relaxed)),
        }
    }

    /// What's been made since `self` was taken.
    pub fn since(&self) -> Made {
        let now = Made::now();
        Made {
            counts: std::array::from_fn(|i| now.counts[i] - self.counts[i]),
            nanos: std::array::from_fn(|i| now.nanos[i] - self.nanos[i]),
        }
    }

    pub fn count(&self, what: What) -> u64 {
        self.counts[what as usize]
    }

    pub fn time(&self, what: What) -> Duration {
        Duration::from_nanos(self.nanos[what as usize])
    }

    /// Whether anything was made.
    pub fn any(&self) -> bool {
        self.counts.iter().any(|&c| c > 0)
    }

    /// What was made, as "3 textures 1.20 ms, 2 meshes 0.40 ms".
    pub fn summary(&self) -> String {
        What::ALL
            .iter()
            .filter(|&&w| self.count(w) > 0)
            .map(|&w| {
                format!(
                    "{} {} {:.2} ms",
                    self.count(w),
                    w.name(),
                    self.time(w).as_secs_f64() * 1000.0
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

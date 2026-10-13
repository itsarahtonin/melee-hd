//! Plays silence on this machine's default output device as the game loop does (a tick's
//! samples 60 times a second) and checks it never ran dry while the samples came on time,
//! printing the queue it kept. Ignored: it needs an audio device. `MELEE_AUDIO_EXCLUSIVE=1`
//! opens the device exclusively (Windows).
//!
//! ```sh
//! cargo test --release -p melee-audio --test device -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use melee_audio::{Output, OutputOptions};

#[test]
#[ignore = "needs an audio output device"]
fn plays_ticks_without_running_dry() {
    let exclusive = std::env::var_os("MELEE_AUDIO_EXCLUSIVE").is_some();
    let out = Output::open_with(OutputOptions { exclusive }).expect("an audio device");
    eprintln!("{} Hz: {}", out.rate, out.describe);
    let per_tick = 32_000.0 / 59.94;
    let silence = vec![[0i16; 2]; 600];
    let mut pushed = 0.0f64;
    let (mut ring, mut device, mut n) = (Duration::ZERO, Duration::ZERO, 0u32);
    let mut late = Vec::new();
    let start = Instant::now();
    let seconds = 10;
    for tick in 0..60 * seconds {
        let due = start + Duration::from_secs_f64(f64::from(tick) / 60.0);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        late.push(Instant::now() - due);
        let k = ((pushed + per_tick) as usize) - pushed as usize;
        out.push(&silence[..k]);
        pushed += per_tick;
        // The first two seconds settle.
        if tick >= 120 {
            let s = out.stats();
            ring += s.ring;
            device += s.device;
            n += 1;
        }
    }
    let s = out.stats();
    late.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    eprintln!(
        "ring at a push {:.2} ms, device {:.2} ms (means), {}-sample callbacks, {} underruns, {} backlogs dropped; pushes late by {:.2} ms at the median, {:.2} at worst",
        ms(ring) / f64::from(n),
        ms(device) / f64::from(n),
        s.callback_frames,
        s.underruns,
        s.skips,
        ms(late[late.len() / 2]),
        ms(late[late.len() - 1]),
    );
    // A busy machine can wake this thread late; the output may run dry only then.
    let worst = late[late.len() - 1];
    assert!(
        s.underruns == 0 || worst > Duration::from_millis(4),
        "ran dry with the samples on time"
    );
    assert_eq!(s.skips, 0, "dropped a backlog");
}

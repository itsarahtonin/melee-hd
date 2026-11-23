//! A match's textures and meshes made before its first frame (`prepare_match`, F5's fourth
//! round): the frames drawn are the frames a renderer that made nothing beforehand draws, and the
//! first frames decode next to none of the meshes the prepared renderer holds. Skips without the
//! disc or a hardware GPU.

use crate::{Gpu, Session, gpu_for};

/// A fast renderer at the console's size, its frames read back.
fn renderer(gpu: &crate::present::Gpu) -> Gpu {
    let mut r = crate::pipelines::renderer(gpu, 1, 1, crate::present::Aspect::GAME);
    r.xfb_readback = true;
    gpu_for(r, false, true)
}

#[test]
fn a_prepared_match_draws_the_same_frames_and_decodes_next_to_nothing() {
    crate::on_game_stack(|| {
        let Ok(disc) = std::env::var("SSBM_DISC") else {
            melee_local_data::skip!("skipped: no disc");
            return;
        };
        let Some((instance, adapter)) = melee_gpu::testing::instance_and_adapter() else {
            eprintln!("skipped: no hardware GPU");
            return;
        };
        let gpu = crate::present::Gpu::with_adapter(instance, adapter);
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.replay = Some(crate::predict_tests::REPLAY.to_owned());
        let mut s = Session::start(&args).expect("the replay's match");
        let new = |s: &Session| {
            let mut f = melee_render::frame::Frames::new();
            f.space.load_dol(&s.dol);
            (f, renderer(&gpu))
        };
        let (mut plain_frames, mut plain) = new(&s);
        let (mut frames, mut prepared) = new(&s);
        let rest = [melee_sim::input::pad::PadStatus::default(); 4];
        for tick in 0..30 {
            let f = s.tick(rest);
            if tick == 0 {
                crate::prepare_match(&mut prepared, &frames, &s);
                let held = prepared.meshes().expect("the mesh cache").size().0;
                assert!(held > 100, "the match's meshes made: {held}");
            }
            if !f.draws.is_empty() {
                let (game, data) = (s.game(), s.data());
                let a = plain.draw(&mut plain_frames, game, data, &f.draws, None);
                let b = prepared.draw(&mut frames, game, data, &f.draws, None);
                assert!(
                    a.last().map(|i| &i.rgba) == b.last().map(|i| &i.rgba),
                    "tick {tick}: the prepared renderer's frame"
                );
            }
            s.recycle(f);
        }
        let (p, q) = (
            plain.meshes().unwrap().stats,
            prepared.meshes().unwrap().stats,
        );
        assert!(
            q.decoded * 10 <= p.decoded,
            "decoded at draws: {} prepared, {} not",
            q.decoded,
            p.decoded
        );
    });
}

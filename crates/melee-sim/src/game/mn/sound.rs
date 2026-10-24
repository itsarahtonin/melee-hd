//! The sound options (`melee/mn/mnsound.c`): up and down move between stereo or mono and the
//! music and sound balance; left and right switch the mix (at once, through the synth and
//! the console's SRAM) or move the balance by 5 between all sound (-100) and all music
//! (100), setting the announcer's and the effects' volumes as they go. B leaves for the
//! Options list with the balance saved.

use gekko_fp as fp;

use super::super::gm::title::{AnimLoop, anim_frame};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_BACK, IN_DOWN, IN_UP};
use super::options::{
    EXIT_TO, HIDDEN, MOBJ_MASK, OptionMenu, SETTINGS, SFX_BACK, SFX_MOVE, SOUND, al,
};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};

/// The processes.
pub const INPUT: u32 = 0x8024_92CC;
pub const THINK: u32 = 0x8024_9A1C;

/// `mnSound_803EEED8`: the chosen mix's flashing, each row's cursor (the mix's, the
/// balance's), and each mix's switch (stereo's, mono's).
const LOOPS: [AnimLoop; 5] = [
    al(0.0, 199.0, 0.0),
    al(0.0, 29.0, 0.0),
    al(30.0, 59.0, 30.0),
    al(10.0, 15.0, -0.1),
    al(0.0, 5.0, -0.1),
];
/// `volume_left`, `volume_right`: the balance's cursor pointing each way.
const VOLUME_LEFT: AnimLoop = al(0.0, 3.0, -0.1);
const VOLUME_RIGHT: AnimLoop = al(4.0, 7.0, -0.1);

/// `mnsound.c`'s statics: the model (`mnSound_804A08A8`) and the screen's GObj
/// (`mnSound_804D6C30`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Sound {
    pub model: super::super::gm::title::ModelDesc,
    pub gobj: Option<GObjId>,
}

impl Running<'_> {
    fn sound_gobj(&self) -> GObjId {
        self.game.options.sound.gobj.expect("the sound screen")
    }

    fn sound_menu(&self) -> OptionMenu {
        self.option_menu(self.sound_gobj()).1
    }

    fn sound_menu_mut(&mut self) -> &mut OptionMenu {
        let g = self.sound_gobj();
        self.option_menu_mut(g)
    }

    /// `gmMainLib_8015ED74`: the save's sound balance.
    #[doc(alias = "gmMainLib_8015ED74")]
    pub fn sound_balance(&self) -> u8 {
        self.game.game_data.thing.save_data.x1CB0.sound_balance as u8
    }

    /// `gmMainLib_8015ED80`.
    #[doc(alias = "gmMainLib_8015ED80")]
    pub fn set_sound_balance(&mut self, b: i8) {
        self.game.game_data.thing.save_data.x1CB0.sound_balance = b;
    }

    /// `updateCenterText`: the row's description.
    fn sound_center_text(&mut self) {
        let m = self.sound_menu();
        if let Some(t) = m.text {
            self.destroy_text(t);
        }
        let id = if m.unk2 == Some(0) { 0xBB } else { 0xBC };
        let t = self.menu_center_text(1, id);
        self.sound_menu_mut().text = Some(t);
    }

    /// `updateChannelAnim`: the mix's switch at `channel`, its two lamps held where they are.
    fn sound_channel_anim(&mut self, j: JObjId, channel: u8) {
        let s = self.options_node(Some(j), 8);
        let l = self.options_node(Some(j), 10);
        let r = self.options_node(Some(j), 9);
        let lf = anim_frame(self, l.expect("the stereo lamp"));
        let rf = anim_frame(self, r.expect("the mono lamp"));
        self.options_set_frame(s, f32::from(channel));
        self.options_set_frame_stopped(l, lf, MOBJ_MASK);
        self.options_set_frame_stopped(r, rf, MOBJ_MASK);
    }

    /// `updateVolumeAnim`: the balance's cursor at `mix` between its two ends, turned the
    /// way it moved (`direction`), and the volumes set from the balance (`gm_801602C0`).
    #[doc(alias = "gm_801602C0")]
    fn sound_volume_anim(&mut self, j: JObjId, mix: u8, direction: i32) {
        let c = self.options_node(Some(j), 6);
        let a = self.options_node(Some(j), 3);
        let b = self.options_node(Some(j), 4);
        let p0 = self.game.objs.jobjs[a.expect("the balance's left end")].translate;
        let p1 = self.game.objs.jobjs[b.expect("the balance's right end")].translate;
        let x = fp::fmadds(
            fp::fdivs(fp::frsp(f64::from(i32::from(mix as i8) + 100)), 200.0),
            fp::fsubs(reg(p1.x), reg(p0.x)),
            reg(p0.x),
        );
        let c = c.expect("the balance's cursor");
        jobj::set_srt(&mut self.game.objs.jobjs, c, |o| o.translate.x = mem(x));
        if direction != 0 {
            let f = if direction < 0 {
                VOLUME_LEFT.start
            } else {
                VOLUME_RIGHT.start
            };
            jobj::req_anim_all(&mut self.game.objs, Some(c), f);
        }
        self.game.audio.lb_apply_balance(mix as i8);
    }

    /// `mnSound_802492CC`, the screen's input: B leaves (the balance saved and the save
    /// written), up or down change rows, left and right switch to stereo or mono or move the
    /// balance toward the sounds or the music.
    #[doc(alias = "mnSound_802492CC")]
    pub fn sound_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        let m = self.sound_menu();
        let row = m.unk2.expect("the sound screen's row");
        let mode = m.unk1.expect("the sound screen's mix");
        let mix = m.unk3.expect("the sound screen's balance");
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.set_sound_balance(mix as i8);
            self.card_game_save_changes();
            self.go_to_list(SETTINGS, 1, EXIT_TO);
            return;
        }
        if ev & (IN_UP | IN_DOWN) != 0 {
            self.lb_audio_call(SFX_MOVE);
            self.sound_menu_mut().unk2 = Some(u8::from(row == 0));
            self.sound_center_text();
            return;
        }
        let j = self.options_jobj(self.sound_gobj());
        let (left, right) = (super::main::IN_LEFT, super::main::IN_RIGHT);
        let dir = if ev & left != 0 {
            -1
        } else if ev & right != 0 {
            1
        } else {
            return;
        };
        if row == 0 {
            let to = u8::from(dir > 0);
            if mode == to ^ 1 {
                self.lb_audio_call(SFX_MOVE);
                self.sound_menu_mut().unk1 = Some(to);
                self.sound_channel_anim(j, to);
                self.lb_set_sound_mode(i32::from(to));
            }
        } else if (dir < 0 && (mix as i8) > -100) || (dir > 0 && (mix as i8) < 100) {
            self.lb_audio_call(SFX_MOVE);
            let mix = (i32::from(mix) + 5 * dir) as u8;
            self.sound_menu_mut().unk3 = Some(mix);
            self.sound_volume_anim(j, mix, dir);
            self.set_sound_balance(mix as i8);
        }
    }

    /// `fn_80249A1C`, the screen's process: gone with its description when the flow leaves
    /// the screen; hidden for its first 20 ticks; then the chosen mix flashing on the mix's
    /// row, the balance's cursor turning, the rows' cursor and the mix's switch.
    #[doc(alias = "fn_80249A1C")]
    pub fn sound_think(&mut self, gobj: GObjId) {
        let (_, m) = self.option_menu(gobj);
        let j = self.options_jobj(gobj);
        if self.game.menu.flow.cur_menu != SOUND {
            // The original reads the description from the user data after freeing it with
            // the GObj (docs/melee-bugs.md).
            gobj::free(self, gobj);
            if let Some(t) = m.text {
                self.destroy_text(t);
            }
            return;
        }
        if m.cursor != 0 {
            let left = m.cursor - 1;
            self.option_menu_mut(gobj).cursor = left;
            let jobjs = &mut self.game.objs.jobjs;
            if left != 0 {
                jobj::set_flags_all(jobjs, Some(j), HIDDEN);
            } else {
                jobj::clear_flags_all(jobjs, Some(j), HIDDEN);
            }
            return;
        }
        let row = m.unk2.expect("the sound screen's row");
        let mode = m.unk1.expect("the sound screen's mix");
        if row == 0 {
            let lamp = self.options_node(Some(j), if mode == 0 { 0xA } else { 9 });
            self.anim_loop_stopped(lamp.expect("a mix's lamp"), LOOPS[0], MOBJ_MASK);
        }
        let c = self.options_node(Some(j), 6).expect("the balance's cursor");
        let f = reg(anim_frame(self, c));
        let l = if reg(VOLUME_LEFT.start) <= f && f <= reg(VOLUME_LEFT.end) {
            VOLUME_LEFT
        } else {
            VOLUME_RIGHT
        };
        self.anim_loop(c, l);
        let s = self.options_node(Some(j), 0xE).expect("the rows' cursor");
        self.anim_loop(s, LOOPS[usize::from(row) + 1]);
        let s = self.options_node(Some(j), 0xB).expect("the mix's switch");
        self.anim_loop(s, LOOPS[usize::from(mode) + 3]);
    }

    /// `mnSound_80249C08`: the screen on render link 4, hidden, its user data (20 ticks
    /// hidden, the console's mix, the mix's row, the save's balance) and process, the
    /// description, the switch, the balance's cursor and the rows' cursor.
    #[doc(alias = "mnSound_80249C08")]
    fn sound_make(&mut self) {
        let m = self.game.options.sound.model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.sound.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the sound screen");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(8);
        let mode = self.lb_get_sound_mode();
        let menu = OptionMenu {
            cursor: 0x14,
            unk1: Some(mode),
            unk2: Some(0),
            unk3: Some(self.sound_balance()),
            text: None,
        };
        let g = &mut *self.game;
        g.options.menus.insert(addr, menu);
        g.world.init_user_data(gobj, 0, UserData::OptionMenu(addr));
        let p = g.world.setup_proc(gobj, CodeRef(THINK), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        self.sound_center_text();
        self.sound_channel_anim(j, mode);
        let c = self.options_node(Some(j), 6);
        self.options_set_frame(c, VOLUME_LEFT.end);
        self.sound_volume_anim(j, menu.unk3.expect("the balance"), 0);
        let s = self.options_node(Some(j), 0xE).expect("the rows' cursor");
        self.anim_loop(s, LOOPS[1]);
        let s = self.options_node(Some(j), 0xB);
        self.options_set_frame(s, LOOPS[usize::from(mode) + 3].end);
        jobj::set_flags_all(&mut self.game.objs.jobjs, Some(j), HIDDEN);
    }

    /// `mnSound_8024A09C`, from the Options list: the flow on the sound screen (0x14), its
    /// model from the menus' archive, the screen and its input process.
    #[doc(alias = "mnSound_8024A09C")]
    pub fn sound_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SOUND;
        f.hovered_selection = 0;
        self.game.options.sound.model = self.menu_model("MenMainConSo_Top");
        self.sound_make();
        self.options_input_proc(INPUT);
    }

    /// `lbAudioAx_80024BD0`: the console's mix (`HSD_SynthGetSoundMode`, `OSGetSoundMode`)
    /// noted, and 0 for stereo, 1 for mono.
    #[doc(alias = "lbAudioAx_80024BD0")]
    #[doc(alias = "HSD_SynthGetSoundMode")]
    pub fn lb_get_sound_mode(&mut self) -> u8 {
        let mode = u32::from(!self.game.audio.mono);
        self.game.audio.lb.sound_mode = mode;
        u8::from(mode != 1)
    }

    /// `lbAudioAx_80024C08`: the mix set to stereo (0) or mono (1) when it isn't already
    /// (`to_synth_mode`), through the synth (`HSD_SynthSetSoundMode`).
    #[doc(alias = "lbAudioAx_80024C08")]
    pub fn lb_set_sound_mode(&mut self, mode: i32) -> bool {
        const TO_SYNTH_MODE: [u32; 2] = [1, 0];
        if !(0..2).contains(&mode) {
            return false;
        }
        let to = TO_SYNTH_MODE[mode as usize];
        if self.game.audio.lb.sound_mode != to {
            self.game.audio.lb.sound_mode = to;
            self.synth_set_sound_mode(to);
        }
        true
    }

    /// `HSD_SynthSetSoundMode`: the synth's mix, every playing effect's voices mixed again,
    /// and the console's SRAM set (`OSSetSoundMode`).
    #[doc(alias = "HSD_SynthSetSoundMode")]
    pub fn synth_set_sound_mode(&mut self, mode: u32) {
        let a = &mut self.game.audio;
        a.synth.sound_mode = mode;
        for n in 0..a.synth.nodes.len() {
            if a.synth.nodes[n].id > 0 {
                a.synth_update_mix(n, true);
            }
        }
        self.os_set_sound_mode(mode);
    }

    /// `OSSetSoundMode`: the console's SRAM's stereo flag set from the synth's mode, written
    /// back to the real-time clock's SRAM when it changes (as `OSSetProgressiveMode`'s,
    /// `SRAM_WRITE`). The SRAM is the console's setting: the frontend keeps it.
    pub fn os_set_sound_mode(&mut self, mode: u32) {
        let mono = mode & 1 == 0;
        if self.game.audio.mono == mono {
            return;
        }
        self.game.audio.mono = mono;
        self.game
            .audio
            .clock_read(self.data, super::super::gm::progressive::SRAM_WRITE);
    }

    /// `lbAudioAx_80027AB0`: the sound banks named for the saved language's directory and
    /// every sound stopped; when the banks were loaded for the other language, its sound bank
    /// (the old one freed, `AXDriver_8038DCFC`), sound `id` and the scenes' banks loaded.
    #[doc(alias = "lbAudioAx_80027AB0")]
    pub fn lb_audio_language(&mut self, id: i32) {
        let us = self.game.game_data.saved_language() == super::super::gm::main_lib::LANG_US;
        let lb = &mut self.game.audio.lb;
        let lang = if us {
            lb.ssm_file[..11].copy_from_slice(b"/audio/us/\0");
            lb.ssm_stem_pos = 10;
            1
        } else {
            lb.ssm_file[..8].copy_from_slice(b"/audio/\0");
            lb.ssm_stem_pos = 7;
            0
        };
        self.game.audio.sfx_key_off_all();
        if self.game.audio.lb.lang == lang {
            return;
        }
        self.game.audio.lb.lang = lang;
        let a = &mut self.game.audio;
        let bank = a.lang_banks[lang as usize].expect("the language's sound bank (smash2.sem)");
        a.driver_replace_bank(self.data, bank);
        if id != 0x83D61 {
            self.sfx_play(id, 0x7F, 0x40);
        }
        self.game
            .audio
            .lb_reload_scene_banks(self.data, &[(0x33, 1), (1, 1), (0x36, 1)]);
        // The loads' waits ran the save's writer (`lb_8001CC84`).
        self.card_writer_take();
    }
}

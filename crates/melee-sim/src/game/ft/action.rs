//! Fighter action scripts (`ftaction.c`): each action state's command script, run alongside
//! its animation, creates hitboxes, effects and sounds and sets flags at given frames.

use gekko_fp as fp;

use super::super::state::Running;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::lb::command::{self, WAIT_FOR_FRAME};

/// Each fighter command's length in words (`ftAction_803C0870`), from opcode 10.
const COMMAND_WORDS: [u8; 49] = [
    5, 5, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 7, 4, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 2, 1, 4,
];

impl Running<'_> {
    /// Runs the action's commands due this frame, `ftAction_80073240`.
    #[doc(alias = "ftAction_80073240")]
    pub fn run_commands(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let frame = fp::fadds(reg(fp.cur_anim_frame), reg(fp.x898_unk));
        fp.x3E4_fighterCmdScript.script.frame_count = fp::stfs(frame);
        if fp.x3E4_fighterCmdScript.script.cursor.is_none() {
            return;
        }
        let s = &mut fp.x3E4_fighterCmdScript.script;
        if s.timer_f32() != WAIT_FOR_FRAME {
            s.timer = fp::stfs(fp::fsubs(reg(s.timer_f32()), reg(fp.frame_speed_mul)));
        }
        loop {
            let fp = &mut self.game.fighters[f.0];
            let fsm = fp.frame_speed_mul;
            let s = &mut fp.x3E4_fighterCmdScript.script;
            if s.cursor.is_none() {
                break;
            }
            if s.timer_f32() == WAIT_FOR_FRAME {
                if s.frame_count_f32() >= fsm {
                    break;
                }
                s.timer = fp::stfs(fp::fneg(reg(s.frame_count_f32())));
            } else if s.timer_f32() > 0.0 {
                break;
            }
            let code = s.opcode(self.data);
            if !command::execute(s, self.data, code) {
                self.fighter_command(gobj, code - 10);
            }
            if self.game.fighters[f.0]
                .x3E4_fighterCmdScript
                .script
                .timer_f32()
                == WAIT_FOR_FRAME
            {
                break;
            }
        }
    }

    /// Runs the commands due by the frame a state started at, the fighter's own through their
    /// catch-up versions, `ftAction_80073354`. The throw flags clear, and clear again whenever
    /// a command starts a wait that's already over.
    #[doc(alias = "ftAction_80073354")]
    pub fn catch_up_commands(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let frame = fp::fadds(reg(fp.cur_anim_frame), reg(fp.x898_unk));
        fp.x3E4_fighterCmdScript.script.frame_count = fp::stfs(frame);
        fp.x2210 = [0; 4];
        if fp.x3E4_fighterCmdScript.script.cursor.is_none() {
            return;
        }
        let s = &mut fp.x3E4_fighterCmdScript.script;
        if s.timer_f32() != WAIT_FOR_FRAME {
            s.timer = fp::stfs(fp::fsubs(reg(s.timer_f32()), reg(fp.frame_speed_mul)));
        }
        loop {
            let fp = &mut self.game.fighters[f.0];
            let fsm = fp.frame_speed_mul;
            let s = &mut fp.x3E4_fighterCmdScript.script;
            if s.cursor.is_none() {
                break;
            }
            if s.timer_f32() == WAIT_FOR_FRAME {
                if s.frame_count_f32() >= fsm {
                    break;
                }
                s.timer = fp::stfs(fp::fneg(reg(s.frame_count_f32())));
            } else if s.timer_f32() > 0.0 {
                break;
            }
            let timer = s.timer_f32();
            let code = s.opcode(self.data);
            if !command::execute(s, self.data, code) {
                self.catch_up_command(gobj, code - 10);
            }
            let fp = &mut self.game.fighters[f.0];
            let now = fp.x3E4_fighterCmdScript.script.timer_f32();
            if now != timer && now <= 0.0 {
                fp.x2210 = [0; 4];
            }
            if now == WAIT_FOR_FRAME {
                break;
            }
        }
    }

    /// Steps over the commands due this frame without running the fighter's own,
    /// `ftAction_8007349C`.
    #[doc(alias = "ftAction_8007349C")]
    pub fn skip_commands(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let frame = fp::fadds(reg(fp.cur_anim_frame), reg(fp.x898_unk));
        fp.x3E4_fighterCmdScript.script.frame_count = fp::stfs(frame);
        if fp.x3E4_fighterCmdScript.script.cursor.is_none() {
            return;
        }
        let fsm = fp.frame_speed_mul;
        let s = &mut fp.x3E4_fighterCmdScript.script;
        if s.timer_f32() != WAIT_FOR_FRAME {
            s.timer = fp::stfs(fp::fsubs(reg(s.timer_f32()), reg(fsm)));
        }
        loop {
            if s.cursor.is_none() {
                break;
            }
            if s.timer_f32() == WAIT_FOR_FRAME {
                if s.frame_count_f32() >= fsm {
                    break;
                }
                s.timer = fp::stfs(fp::fneg(reg(s.frame_count_f32())));
            } else if s.timer_f32() > 0.0 {
                break;
            }
            let code = s.opcode(self.data);
            if !command::execute(s, self.data, code) {
                let words = COMMAND_WORDS
                    .get((code - 10) as usize)
                    .unwrap_or_else(|| panic!("fighter command {code} doesn't exist"));
                s.skip(u32::from(*words));
            }
            if s.timer_f32() == WAIT_FOR_FRAME {
                break;
            }
        }
    }
}

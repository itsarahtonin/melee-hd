//! A player's match statistics (`plstale.c`, `plattack.c`, and the resets in `plbonus.c`
//! and `pl_040D.c`): the stale-move queue, attack counts, and the records bonuses are
//! scored from. The block's tail past the stale-move table (`xDB0`) is the rest of
//! `pl_StaleMoveTableExt_t` ([`super::types::StatsExt`]), which the original reaches by
//! casting the table.

use super::super::state::Game;
use super::types::AttackStats;

impl Game {
    /// Adds how far smash DI moved a player to their stats (`xCDC`, `xCE0`), `pl_800401F0`.
    #[doc(alias = "pl_800401F0")]
    pub fn add_sdi_distance(&mut self, slot: i32, _sub: i32, dx: f64, dy: f64) {
        use gekko_fp as fp;
        let abs = |v: f64| if v < fp::frsp(0.0) { fp::fneg(v) } else { v };
        let t = self.player_stale_moves(slot);
        t.sdi_distance_x = crate::fpu::mem(fp::fadds(crate::fpu::reg(t.sdi_distance_x), abs(dx)));
        t.sdi_distance_y = crate::fpu::mem(fp::fadds(crate::fpu::reg(t.sdi_distance_y), abs(dy)));
    }

    /// Clears every slot for a new match, `Player_InitAllPlayers`.
    #[doc(alias = "Player_InitAllPlayers")]
    pub fn init_all_players(&mut self) {
        self.init_attack_instance();
        self.init_attack_records();
        for i in 0..6 {
            self.reset_player(i);
            self.reset_player_records(i);
        }
    }

    /// Clears a slot and its records, `Player_80036CF0`.
    #[doc(alias = "Player_80036CF0")]
    pub fn reinit_player(&mut self, slot: i32) {
        self.reset_player(slot);
        self.reset_player_records(slot);
    }

    /// Clears a slot for a new opponent mid-match, `Player_80036D24`: the others forget it
    /// in their records.
    #[doc(alias = "Player_80036D24")]
    pub fn replace_player(&mut self, slot: i32) {
        self.reset_player(slot);
        self.forget_player_records(slot);
    }

    /// `plStale_InitAttackInstance`.
    #[doc(alias = "plStale_InitAttackInstance")]
    pub fn init_attack_instance(&mut self) {
        self.pl.stale_attack_instance = 1;
    }

    /// Takes the next attack instance, skipping 0, `plStale_IncrementAttackInstance`.
    #[doc(alias = "plStale_IncrementAttackInstance")]
    pub fn next_attack_instance(&mut self) -> u16 {
        let before = self.pl.stale_attack_instance;
        self.pl.stale_attack_instance = before.wrapping_add(1);
        if self.pl.stale_attack_instance == 0 {
            self.pl.stale_attack_instance = 1;
        }
        before
    }

    /// Empties a player's stale-move queue, `plStale_ResetStaleMoveTableForPlayer`.
    #[doc(alias = "plStale_ResetStaleMoveTableForPlayer")]
    pub fn reset_stale_moves(&mut self, slot: i32) {
        let t = self.player_stale_moves(slot);
        t.current_index = 0;
        for m in &mut t.queue {
            m.move_id = 0;
            m.attack_instance = 0;
        }
    }

    /// Records a hit by attack `id`, instance `instance`, in `slot`'s stale-move queue unless
    /// that hit is already there (the tail of `plStale_UpdateStaleMovesFromFighter` and
    /// `plStale_UpdateStaleMovesFromItem`).
    pub fn record_stale_move(&mut self, slot: i32, id: i32, instance: u16) {
        if id == 1 {
            return;
        }
        let t = self.player_stale_moves(slot);
        if t.queue
            .iter()
            .any(|m| id == i32::from(m.move_id) && instance == m.attack_instance)
        {
            return;
        }
        let i = t.current_index as usize;
        t.queue[i].move_id = id as u16;
        t.queue[i].attack_instance = instance;
        t.current_index = if t.current_index == 9 {
            0
        } else {
            t.current_index + 1
        };
    }

    /// `plAttack_80037590`.
    #[doc(alias = "plAttack_80037590")]
    pub fn init_attack_records(&mut self) {
        self.pl.attack_instance = 1;
    }

    /// Takes the next attack record number, skipping 0, `plAttack_80037B08`.
    #[doc(alias = "plAttack_80037B08")]
    pub fn next_attack_record(&mut self) -> u16 {
        let before = self.pl.attack_instance;
        self.pl.attack_instance = before.wrapping_add(1);
        if self.pl.attack_instance == 0 {
            self.pl.attack_instance = 1;
        }
        before
    }

    /// Clears a player's attack statistics, `plAttack_8003759C`.
    #[doc(alias = "plAttack_8003759C")]
    pub fn reset_action_stats(&mut self, slot: i32) {
        let s = self.player_action_stats(slot);
        clear_attack_stats(&mut s.attacks);
        clear_attack_stats(&mut s.hits);
        // `x358.x358_hits`, a third `AttackStats` the mirror keeps as bytes; its first
        // 0x194 bytes are the total and counts, then five counts and `x1A8`.
        s.landed.fill(0);
        s.victims.fill(0);
        s.front_hits = 0;
        s.other_hits = 0;
        s.back_hits = 0;
        s.down_hits = 0;
        s.turns = 0;
        s.squats = 0;
        s.walks = 0;
        s.dashes = 0;
        s.runs = 0;
        s.jumps = 0;
        s.air_jumps = 0;
        s.wall_bounces = 0;
        s.more_actions = [0; 8];
        s.has_turned = false;
        s.has_walked = false;
        s.has_jumped = false;
        s.has_air_jumped = false;
    }

    /// Clears a player's bonus records, `pl_8003891C`: the stale moves, the attack counts,
    /// the per-frame counters, the item logs and every record the bonuses read.
    #[doc(alias = "pl_8003891C")]
    pub fn reset_player_records(&mut self, slot: i32) {
        self.reset_stale_moves(slot);
        self.reset_action_stats(slot);
        self.reset_frame_counters(slot);
        self.reset_ko_ring(slot);
        self.reset_bob_omb_ring(slot);
        let p = self.player_mut(slot);
        let t = &mut p.stale_moves;
        t.items_picked_up = [0; 39];
        t.fresh_items_picked_up = [0; 39];
        t.container_runs = [0; 39];
        t.pokemon_released = [0; 30];
        t.bonuses = [0; 215];
        t.damage_taken = 0.0;
        t.peak_damage = 0.0;
        t.damage_recovered = 0;
        t.damage_dealt = 0.0;
        t.pummel_damage = 0.0;
        t.scope_damage = 0.0;
        t.damage_to = [0.0; 6];
        t.combo_step = 0;
        t.star_falls = 0;
        t.stage_damage = 0.0;
        t.average_percent = 0.0;
        t.victim_percent_sum = 0.0;
        t.victim_percent_count = 0;
        t.flowered_by = 6;
        t.up_kos = 0;
        t.last_ko_by = 6;
        t.last_ko_attack = [0; 4];
        let c = &mut t.last_ko_hit;
        c.attacker_percent = 0;
        c.victim_percent = 0;
        c.kb_applied1 = 0.0;
        c.item_thrower = 6;
        c.spike = false;
        c.spike_after_strong = false;
        c.metal = false;
        c.bunny_hood = false;
        c.starman = false;
        c.cloak = false;
        c.giant = false;
        c.tiny = false;
        c.landed = false;
        c.from_behind = false;
        c.at_edge = false;
        c.on_ledge = false;
        c.forward_attack = false;
        t.last_ko_instance = 0;
        t.powershields = 0;
        t.sdi_distance_y = 0.0;
        t.sdi_distance_x = 0.0;
        t.strong_hits_taken = 0;
        t.ledge_catches = 0;
        t.mash_frames = 0;
        t.mashes = 0;
        p.ext.presses = 0;
        p.ext.lowest_value = 0.0;
        p.stale_moves.ray_gun_hits = 0;
        let e = &mut p.ext;
        for v in [
            &mut e.longest_without_dealing,
            &mut e.since_dealing,
            &mut e.longest_without_taking,
            &mut e.since_taking,
            &mut e.long_streak_frames,
            &mut e.streak,
            &mut e.attack_0x68_frames,
            &mut e.air_frames,
            &mut e.ground_frames,
            &mut e.steep_frames,
            &mut e.nudged_frames,
            &mut e.edge_frames,
            &mut e.heavy_frames,
            &mut e.screw_attack_frames,
            &mut e.saturn_frames,
            &mut e.longest_hold,
            &mut e.hold_frames,
            &mut e.shortest_hold,
            &mut e.their_taunt_window,
            &mut e.own_taunt_window,
            &mut e.ko_window,
        ] {
            *v = 0;
        }
        p.ext.window_sdi = 0.0;
        p.ext.since_press = 0;
        p.ext.quickest_press = u32::MAX;
        p.ext.respawn_window = 0;
        p.ext.last_ko_by = 6;
        p.ext.altered_frames = 0;
        p.ext.item_thrower = 6;
        p.ext.kind_ea_kos = 0;
        p.ext.kind_ea_falls = 0;
        p.ext.crowd_reactions = 0;
        let e = &mut p.ext;
        for v in [
            &mut e.walked,
            &mut e.rose,
            &mut e.fell,
            &mut e.knocked,
            &mut e.farthest_knocked,
            &mut e.farthest_knocked_away,
            &mut e.center_distance,
        ] {
            *v = 0.0;
        }
        p.ext.center_distance_frames = 0;
        p.ext.opponent_distance = 0.0;
        p.ext.opponent_distance_frames = 0;
        // Neither `window_attacks` (`xDB0`), which this reset leaves, nor `window_heavy` (below).
        let e = &mut p.ext;
        for v in [
            &mut e.window_wall_bounces,
            &mut e.window_action_112,
            &mut e.window_action_99,
            &mut e.window_action_111,
            &mut e.window_hits,
            &mut e.window_crowd,
            &mut e.window_powershields,
            &mut e.window_presses,
            &mut e.hits_flag,
            &mut e.presses_flag,
        ] {
            *v = 0;
        }
        // `xDD0`'s and `xDD1`'s bits, one by one.
        p.ext.frame_flags = 0;
        p.ext.ko_flags = 0;
        p.ext.window_heavy = 0;
    }

    /// Clears a player's per-frame counters, `pl_80040DDC`.
    /// Counts damage healed toward the slot's statistics (`xC68`), `pl_80040B8C`: not for a
    /// sub-fighter.
    #[doc(alias = "pl_80040B8C")]
    pub fn add_healed(&mut self, slot: i32, sub: i32, amount: i32) {
        if sub != 0 {
            return;
        }
        let t = self.player_stale_moves(slot);
        t.damage_recovered = t.damage_recovered.wrapping_add(amount);
    }

    #[doc(alias = "pl_80040DDC")]
    pub fn reset_frame_counters(&mut self, slot: i32) {
        let t = self.player_combos(slot);
        t.last_victim = 0;
        t.last_damage = 0.0;
        t.combo_hits = 0;
        t.combo_count = 0;
        for e in &mut t.by_attacker {
            e.damage = 0.0;
            e.hits = 0;
            e.repeats = 0;
            e.attack = 0;
            e.air_hits = 0;
            e.shield_jabbed = false;
        }
    }

    /// Clears the ring of the attack instances that KOed (`x8C0`), `pl_80041524`.
    #[doc(alias("reset_x8c0"))]
    #[doc(alias = "pl_80041524")]
    pub fn reset_ko_ring(&mut self, slot: i32) {
        let t = &mut self.player_stale_moves(slot).ko_ring;
        t.next = 0;
        for e in &mut t.entries {
            e.instance = 0;
            e.kos = 0;
            e.self_ko = false;
        }
    }

    /// Clears the ring of the Bob-ombs thrown (`x8D8`), `pl_800418F4`.
    #[doc(alias("reset_x8d8"))]
    #[doc(alias = "pl_800418F4")]
    pub fn reset_bob_omb_ring(&mut self, slot: i32) {
        let t = &mut self.player_stale_moves(slot).bombhei_ring;
        t.next = 0;
        for e in &mut t.entries {
            e.spawn = 0;
            e.hit = false;
            e.used = false;
        }
    }

    /// A Bob-omb's hit on the player, by its spawn number (`Item::x1C`): the Bob-omb's entry
    /// among the five the player last threw (`x8D8`) is marked as having hit, `pl_80041B08`.
    #[doc(alias = "pl_80041B08")]
    pub fn count_bombhei_hit(&mut self, slot: i32, spawn: u16) {
        for e in &mut self.player_stale_moves(slot).bombhei_ring.entries {
            if u32::from(spawn) == e.spawn {
                e.hit = true;
            }
        }
    }

    /// Clears a player's bonus counts (`x904`), `fn_80038FB8`.
    #[doc(alias = "fn_80038FB8")]
    pub fn reset_bonus_counts(&mut self, slot: i32) {
        self.player_stale_moves(slot).bonuses = [0; 215];
    }

    /// Clears the records that follow who KO'd whom when a slot is replaced, `pl_80038F10`.
    #[doc(alias = "pl_80038F10")]
    pub fn forget_player_records(&mut self, slot: i32) {
        let p = self.player_mut(slot);
        p.ext.own_taunt_window = 0;
        p.ext.ko_window = 0;
        p.ext.respawn_window = 0;
        p.ext.last_ko_by = 6;
        for i in 0..6 {
            let o = self.player_mut(i);
            if slot == o.ext.last_ko_by as i32 {
                o.ext.respawn_window = 0;
                o.ext.last_ko_by = 6;
            }
            if slot == o.stale_moves.flowered_by as i32 {
                o.stale_moves.flowered_by = 6;
            }
            if slot == o.ext.item_thrower as i32 {
                o.ext.item_thrower = 6;
            }
        }
    }
}

/// `clearAttackStats`.
fn clear_attack_stats(s: &mut AttackStats) {
    s.by_attack_counts = [0; 100];
    s.total = 0;
    s.thrown_item_count = 0;
    s.aerials_count = 0;
    s.specials_count = 0;
    s.strikes_count = 0;
    s.reflectors_count = 0;
    s.spikes_count = 0;
}

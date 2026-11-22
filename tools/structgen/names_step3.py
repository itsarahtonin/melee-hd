"""Wave 6, step 3 (task 23): the port's names for the other generated structs' fields the decomp
names by offset (or leaves unknown), by struct (its C name): C name to Rust name, as `names.py`
holds steps 1 and 2's, which reads this file into `FIELD_NAMES`.

Names come from the decompilation where it has them (`x0_gobj` is `gobj`), else from the docs,
the ports' uses and ssbm-rs. A field nothing reads or writes is `pad_*` (with its offset) or
`unused_*`; one written and never read is `unread_*`.
"""

# A hitbox (`HitCapsule`: a fighter's, a thrown fighter's, an item's).
HIT_CAPSULE = {
    # The hit group: boxes of one group share their victims (`ftColl_80076808`).
    "x4": "group",
    # The damage as a whole number (`fctiwz`), at creation.
    "unk_count": "whole_damage",
    "x24": "kb_growth",
    # The weight-dependent ("set") knockback, none at 0.
    "x28": "set_kb",
    "x2C": "base_kb",
    "x34": "shield_damage",
    "x40_b0": "clanks",
    "x40_b1": "rebounds",
    "x40_b2": "hits_air",
    "x40_b3": "hits_ground",
    # Frames before a victim can be hit again (a victim's `rehit_timer`).
    "x40_b4": "rehit_rate",
    # An item's box that hits an item logs the hit as kind 8 rather than 0 (`it_80270E30`).
    "x41_b4": "item_hit_alt",
    # An item's box logs its hit on a fighter as kind 4 (`ftColl_80077C60`), and its absorb as
    # the second kind (`ftColl_80077464`).
    "x41_b5": "fighter_hit_alt",
    # An item's box meets a shield in mode 2 rather than 1 (`ftColl_80077688`).
    "x41_b6": "shield_hit_alt",
    "x41_b7": "reflectable",
    "x42_b0": "absorbable",
    # An item's box meets a fighter's shield bubble (`Fighter::state_flags`' `b0`).
    "x42_b1": "meets_bubble",
    # An item's box misses a fighter facing the way the item faces.
    "x42_b2": "misses_same_facing",
    # An item's box bounces off a shield.
    "x42_b3": "bounces_off_shield",
    # Reflectors and absorbers meet it; a bubble that faces only one way passes the rest.
    "x42_b4": "meets_reflector",
    "x42_b5": "hits_fighters",
    # Grabs only the hurtboxes that can be grabbed.
    "x42_b6": "grabbable_hurtboxes_only",
    "x42_b7": "hits_items",
    # The decomp's command field name (`create_hitbox_3.item_hit_interaction`).
    "x43_b0": "item_hit_interaction",
    # The offset isn't scaled with the fighter (`ftColl_8007AD18` divides it by the scale).
    "x43_b1": "unscaled_offset",
    # It hits nothing that clanks or reflects: a "phantom" box.
    "x43_b2": "phantom",
    "x43_b3": "unused_43_b3",
    "x43_b4": "unused_43_b4",
    "x43_b5": "unused_43_b5",
    "x43_b6": "unused_43_b6",
    "x43_b7": "unused_43_b7",
    "x44": "victims_1_len",
    "x45": "victims_2_len",
    "x46": "pad_46",
    # Where the box is, and where it was the frame before.
    "x4C": "pos",
    "x58": "prev_pos",
    # The thrower of a thrown fighter's box, or the grabbed-victim-only bit (`HitOwner`).
    "x134": "owner",
}

HIT_VICTIM = {
    # Frames until the victim can be hit again (from the box's `rehit_rate`).
    "x4": "rehit_timer",
}

HURT_CAPSULE = {f"x24_b{i}": f"unused_24_b{i}" for i in range(1, 8)}

# A camera box (`CmSubject`): two vectors its reset zeroes and nothing reads.
CM_SUBJECT = {"x54": "unread_54", "x60": "unread_60"}

CAMERA_DEBUG_MODE = {"_58": "unused_58"}

# An item's flag bytes the decomp types as other structs': `xDCD` (`CameraBoxFlags`, the item's
# `hit_flags`) and `xDCC` (`Item_xDCC_flag`, its `state_flags`).
CAMERA_BOX_FLAGS = {"x0": "bits"}
CAMERA_BOX_FLAGS_X0 = {
    # Its throw and drop sounds are silenced (`it_8026BD0C`).
    "b2": "silenced",
    # It doesn't slide on the ground (`it_8026BD6C`, held items).
    "b3": "no_slide",
    # It stops dead when it lands.
    "b4": "stops_on_land",
    # It may hit its own player, its player's team (`it_80275444` sets both), and its own
    # player's and ownerless items may hit it (`it_80275414`).
    "b5": "hits_own_player",
    "b6": "hits_own_team",
    "b7": "hit_by_own_items",
}
ITEM_XDCC_FLAG = {
    # A reflection leaves its hitboxes' damage as it is (`Item_80269F14`'s reflect).
    "b0": "keeps_reflected_damage",
    # A reflection doesn't take the item over: one a reflector's `b4` asked for, and a Poké
    # Ball's from its `b5`.
    "b1": "keeps_owner_reflected",
    "b2": "keeps_pokeball_reflected",
    # It checks the blast zones.
    "b3": "checks_blast_zones",
}

# A fighter's damage state (`dmg`, `Fighter::dmg`, the fighter's `x182C` to `x1960`).
DMG = {
    # The damage it takes is scaled by this (Yoshi's egg sets it).
    "x182c_behavior": "damage_scale",
    "x1830_percent": "percent",
    # What its armor (Kirby's stone, a grab's) has left before it breaks.
    "x1834": "armor_hp",
    # The damage taken this frame, applied at the collision resolve.
    "x1838_percentTemp": "pending_damage",
    "x183C_applied": "damage_applied",
    # The largest whole damage of the frame's tip hits.
    "x1840": "max_tip_damage",
    "x1848_kb_angle": "kb_angle",
    "x184c_damaged_hurtbox": "damaged_hurtbox",
    "x1854_collpos": "coll_pos",
    "x1860_element": "element",
    # The weighed hit's severity, damage, direction, angle and height: only written.
    "x1864": "unread_1864",
    "x1868_source": "source",
    "x186c": "unread_186c",
    "x1870": "unread_1870",
    "x1874": "unread_1874",
    "x1878": "unread_1878",
    # A tip's hit (`ftColl_8007AB80`), held back while its hitlag lasts (`ftColl_8007BE3C`):
    # its knockback, where it hit, its element and severity, whose it was and its damage.
    "x187c": "tip_kb",
    "x1880": "tip_pos",
    "x188c": "tip_element",
    "x1890": "tip_severity",
    "x1894": "tip_source",
    "x1898": "tip_damage",
    # Frames of a hit still to apply: any keeps the frame's hit handling running.
    "x189C_unk_num_frames": "hit_frames",
    # The tips' knockback the frame applied, 0 without tips.
    "x18a0": "tip_kb_applied",
    "x18A4_knockbackMagnitude": "kb_magnitude",
    # The knockback of the hit that sent it, which SDI's threshold reads.
    "x18A8": "hit_kb",
    "x18ac_time_since_hit": "time_since_hit",
    # The model's shake (`ftCommon_8008021C`).
    "x18B8": "shake_x",
    "x18BC": "shake_y",
    # Whoever hit it last: their spawn number, their player (6: none), the attack, its source's
    # class (3: an item) and kind.
    "x18C0": "source_spawn",
    "x18c4_source_ply": "source_player",
    "x18C8": "source_attack",
    "x18CC": "source_class",
    "x18D0": "source_kind",
    # The last attack's record and the hit records (`ft_800892D4` clears them).
    "x18d4": "attack_record",
    "x18d8": "hit_records",
    "x18ec_instancehitby": "instance_hit_by",
    # Health a food or a heart still has to give (stamina mode).
    "x18F0": "healing",
    # A cape's turn's frames.
    "x18F4": "cape_turn_frames",
    # The hit shake's kind (0 in the air, 1 on the ground, 2 electric), its frames, step, length
    # and the floor's normal it shakes along.
    "x18F8": "shake_kind",
    "x18f9": "unused_18f9",
    "x18fa_model_shift_frames": "model_shift_frames",
    "x18FC": "shake_step",
    "x18FD": "shake_len",
    "x1900": "shake_normal_x",
    "x1904": "shake_normal_y",
    # The sound a hard hit queues for when hitlag ends, and the character's table of them.
    "x1908": "queued_sfx",
    "x190C": "queued_sfx_table",
    # Frames counted toward a stage's damage over time (`Fighter_procAnim`).
    "x1910": "dot_frames",
    # The largest whole damage it dealt an item this frame.
    "x1914": "item_hit_damage",
    # A clank's recoil.
    "x191C": "clank_damage",
    # The largest damage its shield took this frame, the push and its direction.
    "x1924": "max_shield_damage",
    "x1928": "shield_push",
    "x192c": "shield_push_dir",
    # The bounds it picks items up and steps on them in (`ft_800819A8`).
    "x1930": "item_pickup_bounds",
    # A bounce off an item: its frames and those left.
    "x1948": "bounce_frames",
    "x194C": "bounce_frames_left",
    "x1950": "stepped_on_item",
    # Frames hitlag is held past its end (the longest of a cape's turns).
    "x1954": "hold_frames",
    "x1958": "pending_hitlag_frames",
    "x195c_hitlag_frames": "hitlag_frames",
}

# A match's rules (`StartMeleeRules`, `vs.start`), which the replays' game info block holds.
START_MELEE_RULES = {
    # How the damage displays show (`ifStatus_802F665C`).
    "x0_3": "damage_display",
    # The frame counter starts at `start_frame` as it is (else one less).
    "x1_0": "exact_start_frame",
    # The coins count toward the clear's bonus.
    "x1_1": "counts_coins",
    # The countdown's announcement is the long one (8 rather than 3).
    "x1_2": "long_countdown",
    # The "Go!" is announced as kind 8 rather than 4.
    "x1_3": "alt_go",
    # The stage's layout is the rules' (`stage_layout`): no alternate from held buttons.
    "x1_4": "fixed_layout",
    # The match's end leaves the controllers' rumble as it is.
    "x1_5": "keeps_rumble",
    # Kept by the HUD's offscreen arrows (`arrows.x30`).
    "x2_1": "arrows_flag",
    # Written by the modes that set up rules, read by none.
    "x2_2": "unread_2_2",
    # The damage displays show KOs and teammates' KOs.
    "x2_5": "damage_display_kos",
    "x2_6": "damage_displays_on",
    "x2_7": "damage_display_fades",
    "x3_0": "shows_score",
    # The pause screen's controls it lists (the stick, Z, L+R+A+Start), whether it lists none,
    # and whether the timers show.
    "x3_1": "pause_shows_stick",
    "x3_2": "pause_z_retries",
    "x3_3": "pause_shows_z",
    "x3_4": "pause_lras_quits",
    "x3_5": "pause_shows_lras",
    "x3_6": "pause_hides_controls",
    "x3_7": "pause_shows_timers",
    # The pause camera (and the rules' `on_pause_override`).
    "x4_0": "pause_camera",
    # A single-player run's or Target Test's rules (`gm_8016B0E8`).
    "x4_2": "single_player",
    # A game over goes to its own scene.
    "x4_3": "game_over_scene",
    # The score is the clear's (`fn_8017F294`), else `score`.
    "x4_4": "clear_scores",
    "x4_5": "allows_continue",
    "x4_6": "time_bonus",
    "x4_7": "damage_bonus",
    # The match has no outcome of its own (it ends from outside).
    "x5_0": "endless",
    # The match goes on past the opponents' KOs (a horde's).
    "x5_1": "goes_on",
    "x5_2": "no_fighter_hits",
    "x5_3": "stadium_zoom",
    "x5_4": "unused_5_4",
    "x5_5": "unused_5_5",
    "x5_6": "unused_5_6",
    "x5_7": "unused_5_7",
    # A Sudden Death: no items but the Bob-ombs' rain (`gm_8016B238`).
    "x6": "sudden_death",
    # The match's ending kind, if the rules set one.
    "x7": "end_kind",
    # The run's kind: 1 a Target Test stage.
    "x9": "run_kind",
    # The stage's layout (0: chosen by held buttons, `fn_8016E5C0`).
    "xA": "stage_layout",
    # Frames the match waits after its end.
    "xD": "end_wait",
    "x14": "start_frame",
    "x18": "score",
    "x1C_pad": "pad_1c",
    # The items the match may spawn, a bit each by kind.
    "x20": "item_switch",
    # The Pokémon a Poké Ball lets out, 0 for a random one.
    "x28": "forced_pokemon",
    "x2C": "quake_scale",
    "x30": "damage_ratio",
    # A single-player run's opponents' list, and a challenger's record.
    "x54": "opponent_list",
    "x58": "challenger_record",
    "pad_x5C": "pad_5c",
}

# A player's line of a match's results (`MatchPlayerData`, `player_standings`), as
# `fn_80165418`'s standings fill it from the player's stats; the stats' names are the records'
# (`gmMainLib`'s `add_stats`) where they add to one.
MATCH_PLAYER_DATA = {
    # The costume (the top six bits), the rumble and the stamina flags (bits 1 and 0).
    "x3_u": "flags",
    # The name tag (0x78: none).
    "x4": "nametag",
    "x9": "remaining_hp",
    "xE": "coins",
    # KOs of the players past the four `kills` count.
    "x18": "kills_rest",
    "x1C": "coins_held",
    "x20": "kos",
    "x24": "falls",
    "x28": "match_frames",
    "x30": "sub_rank",
    # Hits over attacks, in percent.
    "x34": "hit_percent",
    "x35_pad": "pad_35",
    "x38": "attacks_hit",
    "x3C": "attacks_total",
    "x40": "damage_dealt",
    "x44": "damage_taken",
    "x48": "damage_recovered",
    "x4C": "peak_damage",
    "x50": "walk_distance",
    "x54": "run_distance",
    "x58": "fall_distance",
    "x5C": "peak_height",
    "x60": "coins_collected",
    "x64": "coins_swiped",
    "x68": "coins_lost",
    # Seconds from the player's stats' timers (`pl_80040CB4`, `pl_80040CD8`).
    "x6C": "air_time",
    "x70": "ground_time",
    "x74": "ground_attacks",
    "x78": "air_attacks",
    "x7C": "items_thrown",
    "x80": "specials",
    "x84": "grabs",
    "x88": "down_attacks",
    # The action stats' word past the high attacks (`pl_800386D8(stats, 112)`).
    "x8C": "action_stat_112",
    # Items picked up, every kind's.
    "x90": "items_picked_up",
    # Two more of the stats' timers, in tenths of frames (`pl_80040D44`, `pl_80040D68`), and two
    # in seconds (`pl_80040CFC`, `pl_80040D20`).
    "x94": "timed_stat_1",
    "x98": "timed_stat_2",
    "x9C": "timed_stat_3",
    "xA0": "timed_stat_4",
    # The stale-move table's `last_attack_stat` (`pl_80040DB8`).
    "xA4": "last_attack_stat",
}

# A player's static block (`_StaticPlayer`, `game.players`), with its flag bytes: the names
# of the getters and setters wave 5 ported (`player_entry_delay`, `player_coins_knocked`).
STATIC_PLAYER = {
    "unk0E": "unused_0e",
    "unk45": "effect_color",
    "unk4C": "entry_delay",
    "unk4D": "kirby_copy_kind",
    "unk4E": "spawn_point",
    "unk4F": "unused_4f",
    # Master Hand's defense ratio (`Player_GetUnk50`, which no setter sets).
    "unk50": "unset_ratio",
    "unk8F": "unused_8f",
    "unk98": "coins_knocked",
    "unk9C": "coins_lost",
    "unkAF": "unused_af",
    # The player's stats past the stale moves (`pl::ext_u32` and the rest).
    "xDB0": "stats_ext",
}
STATIC_PLAYER_FLAGS = {
    "b0": "rumbles",
    # A joiner of a setup that has several (drawn as a wireframe), `Player_SetFlagsBit1`.
    "b1": "joiner",
    # The player has a second fighter (the Ice Climbers' Nana).
    "b2": "two_fighters",
    # Its fighter comes in through the entry state.
    "b3": "enters",
    # The entry waits `ftCommonData` x6C8 frames more.
    "b4": "entry_waits",
    # The fighter's `takes_damage_cloaked` and `no_color_anims`.
    "b6": "damage_cloaked",
    "b7": "no_color_anims",
}
STATIC_PLAYER_MORE_FLAGS = {
    # The fighter's `x2034` setup runs its rule set (`ftCo_800C8FC4`).
    "b0": "init_rule_set",
    # The fighter's `rules_voice_muted`, `is_stamina` and `sfx_size`.
    "b1": "voice_muted",
    "b2": "stamina",
    # Offscreen damage applies to it (`Fighter_procAnim`).
    "b3": "offscreen_damage",
    # No magnifier bubble shows it.
    "b4": "no_magnifier",
    # It's KO'd upward (`enter_dead_up`).
    "b5": "dies_up",
    "b6": "sfx_size",
}
STATIC_PLAYER_FLAGS_AE = {
    # The fighter's `attrs_scaled`.
    "b0": "attrs_scaled",
    # Read by Kirby's setup (`ftKb_Init`), set by nothing a match reaches.
    "b1": "kirby_init_flag",
    "b2": "unused_b2",
    "b3": "unused_b3",
    "b4": "unused_b4",
    "b5": "unused_b5",
    "b6": "unused_b6",
    "b7": "unused_b7",
}

# The structs by step 3's commits, in order.
GROUPS = [
    {"HitCapsule": HIT_CAPSULE, "HitVictim": HIT_VICTIM, "HurtCapsule": HURT_CAPSULE},
    {
        "CmSubject": CM_SUBJECT,
        "CameraDebugMode": CAMERA_DEBUG_MODE,
        "CameraBoxFlags": CAMERA_BOX_FLAGS,
        "CameraBoxFlags_x0": CAMERA_BOX_FLAGS_X0,
        "Item_xDCC_flag": ITEM_XDCC_FLAG,
    },
    {"dmg": DMG},
    {"StartMeleeRules": START_MELEE_RULES, "MatchPlayerData": MATCH_PLAYER_DATA},
    {
        "_StaticPlayer": STATIC_PLAYER,
        "_StaticPlayer_flags": STATIC_PLAYER_FLAGS,
        "_StaticPlayer_more_flags": STATIC_PLAYER_MORE_FLAGS,
        "_StaticPlayer_flagsAE": STATIC_PLAYER_FLAGS_AE,
    },
]

STEP_3 = {}
for _group in GROUPS:
    STEP_3.update(_group)

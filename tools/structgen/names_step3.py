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
    # The action stats' ledge catches (id 112, `pl_800386D8(stats, 112)`: `more_actions[4]`).
    "x8C": "ledge_catches",
    # Items picked up, every kind's.
    "x90": "items_picked_up",
    # Two more of the stats' timers, in tenths of frames (`pl_80040D44`, `pl_80040D68`), and two
    # in seconds (`pl_80040CFC`, `pl_80040D20`).
    "x94": "timed_stat_1",
    "x98": "timed_stat_2",
    "x9C": "timed_stat_3",
    "xA0": "timed_stat_4",
    # The stale-move table's `teammate_hits` (`pl_80040DB8`).
    "xA4": "teammate_hits",
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

# The fighter's data structs (the continuation's second group): a character's save records
# (`FighterData`, its 1P records `FighterData_x7C`), a smash attack's charge (`_SmashAttr`), the
# common attributes (`ftCo_DatAttrs`, Yoshi's egg `ftCo_DatAttrs_xBC_t`), a part's bone
# (`FighterBone`), the step-on boxes (`lb_80014638_arg0_t`, `arg1_t`), the attack record
# (`Struct2074`) and a hit's record (`ft_800898B4_t`).
FIGHTER_DATA_X7C = {
    # Target Test: cleared, then the best time; before it, the most targets broken.
    "b0": "target_cleared",
    "b1": "ten_man_cleared",
    "b2": "hundred_man_cleared",
    "b3": "fifteen_min_cleared",
    "b4": "classic_cleared",
    "b5": "adventure_cleared",
    "b6": "all_star_cleared",
    # Training's best combo.
    "x7E": "training_combo",
    # The highest level each 1P mode was cleared on.
    "x80": "classic_level",
    "x81": "adventure_level",
    "x82": "all_star_level",
    "x83": "unused_83",
    "x84": "home_run_distance",
    "x88": "classic_score",
    "x8C": "adventure_score",
    "x90": "all_star_score",
    "x94": "target_record",
    "x98": "ten_man_time",
    "x9C": "hundred_man_time",
    "xA0": "three_min_kos",
    "xA2": "fifteen_min_kos",
    "xA4": "endless_kos",
    "xA8": "cruel_kos",
}
FIGHTER_DATA = {
    # The times played, a big-endian half in two bytes.
    "x78": "play_count_hi",
    "x79": "play_count_lo",
    # `x0.b0`: the character was played.
    "x7A": "played_flags",
    "x7B": "unused_7b",
    "x7C": "records",
}
FIGHTER_BONE = {
    # The joint an animation is read into before it's blended onto `joint`.
    "x4_jobj2": "interp_joint",
    "x8": "flags",
    # Byte 0 the bone's depth in the tree, byte 1 flags.
    "xC_u": "tree",
}
# An item's top that can be stepped on: its motion this frame, and its span.
LB_STEP_TOP = {"unk_x": "motion_x", "unk_y": "motion_y", "x8": "span"}
# A hit's record (`dmg.hit_records`), copied from the attack record and filled in at the hit.
FT_800898B4 = {
    "x0": "attacker_percent",
    "x4": "victim_percent",
    # The player who last hit the attacker with an item (6 none): a hit on oneself is a bonus.
    "xC": "item_thrower",
    # A hitbox's angle in the common data's spike range (`pl_80037ECC`).
    "x10_b0": "spike",
    "x10_b1": "metal",
    "x10_b2": "bunny_hood",
    "x10_b3": "starman",
    "x10_b4": "cloak",
    "x10_b5": "giant",
    "x10_b6": "tiny",
    "x10_b7": "landed",
    "x11_b0": "from_behind",
    # The victim at the magnifier's edge.
    "x11_b1": "at_edge",
    # The victim hanging on a ledge (`CliffWait`).
    "x11_b2": "on_ledge",
    # A spike on a victim still in a strong hit's knockback from another player.
    "x11_b3": "spike_after_strong",
    # A forward tilt or smash from an attacker not holding the victim (`ft_80089914`).
    "x11_b4": "forward_attack",
}

# The player's stats (the continuation's third group): the stale-move table's records past the
# stale moves (`StaleMoveTable`), the action stats (`plActionStats`, `plAttackStats`), the
# combo counters (`pl_x5EC_t`), the KO ring (`pl_x8C0_t`) and the Bob-omb ring (`pl_x8D8_t`).
STALE_MOVE_TABLE = {
    "x5EC": "combos",
    # By the item log's kind: items taken in hand that had been thrown, and of those, ones no
    # player had touched (`pl_8003E17C`).
    "x674": "items_picked_up",
    "x710": "fresh_items_picked_up",
    # Each kind's run of containers giving it (`pl_8003E4A4`).
    "x7AC": "container_runs",
    # Each Pokémon's count out of the player's Poké Balls.
    "x848": "pokemon_released",
    "x8C0": "ko_ring",
    "x8D8": "bombhei_ring",
    # The bonuses, by kind: a flag or a count.
    "x904": "bonuses",
    "xC60": "damage_taken",
    "xC64": "peak_damage",
    "xC68": "damage_recovered",
    "xC6C": "damage_dealt",
    # The damage dealt by pummels, and by the Super Scope's shots.
    "xC70": "pummel_damage",
    "xC74": "scope_damage",
    # The damage dealt to each player.
    "xC78": "damage_to",
    # The combo order bonus's step (`pl_8003FC88`).
    "xC90": "combo_step",
    # Falls as a star KO.
    "xC94": "star_falls",
    # The damage taken from the stage's devices.
    "xC98": "stage_damage",
    # The match's running average of the percent.
    "xC9C": "average_percent",
    # The victims' percents at the hits, and their count (`pl_8003ED0C`).
    "xCA0": "victim_percent_sum",
    "xCA4": "victim_percent_count",
    # The Ray Gun's hits in a row.
    "xCA8": "ray_gun_hits",
    # The player whose Lip's Stick flowered this one (6 none).
    "xCAC": "flowered_by",
    # Hits on teammates under friendly fire (`pl_80040D8C`).
    "xCB0": "teammate_hits",
    # KOs off the top.
    "xCB4": "up_kos",
    # The last KO's: who scored it, the attack's flags, its hit's record and attack instance.
    "xCB8": "last_ko_by",
    "xCBC": "last_ko_attack",
    "xCC0": "last_ko_hit",
    "xCD4": "last_ko_instance",
    "xCD8": "powershields",
    # The SDI's distance in x and y (`pl_800401F0`).
    "xCDC": "sdi_distance_x",
    "xCE0": "sdi_distance_y",
    # Hits taken with knockback past `pl_804D6470` x98.
    "xCE4": "strong_hits_taken",
    "xCE8": "ledge_catches",
    # The frames spent mashing out of a grab or a flower, and the mashes.
    "xCEC": "mash_frames",
    "xCF0": "mashes",
}
PL_ACTION_STATS = {
    # A third `plAttackStats` the mirror keeps as bytes: the attacks that landed (an attack
    # record's first hit), by id.
    "x358": "landed",
    # By attack id, the players it hit (a bit each).
    "x504": "victims",
    "x568": "front_hits",
    "x56C": "back_hits",
    "x570": "other_hits",
    # Down tilts' and down smashes' hits.
    "x574": "down_hits",
    # The actions counted past the attacks' ids (`pl_800386D8`'s ids from 100): turns, squats,
    # walks, dashes, runs, jumps, air jumps, wall bounces.
    "x578": "turns",
    "x57C": "squats",
    "x580": "walks",
    "x584": "dashes",
    "x588": "runs",
    "x58C": "jumps",
    "x590": "air_jumps",
    "x594": "wall_bounces",
    # Ids 108 to 115: the downs (two), techs, platform drops, ledge catches, taunts, Fox's
    # taunt, powershields.
    "x598": "more_actions",
    "x5B8": "unused_5b8",
    # The player turned, walked, jumped, jumped in the air (the attack flags' bits).
    "x5BC_b0": "has_turned",
    "x5BC_b1": "has_walked",
    "x5BC_b2": "has_jumped",
    "x5BC_b3": "has_air_jumped",
}
PL_ATTACK_STATS = {
    # The attacks but grabs, pummels and throws (the attack flags' `count_x1A0`).
    "x1A0_count": "strikes_count",
    # The reflectors' (the attack flags' `count_x1A4`: the cape, the shine, the bat, ...).
    "x1A4_count": "reflectors_count",
    # Hitboxes angled in the common data's spike range (`pl_80037ECC`).
    "x1A8": "spikes_count",
}
PL_COMBOS = {
    "x0": "last_victim",
    "x4": "last_damage",
    # The hits in combos, and the combos (`pl_800414C0`'s average).
    "x8": "combo_hits",
    "xC": "combo_count",
    # By attacker, its combo on this player.
    "x10": "by_attacker",
}
PL_COMBO = {
    "x0": "damage",
    "x4": "hits",
    "x6": "unused_6",
    # The same attack's hits in a row, and that attack.
    "x8": "repeats",
    "xC": "attack",
    # Hits while this player's in the air (cleared when it lands).
    "x10": "air_hits",
    # A jab hit it while it shielded.
    "x12_b0": "shield_jabbed",
}
# A ring of the player's last five attack instances that KOed (`fn_800415B0`).
PL_KO_RING = {"x0": "next", "x4": "entries"}
PL_KO_RING_ENTRY = {"x0": "instance", "x2": "kos", "x3_b0": "self_ko"}
# A ring of the player's last five Bob-ombs thrown (`pl_800419AC`).
PL_BOMBHEI_RING = {"x0": "next", "x4": "entries"}
PL_BOMBHEI_RING_ENTRY = {"x0": "spawn", "x4_b0": "hit", "x4_b1": "used"}

# The save and the event mode (the continuation's fourth group): the save's records
# (`GmSaveData`), its unlocks' counters (`gmm_retval_ED98`, `EDB0`, `EDBC`, `gmm_x1868_1A8_t`),
# the event mode's run (`EventData`) and Target Test's choice (`gmm_x0_584_t`). Their
# address-named types are named too (`config.RENAME_TYPES`).
GM_SAVE_DATA = {
    "x186A": "unlocked_stages",
    "x186C": "unlocked_features",
    "unk_8": "challenger_records",
    "unk_28": "stage_unlock_records",
    "unk_30": "trophy_records",
    "unk_1A8": "message_records",
    "x1A30": "vs_contestants",
    "x1A34": "vs_play_time",
    "x1A38": "combined_vs_play_time",
    "x1A3C": "one_player_time",
    "x1A40": "unused_1a40",
    # The tournament's sudden deaths' seconds.
    "x1A44": "tournament_time",
    "x1A48": "coins",
    "x1A4C": "most_coins",
    "x1A50": "power_count",
    "x1A54": "power_time",
    "x1A58": "total_damage",
    "x1A5C": "total_kos",
    "x1A60": "total_self_destructs",
    "x1A64": "unused_1a64",
    # A bit per event cleared, and each event's best (the C splits the array).
    "x1A68": "events_cleared",
    "x1A70": "event_scores",
    "padding_x1A70": "event_scores_rest",
    # Event 51 ended with all three stocks left.
    "x1B3C": "final_event_all_stocks",
    # The messages pending and noted (a bit each), the trophies (a bit each), and each
    # message's time.
    "x1B40": "messages_pending",
    "x1B4C": "messages_noted",
    "x1B58": "trophies",
    "x1B80": "message_times",
    # The bonuses seen (a bit each; the C splits the array).
    "x1C88": "bonuses_seen",
    "padding_x1C88": "bonuses_seen_rest",
    "x1CB0": "prefs",
    "x1F2C": "fighters",
}
# The challengers' conditions.
CHALLENGER_RECORDS = {
    "x0": "vs_matches",
    "x4": "combined_vs_play_time",
    # Bits by character: played; Classic, Adventure, All-Star and Target Test cleared.
    "xC": "played",
    "x10": "classic_cleared",
    "x14": "adventure_cleared",
    "x18": "all_star_cleared",
    "x1C": "target_cleared",
}
# The stages' unlock conditions.
STAGE_UNLOCK_RECORDS = {"x0": "vs_matches", "x4": "target_cleared"}
# The trophies' unlock conditions.
TROPHY_RECORDS = {
    "x0": "vs_matches",
    "x4": "coin_matches",
    "x8": "target_cleared",
    "xC": "kos",
    # The VS distance walked (`fn_80161C90`).
    "x10": "walk_distance",
    "x14": "most_coins",
    "x18": "plays",
    # Each character's best Target Test time (the C splits the array).
    "x4C": "target_best",
    "padding_x4C": "target_best_rest",
    "xB0": "home_run_best",
    "x114": "training_combo_best",
}
# The messages' conditions: the KOs, and three messages shown (every character, the first six
# stages, everything).
MESSAGE_RECORDS = {"x0": "kos", "x4": "all_characters_shown", "x5": "stages_shown", "x6": "everything_shown"}
EVENT_DATA = {
    # The first player's character and costume, as played.
    "x0": "char_kind",
    "x1": "char_color",
    # The select's choice.
    "x2": "chosen_char",
    "x3": "chosen_color",
    "unk_535": "event",
    "x6": "port",
    "x7": "team",
    # The random opponents kept for the event's retries (players 2 to 4).
    "x8": "random_char_1",
    "x9": "random_char_2",
    "xA": "random_char_3",
    "xB_0": "timer_counts_up",
    "xB_1": "cleared",
    "xB_2": "round_won",
    "xB_3": "crowd",
    "xB_4": "series",
    "xB_5": "voices_loaded",
    "xB_6": "timed",
    "xB_7": "unused_b7",
    "xC": "score",
    # The event's process's counter (the egg dropped, a round's wait, the joiner joined) and
    # state (the egg broke, Event 36's round).
    "x10": "proc_counter",
    "x14": "unread_14",
    "x18": "proc_state",
    "x1C": "quake_scale",
    # A series: the round, the first player's stocks and damage carried over, the time left,
    # the series' frames, the character kept (Sheik, 0x21 none).
    "x20": "round",
    "x24": "round_stocks",
    "x28": "round_damage",
    "x2C": "round_seconds",
    "x30": "round_frames",
    "x34": "series_frames",
    "x38": "kept_char",
    "x3C": "coins",
    "x40": "play_frames",
    # How the files load (`gm_801BBB64`: 0 the first player's character set, 1 to choose, 2 to
    # 4 a series' rounds), whether a Kirby crowd copies, the stage, the players' characters and
    # costumes.
    "x44": "load_kind",
    "x45": "crowd_kirby_copies",
    "x48": "stage",
    "x4C": "chars",
    "x50": "colors",
    "unk_584": "target_test",
}
TARGET_TEST_CHOICE = {"unk_584": "ckind", "unk_585": "color", "unk_586": "nametag", "unk_587": "unused_587"}

# The modes (the continuation's fifth group): the VS scene's state and its fighters
# (`VsSceneState`, `VsSceneFighter`), a slot's start data (`PlayerInitData`), the match's end
# (`MatchEnd`, `UnkResultPlayerData`), the game's data (`gmm_x0` and its parts), the saved
# rules (`GameRules`), the VS modes' data (`VsModeData`), a name tag (`NameTagData`) and the
# scene loop (`gm_80479D58_t`, `gm_801677C0_s`). Their address-named types are named too.
VS_SCENE_STATE = {
    # 0 playing, 1 the frames after GAME!, 2 the clear screen, 3 leaving (the VS scene's
    # frame function, `gm_Scene_Vs_OnFrame`, switches on it).
    "unk_0": "phase",
    # Camera mode's shot: the HUD off, the pause's quit buttons ignored.
    "unk_3": "photo_mode",
    # A single-player match cleared (`clear_start`): its mode's own results count.
    "unk_9": "cleared",
    # The staggered entries' delay, 5 frames more for each slot that enters.
    "unk_A": "entry_delay",
    # One more than the GAME! announcement's kind (0 the outcome's own).
    "unk_B": "end_kind",
    # The opponents that joined (`fn_8016B728`).
    "unk_C": "joiners",
    # The slot of the last KO (6 none), for the stage bonuses.
    "unk_D": "last_ko",
    "unk_F": "unused_f",
    # The GAME! announcement's lines, one more than each sound (0 none), and the frame of the
    # second.
    "unk_10": "end_sfx_1",
    "unk_14": "end_sfx_2",
    "unk_18": "end_sfx_2_frame",
    # The countdown's sounds (`lbl_803D5630`) and the entries' voices (`lbl_803D5620`).
    "unk_1C": "countdown_sfx",
    "unk_20": "entry_voices",
    "unk_2C": "timer_frames",
    # The first player's score (`gm_8016B6E8`).
    "unk_2E": "score",
    # The frames after GAME!, counted to the rules' `end_wait`.
    "unk_30": "end_frames",
    # The weight on each item kind's count in the random items' table.
    "unk_34": "item_weight",
    "x24C": "match_end",
}
VS_SCENE_FIGHTER = {
    "x0": "ckind",
    # The entry's countdown (0x82 at entry): the announcer's line at 0x50.
    "x1": "entry_timer",
    # `b0` metal, `b1` (`PlayerInitData`'s `unread_c_b3`), `b2` invisible, `b3` shows the
    # team's hearts, `b4` Zelda and Sheik swapped on load, `b5` the stamina display explodes.
    "x4": "flags",
    "x5": "unused_5",
    "x6": "respawn_damage",
    # A respawn platform's place in use (counts down from 0x90), and the character on it.
    "x8": "platform_timer",
    "x9": "unread_9",
    # The fighter that respawns (0 the first), and `0x8000` to show every camera layer on
    # respawning; nothing sets either.
    "xB": "respawn_sub",
    "xC": "respawn_flags",
}
PLAYER_INIT_DATA = {
    # The player's `more_flags.sfx_size`.
    "xB": "sfx_size",
    # The fighter comes in through the entry state, staggered (`entry_delay`).
    "xC_b1": "enters",
    # Copied to the VS fighter's flag `b1`, which nothing reads.
    "xC_b3": "unread_c_b3",
    # The player's `attrs_scaled`, `voice_muted` and `stamina`.
    "xC_b5": "attrs_scaled",
    "xC_b6": "voice_muted",
    "xC_b7": "stamina",
    # The stamina display explodes at 0 HP (the hands).
    "xD_b0": "hp_explodes",
    # The fighter shows its team's hearts.
    "xD_b1": "shows_hearts",
    # The player takes no offscreen damage, has no magnifier bubble, is KO'd upward.
    "xD_b2": "no_offscreen_damage",
    "xD_b3": "no_magnifier",
    "xD_b4": "dies_up",
    "xD_b5": "unused_d_b5",
    "xD_b6": "unused_d_b6",
    "xD_b7": "unused_d_b7",
}
MATCH_END = {
    # The scene tick the standings were last worked out (`gm_8016C75C`).
    "x0": "standings_tick",
    "x7": "joiners",
    # A single-player mode's bonuses' bits (Classic's, Adventure's, All-Star's results).
    "_x448": "mode_bonuses",
    # Each player's decisions: earned and their points (`fn_8016C4F4`).
    "x44C": "decisions",
}
PLAYER_DECISIONS = {"x0": "earned", "x104": "points"}
GAME_DATA = {
    # The attract loop's and the how-to's rounds.
    "unk_1": "demo_count",
    # The attract loop's history (`gm_random_history`): how often each character (from here)
    # and each stage (from byte 0x1C) has played; the C splits the array.
    "unk_2": "attract_history",
    "pad_3": "attract_history_rest",
    # Matches lost to each challenger since power-on (`gmMainLib_8015DB6C`).
    "x39": "challenger_losses",
    "unk_44": "trophy_notices",
    "x1850": "rules",
}
GAME_RULES = {
    "unk_xc": "sd_penalty",
    # The diagrams' names mode and cursors, saved for the screens' next entry.
    "xD": "diagram_name_mode",
    "xE": "fighter_cursor_hi",
    "xF": "fighter_cursor_lo",
    "unk_x10": "name_cursor_hi",
    "x11": "name_cursor_lo",
    "x12": "diagram2_fighter",
    "x13": "diagram2_name",
    # The sound test's song, the gallery's music.
    "unk_14": "gallery_song",
}
ONE_PLAYER_MODES = {"unk_51C": "classic", "unk_522": "adventure", "unk_528": "all_star", "unk_530": "event"}
# The run's round to resume.
ONE_PLAYER_CHOICE = {"x5": "round"}
VS_MODE_DATA = {f"unk_0x{i}": f"unused_{i}" for i in range(3, 8)}
NAME_TAG_DATA = {
    # The byte a four-letter name ends at; the handicap; a name read past its end reaches the
    # padding.
    "x1A0": "name_end",
    "x1A2": "handicap",
    "padding_x1A2": "pad_1a3",
}
SCENE_LOOP = {
    "unk_0": "ticks",
    "unk_4": "frames_drawn",
    # Ticks the game clock was due.
    "unk_8": "game_ticks",
    # 1 ends the scene after the frame, 2 at once.
    "unk_C": "ending",
    "unk_10": "pause",
}
DEBUG_PAUSE = {
    # The debug pause's bits, a frame step's, each as last applied.
    "x0": "pause_bits",
    "x1": "applied_pause_bits",
    "x2": "step_bits",
    "x3": "applied_step_bits",
    # The process links the pause stops, and those the scheduler skips this tick.
    "unk_20": "paused_links",
    "unk_28": "skip_links",
    "unk_34": "clock_mask",
    # The tick runs the game (no debug pause, or a frame step).
    "unk_38_0": "runs",
    "unk_38_1": "unused_38_1",
}

# The HUD (the continuation's sixth group): the announcements (`Element_803F9628`), the layout
# (`ifAll_804A0FD8_t`), the stock icons and the single-player tallies (`ifStock_*`), the
# damage displays' flags, the magnifier's colors, the coin counter, the off-screen arrows and
# the scores (`un_*`). Their address-named types are named too.
ANNOUNCEMENT = {
    "x0": "gobj",
    "x4": "unread_4",
    "x8": "proc",
    # The announcement's own first line (the table's), played at `sfx_frame` unless a caller
    # gave one; the caller's second line at `sfx_2_frame`.
    "xC": "default_sfx",
    "x10": "sfx_frame",
    "x11": "sfx_2_frame",
    "x12": "flags",
    "x13": "unused_13",
    "x14": "model",
    "x18": "on_start",
    "x1C": "on_end",
    "x20": "sfx_1",
    "x24": "sfx_2",
}
ANNOUNCEMENT_FLAGS = {
    "x0": "sfx_1_played",
    "x1": "sfx_2_played",
    # The start callback ran.
    "x2": "started",
    **{f"x{i}": f"unused_{i}" for i in range(3, 8)},
}
# Anim 1 (the damage going the good way: down, or up in stamina) plays.
IF_DAMAGE_FLAGS = {"unk10": "heal_anim", "unk1": "unused_b7"}
# Frames since the display blew apart (to 0xC8, when it's freed).
IF_DAMAGE_STATE = {"unk9": "fade_frames"}
# The players the steal slots' stocks went to (slots 5 and 6).
IF_STOCK_USER_DATA = {"x3": "steal_takers"}
HUD_LAYOUT = {
    "x8": "root",
    "xC": "timer_pos",
    # Each player's display's place, and the places for three and for two players.
    "x18": "player_pos",
    "x60": "player_pos_3",
    "x84": "player_pos_2",
}
# The stage's bubble colors (`GroundParam`'s), for the magnifier.
IF_MAGNIFY = {
    "x4": "bubble_color_0",
    "x8": "bubble_color_2",
    "xC": "bubble_color_6",
    "x10": "bubble_color_8",
    "pad": "pad_ec",
}
HUD_STOCKS = {"x0": "models", "x4": "counter_model"}
# A player's stock icons' model and its joints by index: 0 to 7 the icons; past them, by
# mode, the coin counter's digits (9 to 13, 13 also its 0), the stock count's group (14), its
# ones (15) and tens or only digit (16).
PLAYER_STOCKS = {
    "x0": "gobj",
    "x4": "joints",
    **{f"x{0x24 + 4 * i:X}": f"joint_{8 + i}" for i in range(9)},
}
# The joiners' tally: 0 to make, 1 made; the joiners' costumes and characters (-1 beaten, -2
# the end), the crowd's count shown, the icons.
JOINER_TALLY = {"x0": "state", "x1": "costumes", "x83": "chars", "x108": "crowd_count", "x10C": "icons"}
# The characters-left tally (All-Star's rest area).
CHARS_LEFT_TALLY = {"x0": "state", "x1": "chars", "x83": "costumes", "x108": "unread_108", "x10C": "icons"}
COIN_COUNTER = {
    "x0_b0": "unread_b0",
    "x0_b1": "counting",
    # A coin every `step` frames.
    "x0_b234": "step",
    "x0_b567": "step_timer",
    "x1": "count",
    "x2": "target",
    "x3": "unused_3",
    "x4": "pos_x",
    "x8": "pos_y",
    "xC": "gobj",
}
OFFSCREEN_ARROWS = {
    "x0": "models",
    "x4": "gobjs",
    "x14": "colors",
    # By slot: the player's arrow is out.
    "x24": "shown",
    # By arrow: its side (0 none, 1 left, 2 right) and its player.
    "x28": "sides",
    "x2C": "players",
    # The rules' `arrows_flag`: no sound.
    "x30": "silent",
    # The arrow whose turn it is to sound, and the sound's timer (to 51).
    "x31": "sounding",
    "x32": "sound_timer",
}
SCORE_DISPLAYS = {"x0": "canvas", "x8": "entries"}
SCORE_DISPLAY = {
    "x0": "gobj",
    "x4": "text",
    "x8": "string",
    "xC": "score",
    # "-" shown in place of the score.
    "x10": "hidden",
}

# The rest (the continuation's seventh group): the items' counts and limits by hold kind
# (`HSD_ObjAllocUnk`), the screen flash (`BgFlashData`, `BgFlashUserData`), a swing chain's
# link (`lb_00F9_UnkDesc0`, `DynamicsData`), an item's spawn (`SpawnItem`), a fighter's
# shadow's holds (`LbShadow`) and the menus' key repeat (`ControllerMap`, `GmControllerMap`).
# Each hold kind's items out and the most allowed (`Item_8026784C`, the common data's).
ITEM_COUNTS = {
    "x0": "common",
    "x4": "common_limit",
    # The common items' shots and parts.
    "x8": "common_shots",
    "xC": "common_shots_limit",
    "x10": "food",
    "x14": "food_limit",
    "x18": "fighter_articles",
    "x1C": "pokemon",
    "x20": "pokemon_limit",
    "x24": "pokemon_shots",
    "x28": "pokemon_shots_limit",
    # Goombas, ReDeads, Octoroks, and the Octorok's stones.
    "x2C": "monsters",
    "x30": "monsters_limit",
    "x34": "monster_shots",
    "x38": "monster_shots_limit",
    # The stages' items (Targets to Klaptraps) and their shots (the Arwings', Birdo's eggs).
    "x3C": "stage_items",
    "x40": "stage_shots",
    "x44": "stage_shots_limit",
    "x48": "trophies",
    "x4C": "trophies_limit",
    "x50": "coins",
    "x54": "coins_limit",
    "x58": "stage_hurtboxes",
    "x5C": "stage_hurtboxes_limit",
    # Training mode's items out (`spawn_origin` 1).
    "x60": "training_items",
    "x64": "training_items_limit",
}
BG_FLASH_DATA = {
    # The fade's colors, the color shown, each channel's value and step.
    "x4": "from",
    "x8": "to",
    "xC": "color",
    "x10": "channels",
    "x20": "steps",
    # The wipe (modes 3 and 4): held (nothing sets it), its step across, its strips' height,
    # done, where it stands, its steps a frame.
    "x30": "wipe_held",
    "x31": "wipe_step",
    "x32": "wipe_strip_height",
    "x33": "wipe_done",
    "x34": "wipe_x",
    "x38": "wipe_y",
    "x3C": "wipe_speed",
    "x40": "gobj",
    "x44": "camera",
}
BG_FLASH_USER_DATA = {"x0": "alpha", "x4": "overlay"}
SWING_LINK = {
    "unk_2C": "joint_pos",
    # The spin left from the last frame: its axis, its angle and the angle it loses a frame.
    "unk_38": "spin_axis",
    "unk_44": "spin",
    "unk_48": "length",
    "unk_4C": "stiffness",
    # The turn back toward the animation's direction a frame, the most it may be from it, and
    # the most it swings in a frame.
    "unk_50": "return_rate",
    "unk_54": "ease_axis",
    "unk_58": "rest_rotation",
    "unk_68": "max_angle",
    "unk_6C": "unread_6c",
    "unk_78": "unread_78",
    "unk_84": "spin_damping",
    "unk_88": "max_swing",
    "unk_8C": "gravity",
}
DYNAMICS_DATA = {"unk_94": "unread_94"}
SPAWN_ITEM = {
    "x0_parent_gobj": "owner",
    # What made it (a fighter or an item), whose attack its hits count as.
    "x4_parent_gobj2": "maker",
    "x10": "origin",
    "x3C_damage": "damage",
    "x3E": "unused_3e",
    "x40": "ignore_id",
    # `b0`: placed against the stage at once.
    "x44_flag": "flags",
    "x45_flag": "unread_45",
    "x46_flag": "unused_46",
    "x47_flag": "unused_47",
    "x48_ground_or_air": "ground_or_air",
}
CONTROLLER_MAP = {
    "x0": "ports",
    "xF0": "repeat_fn",
    # The key repeat: the first delay; the delays after, by how long the buttons are held.
    "xF4": "repeat_delay",
    "xF6": "slow_repeat",
    "xF8": "mid_after",
    "xFA": "mid_repeat",
    "xFC": "fast_after",
    "xFE": "fast_repeat",
}
GM_CONTROLLER_MAP = {"x2C": "held_frames"}

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
    {},
    {"StartMeleeRules": START_MELEE_RULES, "MatchPlayerData": MATCH_PLAYER_DATA},
    {
        "_StaticPlayer": STATIC_PLAYER,
        "_StaticPlayer_flags": STATIC_PLAYER_FLAGS,
        "_StaticPlayer_more_flags": STATIC_PLAYER_MORE_FLAGS,
        "_StaticPlayer_flagsAE": STATIC_PLAYER_FLAGS_AE,
    },
    {},
    {
        "FighterData_x7C": FIGHTER_DATA_X7C,
        "FighterData": FIGHTER_DATA,
        "FighterBone": FIGHTER_BONE,
        "lb_80014638_arg1_t": LB_STEP_TOP,
        "ft_800898B4_t": FT_800898B4,
    },
    {
        "StaleMoveTable": STALE_MOVE_TABLE,
        "plActionStats": PL_ACTION_STATS,
        "plAttackStats": PL_ATTACK_STATS,
        "pl_x5EC_t": PL_COMBOS,
        "pl_x5EC_t_x10": PL_COMBO,
        "pl_x8C0_t": PL_KO_RING,
        "pl_x8C0_t_x4": PL_KO_RING_ENTRY,
        "pl_x8D8_t": PL_BOMBHEI_RING,
        "pl_x8D8_t_x4": PL_BOMBHEI_RING_ENTRY,
    },
    {
        "GmSaveData": GM_SAVE_DATA,
        "gmm_retval_ED98": CHALLENGER_RECORDS,
        "gmm_retval_EDB0": STAGE_UNLOCK_RECORDS,
        "gmm_retval_EDBC": TROPHY_RECORDS,
        "gmm_x1868_1A8_t": MESSAGE_RECORDS,
        "EventData": EVENT_DATA,
        "gmm_x0_584_t": TARGET_TEST_CHOICE,
    },
    {
        "VsSceneState": VS_SCENE_STATE,
        "VsSceneFighter": VS_SCENE_FIGHTER,
        "PlayerInitData": PLAYER_INIT_DATA,
        "MatchEnd": MATCH_END,
        "UnkResultPlayerData": PLAYER_DECISIONS,
        "gmm_x0": GAME_DATA,
        "GameRules": GAME_RULES,
        "gmm_x0_vsdata": ONE_PLAYER_MODES,
        "gmm_x0_528_t": ONE_PLAYER_CHOICE,
        "VsModeData": VS_MODE_DATA,
        "NameTagData": NAME_TAG_DATA,
        "gm_80479D58_t": SCENE_LOOP,
        "gm_801677C0_s": DEBUG_PAUSE,
    },
    {
        "Element_803F9628": ANNOUNCEMENT,
        "Element_803F9628_x12": ANNOUNCEMENT_FLAGS,
        "IfDamageFlags": IF_DAMAGE_FLAGS,
        "IfDamageState": IF_DAMAGE_STATE,
        "IfStockUserData": IF_STOCK_USER_DATA,
        "ifAll_804A0FD8_t": HUD_LAYOUT,
        "ifMagnify": IF_MAGNIFY,
        "ifStock_804A1378": HUD_STOCKS,
        "ifStock_804A1378_per_player": PLAYER_STOCKS,
        "ifStock_804A1774": JOINER_TALLY,
        "ifStock_804A1ACC": CHARS_LEFT_TALLY,
        "un_803F9E08_t": COIN_COUNTER,
        "un_804A1F10_t": OFFSCREEN_ARROWS,
        "un_804A1F58_t": SCORE_DISPLAYS,
        "un_804A1F58_x8_t": SCORE_DISPLAY,
    },
    {
        "HSD_ObjAllocUnk": ITEM_COUNTS,
        "BgFlashData": BG_FLASH_DATA,
        "BgFlashUserData": BG_FLASH_USER_DATA,
        "lb_00F9_UnkDesc0": SWING_LINK,
        "DynamicsData": DYNAMICS_DATA,
        "SpawnItem": SPAWN_ITEM,
        "controller_map": CONTROLLER_MAP,
        "gm_controller_map": GM_CONTROLLER_MAP,
    },
]

# The port's names for the decomp's address-named structs (C name to Rust name), which
# `config.RENAME_TYPES` takes.
TYPE_NAMES = {
    "FighterData_x7C": "FighterRecords",
    "gmm_retval_ED98": "ChallengerRecords",
    "gmm_retval_EDB0": "StageUnlockRecords",
    "gmm_retval_EDBC": "TrophyRecords",
    "gmm_x1868_1A8_t": "MessageRecords",
    "gmm_x0_584_t": "TargetTestChoice",
    "gmm_x0": "GameData",
    "gmm_x0_44_t": "TrophyNotices",
    "gmm_x0_vsdata": "OnePlayerModes",
    "gmm_x0_528_t": "OnePlayerChoice",
    "gmm_x0_vsmodes": "VsModes",
    "UnkResultPlayerData": "PlayerDecisions",
    "gm_80479D58_t": "SceneLoop",
    "gm_801677C0_s": "DebugPause",
    "Element_803F9628": "Announcement",
    "Element_803F9628_x12": "AnnouncementFlags",
    "ifAll_804A0FD8_t": "HudLayout",
    "ifStock_804A1378": "HudStocks",
    "ifStock_804A1378_per_player": "PlayerStocks",
    "ifStock_804A1774": "JoinerTally",
    "ifStock_804A1ACC": "CharsLeftTally",
    "un_803F9E08_t": "CoinCounter",
    "un_804A1F10_t": "OffscreenArrows",
    "un_804A1F58_t": "ScoreDisplays",
    "un_804A1F58_x8_t": "ScoreDisplay",
    "HSD_ObjAllocUnk": "ItemCounts",
    "lb_00F9_UnkDesc0": "SwingLink",
}

STEP_3 = {}
for _group in GROUPS:
    STEP_3.update(_group)

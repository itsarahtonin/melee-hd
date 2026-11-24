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
# The structs by step 3's commits, in order.
GROUPS = [
    {"HitCapsule": HIT_CAPSULE, "HitVictim": HIT_VICTIM, "HurtCapsule": HURT_CAPSULE},
    {
        "CmSubject": CM_SUBJECT,
        "CameraDebugMode": CAMERA_DEBUG_MODE,
    },
    {},
    {},
    {},
    {},
    {
        "FighterData_x7C": FIGHTER_DATA_X7C,
        "FighterData": FIGHTER_DATA,
        "FighterBone": FIGHTER_BONE,
        "ft_800898B4_t": FT_800898B4,
    },
    {},
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
        "gmm_x0": GAME_DATA,
        "GameRules": GAME_RULES,
        "gmm_x0_vsdata": ONE_PLAYER_MODES,
        "gmm_x0_528_t": ONE_PLAYER_CHOICE,
        "VsModeData": VS_MODE_DATA,
        "NameTagData": NAME_TAG_DATA,
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

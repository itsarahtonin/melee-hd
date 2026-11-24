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
        "FighterBone": FIGHTER_BONE,
        "ft_800898B4_t": FT_800898B4,
    },
    {},
    {},
    {},
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

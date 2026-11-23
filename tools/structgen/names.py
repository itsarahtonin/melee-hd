"""The port's names for the generated structs' fields the decomp names by offset (or leaves
unknown), by struct: C name to Rust name. structgen keeps the C name as a list-form
`#[doc(alias("x2114"))]` on the field, and the comparer's labels give both
(`hitlag_frames (x2114)`); `rename_readers.py` renames every use from this map, on any branch.

Names come from the decompilation where it has them (`x8_spawnNum` is `spawn_num`), else from
the docs, the ports' uses and ssbm-rs. A field nothing reads or writes is `pad_*` (with its
offset, as padding is elsewhere) or `unused_*`; one written and never read is `unread_*`.

Wave 6's steps so far: `Fighter` and `CpuFighter` (task 18), then `Item`, the structs only it
holds and the kinds' variables (`Item_ItemVars`' members, task 22). The script applies every
step's renames at once, so a branch from before either step is brought up to both.
"""

# `Item`, the item's block.
ITEM = {
    # A freed item's link to the next free item (`HSD_ObjFree` writes it over the first word).
    "x0": "next_free",
    "x8": "unused_8",
    # The spawn's `x10`; 1 counts the item among the training mode's (`Item_802676F4`).
    "x18": "spawn_origin",
    "x1C": "spawn_num",
    "x20_team_id": "team",
    "x21": "unused_21",
    "x22": "unused_22",
    "x23": "unused_23",
    # The scale of the hitbox last made, `it_802790C0`.
    "x3C": "hitbox_scale",
    "x40_vel": "vel",
    # The tick's push (the wind's), added to the position and cleared.
    "x58_vec_unk": "wind_push",
    # The floor's motion under it, added to the position.
    "x64_vec_unk2": "floor_motion",
    "x70_nudge": "nudge",
    # The skid along the ground (`it_8027737C`), the slide down a slope (`it_80277040`) and the
    # slide before it.
    "x7C": "skid_vel",
    "x88": "slide_vel",
    "x94": "last_slide_vel",
    # Cleared with the knockback vectors (`it_802734B4`), never read.
    "xA0": "unread_a0",
    # The surface last touched (`it_80276CEC`): its normal and its line.
    "xAC_unk": "surface_normal",
    "xB8_itemLogicTable": "logic_table",
    "xBC_itemStateContainer": "state_table",
    "xC4_article_data": "article",
    "xC8_joint": "joint",
    "xCC_item_attr": "attrs",
    "xD0_itemStateDesc": "state_desc",
    "xD4_dynamicBones": "dynamic_bones",
    "x374_dynamicBonesNum": "dynamic_bone_count",
    "x378_itemColl": "coll",
    # The fighter holding it (`it_802742F4`).
    "x51C": "holder",
    "x520_cameraBox": "camera_box",
    "x524_cmd": "cmd",
    "x548_colorOverlay": "color_overlay",
    # Whether `tint` applies (Mr. Game & Watch's items, from his team, `it_8027CE64`).
    "x5C8": "tinted",
    # The model's alpha, 0xFF opaque.
    "x5C9": "alpha",
    "x5CA": "pad_5ca",
    "x5CB": "pad_5cb",
    "x5CC_currentAnimFrame": "anim_frame",
    "x5D0_animFrameSpeed": "anim_speed",
    "x5D4_hitboxes": "hitboxes",
    "xAC4_ignoreItemID": "ignore_id",
    "xAC8_hurtboxNum": "hurtbox_count",
    "xACC_itemHurtbox": "hurtboxes",
    # The box a fighter's feet are checked against to step on it (`it_80274D6C`).
    "xB54": "stomp_box",
    # The dynamics' points (`it_80272304`) and their count.
    "xB68": "dynamics_count",
    "xB69": "pad_b69",
    "xB6A": "pad_b6a",
    "xB6B": "pad_b6b",
    "xB6C_vars": "dynamics_points",
    "xBBC_dynamicBoneTable": "bone_table",
    "xBC0": "effect_queue",
    "xBC4": "tint",
    "xBC8": "outline",
    # The pickup range's offset and size (the attributes' `x30`, `x38`).
    "xBCC_unk": "grab_offset",
    "xBD4_grabRange": "grab_range",
    # The push box: its kind's at the item's scale (`it_80274E44`), and turned with the item
    # (`it_80271A58`).
    "xBDC": "push_box_base",
    "xBEC": "push_box",
    # The collision box a throw starts from (`it_80275BC8`), the kind's (`it_80275DFC`) and the
    # one in use (`it_80275D5C`).
    "xBFC": "throw_coll_box",
    "xC0C": "kind_coll_box",
    "xC1C": "coll_box",
    "xC2C": "surface_line",
    # The line it last touched, floor, wall or ceiling (-1 none).
    "xC30": "touched_line",
    "xC34_damageDealt": "damage_dealt",
    # The kind and position of the item it last hit (`itFlipper_DmgDealt`).
    "xC38": "hit_item_kind",
    # Its hitboxes' damage scales: the enemies' and stage items' by the CPU level
    # (`ItUnkHoldKind`), the state's, and the one a throw or drop gives the next state.
    "xC3C": "damage_scale",
    "xC40": "damage_mul",
    "xC44": "next_damage_mul",
    # The tick's largest damage dealt in a clank, to a reflector and to a shield.
    "xC48": "clank_damage",
    "xC4C": "reflected_damage",
    "xC50": "shield_damage",
    # The shield it hit: the bounce's angle and the surface's normal.
    "xC54": "shield_angle",
    "xC58": "shield_normal",
    "xC64_reflectGObj": "reflector",
    "xC68": "reflect_dir",
    "xC6C": "reflect_damage_mul",
    "xC70": "reflect_speed_mul",
    # The reflector's attack (its `attack_flags` and `attack_record`'s parts), which becomes
    # the item's (`Item_80269F14`).
    "xC74": "reflect_attack_flags",
    "xC78": "reflect_record_a",
    "xC80": "reflect_record_b",
    "xC88": "reflect_record_word",
    "xC8C": "reflect_record_short",
    "xC90_absorbGObj": "absorber",
    # Cleared with the damage taken (`it_802753BC`), never read.
    "xC94": "unread_c94",
    # Yoshi's egg's damage behavior (`it_802F2F34`), never read.
    "xC98": "unread_c98",
    "xC9C": "damage_taken",
    # The tick's damage taken, its largest hit, and the damage its hitlag comes from.
    "xCA0": "hit_damage",
    "xCA4": "max_hit_damage",
    "xCA8": "hitlag_damage",
    "xCAC_angle": "kb_angle",
    "xCB0_source_ply": "source_player",
    # The kind of the item that hit it (-1 none).
    "xCB4": "hit_by_kind",
    "xCB8_outDamageDirection": "out_damage_dir",
    "xCBC_hitlagFrames": "hitlag_frames",
    # The least hitlag of a shield it hit (the fighter's `counter_item_min_hitlag`).
    "xCC0": "min_hitlag",
    "xCC4": "hit_element",
    "xCC8_knockback": "knockback",
    "xCCC_incDamageDirection": "damage_dir",
    # Away from the fighter that stepped on it.
    "xCD0": "stomp_dir",
    "xCD4": "hit_item_pos",
    # The position of the item that hit it.
    "xCE0": "hit_by_pos",
    "xCEC_fighterGObj": "attacker",
    # The owner of the item that hit it.
    "xCF0_itemGObj": "attacker_owner",
    # The fighter its hitbox last met (a hit, a shield, a clank).
    "xCF4_fighterGObjUnk": "met_fighter",
    "xCFC": "stomper",
    # The grab kinds it can't grab (against a fighter's `grab_immune`).
    "xD08": "grab_mask",
    # Nonzero: a fighter can't catch it (`it_80274ECC`).
    "xD09": "no_catch",
    "xD0A": "pad_d0a",
    "xD0B": "pad_d0b",
    # 2 while it can't be hit and blinks (`it_802756D0`), and the frames left.
    "xD0C": "invincibility",
    "xD10": "grab_distance",
    "xD3C_spinSpeed": "spin_speed",
    "xD40": "invincible_frames",
    "xD44_lifeTimer": "life_timer",
    "xD48_halfLifeTimer": "half_life",
    "xD4C": "ammo",
    "xD50_landNum": "land_count",
    "xD54_throwNum": "throw_count",
    # The frames a slide uphill has lasted (`it_80277040`).
    "xD58": "slide_frames",
    # The skid's state: 0 none, 1 skidding, 2 told to (`it_80277544`).
    "xD5C": "skid",
    # The two looping sounds' handles, and the last sound played.
    "sfx_unk1": "sfx_loop_1",
    "sfx_unk2": "sfx_loop_2",
    "xD6C": "sfx",
    "xD70": "pickup_sfx",
    "xD74": "throw_sfx",
    "xD78": "drop_sfx",
    # The sound of a destroy of type 1 or 2 (with the attributes' `x68` effect).
    "xD80": "break_sfx",
    "xD84": "bounce_sfx",
    "xD88_attackID": "attack_id",
    "xD8C_attack_instance": "attack_instance",
    "xD8E": "pad_d8e",
    # The attack its hits count as: its flags and its record's parts (a fighter's `attack_flags`
    # and `attack_record`).
    "xD90": "attack_flags",
    "xD94": "attack_record_a",
    "xD9C": "attack_record_b",
    "xDA4_word": "attack_record_word",
    "xDA8_short": "attack_record_short",
    "xDAA": "debug_flags",
    "xDAC_itcmd_var0": "cmd_var0",
    "xDB0_itcmd_var1": "cmd_var1",
    "xDB4_itcmd_var2": "cmd_var2",
    "xDB8_itcmd_var3": "cmd_var3",
    "xDBC": "cmd_flags",
    # Set when pushed by a fighter (1) or an item (2) (`it_80271B60`, `it_80271F78`).
    "xDC0": "push_flags",
    # The holder's part it's held by.
    "xDC4": "hold_part",
    "xDC8_word": "flags",
    # Flag bytes: `b1`, `b2` its reflection, `b3` catchable; which hits it deals (its own
    # player's, its team's); which hits it takes; its drawing; its lifetime, the enemies' and
    # the debug display's; its pushing and spinning.
    "xDCC_flag": "state_flags",
    "xDCD_flag": "hit_flags",
    "xDCE_flag": "hurt_flags",
    "xDCF_flag": "draw_flags",
    "xDD0_flag": "status_flags",
    "xDD1_flag": "phys_flags",
    "xDD2_flag": "unused_dd2",
    "xDD3_flag": "unused_dd3",
    "xDD4_itemVar": "vars",
}

# The structs only `Item` holds: its flag word (`xDC8_word`'s bits), its hitboxes, its dynamics'
# points, its outline color, the items' fighter tracking (`Item_FtTrack`), the links of
# chains and strings (`ItemLink`) and the coin's bone (`it_2E5A_SubVars`); then the kinds'
# variables (`Item_ItemVars`' members), by struct.
ITEM_FLAGS = {
    # Held for another (its owner), the holder in `holder` (`it_802742F4`).
    "x0": "held_for_owner",
    "x1": "not_from_fighter",
    "x2": "holds_still",
    # Hitlag asked for by a hit (`x3`, also every item's pause) or a clank (`x4`), asked to end
    # (`x5`, `x6`), running for a hit or a clank (`x7`, `x8`), any hitlag (`x9`), its frames
    # counting (`xA`) and from damage taken (`xB`).
    "x3": "hit_hitlag_asked",
    "x4": "clank_hitlag_asked",
    "x5": "hit_hitlag_ending",
    "x6": "clank_hitlag_ending",
    "x7": "hit_hitlag",
    "x8": "clank_hitlag",
    "x9": "in_hitlag",
    "xA": "hitlag_counting",
    "xB": "damage_hitlag",
    "xC": "hits_counted",
    # A shield's least hitlag (`min_hitlag`) applies.
    "xD": "shield_hitlag",
    # It keeps its fighter kind's data loaded (`ecb_lock` holds the kind).
    "xE": "holds_fighter_data",
    # The players who have held it, a bit each.
    "xF": "held_by_players",
    "x13": "held",
    "x14": "thrown",
    "x15": "pickup",
    "x16": "hitbox_on",
    "x17": "spin_axis",
    "x19": "turns_to_face",
    "x1A": "keeps_clear",
    "x1B": "random_push",
    "x1C": "clear_of_fighters",
    "x1D": "clear_of_items",
    # Enemies (hold kind 3) push it.
    "x1E": "pushed_by_enemies",
    "x1F": "may_skid",
}

ITEM_HITBOX = {
    # Hits even where fighters take no hits (`gm_8016B1C4`).
    "x138": "always_hits",
}

XB6C = {
    "xB6C": "offset",
    "xB78": "radius",
    "xB7C": "bone",
    "xB84": "pos",
    "xB90": "bone_id",
}

ITEM_MOD = {"x0_unk": "color"}

FT_TRACK = {
    # Set until the first item takes the fighters' positions (`it_802722B0`).
    "x154": "untracked",
}

ITEM_LINK = {
    # Only the reset writes it.
    "x20": "unread_20",
    "x2C_b0": "out",
    "x2C_b1": "touching",
    "x2C_b2": "sounded",
    # The wall line it touched.
    "x1CC": "wall_line",
}

SUB_2E5A = {"x0": "radius", "x4": "bone", "x8": "pos", "x14": "prev_pos"}

ITEM_VARS = {
    "itBombHei_ItemVars": {
        # A countdown: the pulse's, the stand-up turn's, the walk's.
        "xDD4": "timer",
        "xDD8": "pulse_dir",
        "xDDC": "exploded",
        "xDE0": "lit",
        "xDE4": "stood_up",
        "xDE8": "scale",
        "xDEC": "fuse_left",
        "xDF0": "fuse",
        "xDF4": "turn_back",
        "xDF8": "pose_height",
        "xDFC": "pose_tilt",
        "xE04": "turn_step",
        "xE0C": "land_vel",
    },
    "itCLinkMilk_ItemVars": {"x0": "parent"},
    "itCapsule_ItemVars": {"x0": "hit", "x4": "opened"},
    "itClimbersBlizzard_ItemVars": {"x0": "scale", "flag0": "launched"},
    "itClimbersIce_ItemVars": {"x0": "owner", "x4": "scale", "x8_b0": "thrown"},
    "itClimbersString_ItemVars": {"x0": "reel", "x4": "tip", "x8": "hand", "xC": "popo", "x14": "hand_joint"},
    "itCoin_ItemVars": {"x0": "trophy", "x4": "trophy_kind", "x8": "spin", "xC": "spin_max", "x10": "unread_10", "x14": "stand"},
    "itDosei_ItemVars": {
        "xDD4": "frames",
        "xDD8": "turn_state",
        "xDDC": "turn_left",
        "xDE0": "scale",
        "xDE4": "turn_pos",
        "xDF0": "hits_left",
        "xDF8": "skid_normal",
    },
    "itDrMarioPill_ItemVars": {"x0": "facing_dir", "x4": "owner"},
    "itEgg_ItemVars": {"x0": "broken", "_8": "unused_8"},
    "itFFlowerFlame_ItemVars": {"unk": "size"},
    "itFFlower_ItemVars": {"x0": "shots", "x4": "flame_on"},
    "itFlipper_ItemVars": {
        "xDD4_flightTimer": "flight_timer",
        "xDD8_isSettled": "settled",
        "xDDC_hitboxTimer": "hitbox_timer",
        "xDE0_spinAngle": "spin_angle",
        "xDE4_spinVel": "spin_vel",
        "xDE8_isStageFixed": "stage_fixed",
        "xDEC_stageAnchor": "stage_anchor",
    },
    "itFoods_ItemVars": {"x0": "dish"},
    "itFoxBlaster_ItemVars": {
        # The recoil's frame, the muzzle's pulse and its step, whether it has fired, and the
        # last six shots' flashes, kinds and directions.
        "xDD4": "recoil_frame",
        "xDD8": "pulse",
        "xDDC": "pulse_step",
        "xDE0": "fired",
        "xDE4": "flashes",
        "xDFC": "shot_kinds",
        "xE14": "shot_dirs",
    },
    "itFoxIllusion_ItemVars": {"xDD4": "ghost_joint", "xDDC": "ghost"},
    "itFreeze_ItemVars": {"x0": "speed", "x4": "normal", "x10": "handed_speed", "x14": "push_decel", "x18": "stop_frames", "unk_1C": "pusher"},
    "itGShell_ItemVars": {
        "xDD4": "life",
        "xDD8": "hit_delay",
        "xDDC": "trail",
        "xDE0": "owner_grace",
        "xDE4": "pop_timer",
        "xDE8": "landings",
        "xDEC_b0": "bounced",
        "xDEC_b1": "grace_counting",
        "xDEC_b2": "blinking",
        "xDEC_b3": "popped",
        "x58_hurtInit": "hurt_init",
    },
    "itHeart_ItemVars": {"xDD4_heal": "heal", "xDD8": "picked_up", "xDDC": "food_slot"},
    "itKinoko_ItemVars": {"x0": "speed", "x4": "normal"},
    "itKirbyHammer_ItemVars": {"x0": "in_air"},
    "itKirby2F23_ItemVars": {
        # Read and written as the union's bytes (`kinds::kirbyinhale`'s `off`).
        "x1C8": "to_mouth_x",
        "x1CC": "to_mouth_y",
        "x1D0": "shrink_range",
        "x1D4": "shrink_scale",
        "x1D8": "pull_speed_x",
        "x1DC": "pull_speed_y",
        "x1E8": "caught_scale",
        "x1F4": "hold_time",
    },
    "itKoopaFlame_ItemVars": {
        "x0_pos": "pos",
        "xC_direction": "direction",
        "x18_vel": "vel",
        "x24_angle": "angle",
        "x28_speed": "speed",
        "x2C_lifetime": "lifetime",
        "x30": "unread_30",
        "x34_base_scale": "base_scale",
        "x38_base_speed": "base_speed",
        "x3C_scale": "scale",
        "x40_frame_counter": "frame_counter",
        "x44_spawned": "spawned",
        "x48_gfx": "gfx",
    },
    "itKusudama_ItemVars": {"x0": "opened", "x4": "timer"},
    "itLGunBeam_ItemVars": {"xE04": "unread_e04"},
    "itLinkArrow_ItemVars": {
        # The trail: the string's point, then the last four positions and rotations.
        "x18": "string_pos",
        "x24": "trail_pos_0",
        "x30": "trail_pos_1",
        "x3C": "trail_pos_2",
        "x48": "trail_pos_3",
        "x54": "trail_rot_0",
        "x64": "trail_rot_1",
        "x74": "trail_rot_2",
        "x84": "trail_rot_3",
        "x94": "angle",
        "x9C": "wobble_frames",
        "xA0": "loosed",
        "xA4": "damage",
        "xA8": "unread_speed",
        "xAC": "unread_charge",
        "xB0": "frames",
        "xB4": "trails",
        "xC0": "scale",
        # Stuck in a shield: its fighter, the shield's center (its z kept as a word), the
        # distance and angle from it.
        "xC4": "shield_fighter",
        "xC8": "shield_x",
        "xCC": "shield_y",
        "xD0": "shield_z",
        "xD4": "shield_distance",
        "xD8": "shield_angle",
        "xE0": "owner",
        "xE4": "line",
        "xE8": "line_angle",
        "xEC": "last_line_angle",
        "xF0": "stuck_frames",
    },
    "itLinkBomb_ItemVars": {"x0": "flags", "x4": "roll_dir", "x8": "sink", "xC": "tilt", "x10": "parent"},
    "itLinkBoomerang_ItemVars": {
        "xDD8": "trail_slot",
        "xDDC": "trail_slots",
        "xDE4": "unwind_frames",
        "xDE8": "returning",
        "xDEC": "owner_kind",
        "xDF0": "trail_pos",
        "xEB0": "trail_rot",
        "xF70": "flight_left",
        "xF74": "heading",
        "xF78": "roll",
        "xF7C": "bounce_limit",
        "xF80": "home_frames",
        "xF84": "turn_limit",
        "xF88": "unwind_step",
        "xF8C": "sfx_timer",
        "xF90": "trails",
        "xF98": "owner",
    },
    "itLinkBow_ItemVars": {"x0": "scale", "x4": "owner"},
    "itLinkHookshot_ItemVars": {"x0": "tip", "x4": "hand", "x8": "fighter", "xC": "tip_model", "x10": "frame", "x14": "done"},
    "itLipstickSpore_ItemVars": {"xDD4": "facing", "xDD8": "step"},
    "itMBall_ItemVars": {"b0": "open", "b1": "out"},
    "itMasterHandBullet_ItemVars": {"x0": "life"},
    "itMasterHandLaser_ItemVars": {"x0": "ended", "x4": "part", "x8": "reach_frames", "x10": "joint"},
    "itMewtwoShadowball_ItemVars": {
        "x0": "unread_life",
        "x4": "motion",
        "x10": "size",
        "x14": "fired",
        "x18": "charge",
        "x1C": "levels",
        "x20": "sparkle",
        "x24": "unread_damage",
        "x28": "unread_effects",
        "x2C": "maker",
        "x30": "wobble_offset",
        "x3C": "wobble_angle",
        "x40": "wobble_size",
        "x44": "wobble_step",
        "x48": "wobble_frame",
        "x4C": "flight_frames",
        "x50": "wobble_scale",
        "x54": "held_scale",
        "x58": "held_offset",
        "x64": "burst_scale",
    },
    "itMsBomb_ItemVars": {"xDD4_b0": "armed"},
    "itNessYoyo_ItemVars": {"x0": "msid", "x4": "speed", "x8": "tip", "xC": "hand", "x10": "ness", "x18": "model"},
    "itNessbat_ItemVars": {"x0": "ness"},
    "itNesspkthundertrail_ItemVars": {"x0": "ball", "x4": "index", "x8": "frames"},
    "itPKFlushExplode_ItemVars": {"xDD4": "charge", "xDD8": "first_scale", "xDDC": "unread_owner"},
    "itPKFlush_ItemVars": {
        "xDD8_PKFlash": "scale",
        "xDDC_PKFlash": "frames",
        "xDE0_PKFlash_Owner": "owner",
        "xDE4_PKFlash": "unread_e4",
    },
    "itPKThunder_ItemVars": {
        "xDD4": "trails",
        "xEEC": "speed",
        "xEF4": "unread_ef4",
        "xEF8": "frame",
        "xEFC": "trail_count",
        "xF00": "hit_ness",
        "xF04": "ness",
        "xF08": "unread_f08",
    },
    "itPeachTurnip_ItemVars": {"xDD4": "picked_up", "xDD8": "face", "xDDC_damage": "damage", "xDE0_scl": "scale", "xDE4_owner": "owner"},
    "itPikachutJoltAir_ItemVars": {"xDD4": "pikachu", "xDD8": "ground_jolt", "xDE8": "normal", "xDF4": "frames"},
    "itPikachutJoltGround_ItemVars": {
        "xDD4": "angle",
        "xDD8": "pikachu",
        "xDDC": "air_jolt",
        "xDE0": "has_effect",
        "xDE4": "placed_frames",
        "xDE8": "last_pos",
        "xDF4": "normal",
        "xE00": "last_normal",
    },
    "itPikachuthunder_ItemVars": {
        "x0": "index",
        "x4": "hit",
        "x8": "wait",
        "xC": "length",
        "x10": "stretch",
        "x14": "stop_frames",
        "x18": "scale",
        "x1C": "vel",
        "x28": "stop_pos",
        "x34": "next",
        "x38": "owner",
    },
    "itRShell_ItemVars": {
        "xDD4": "life",
        "xDD8": "hit_delay",
        "xDDC": "trail",
        "xDE0": "accel",
        "xDE4": "chase_delay",
        "xDE8": "landings",
        "xDEC": "hits_left",
        "xDF0": "hit",
        "xDF4_b0": "bounced",
    },
    "itSamusBomb_ItemVars": {"x0": "exploded"},
    "itSamusChargeshot_ItemVars": {
        "xDD8": "angle",
        "xDDC": "speed",
        "xDE0": "unread_e0",
        "xDE4": "size",
        "xDE8": "fired",
        "xDEC": "charge",
        "xDF0": "levels",
        "xDF4": "sparkle",
        "xDF8": "unread_damage",
        "xDFC": "effects",
        "xE00": "maker",
        "xE04": "unread_scale",
        "xE2C": "unread_e2c",
        "xE30": "unread_e30",
        "xE34": "unread_e34",
    },
    "itSamusGrapple_ItemVars": {"x0": "tip", "x4": "hand", "x8": "fighter", "xC": "tip_model", "unk_10": "frame", "x14": "done", "x15": "links_made", "x16": "hanging"},
    "itSamusMissile_ItemVars": {"x4": "msid", "x8": "homing_left"},
    "itScopeBeam_ItemVars": {"x0": "charge"},
    "itSeakChain_ItemVars": {"x0": "tip", "x4": "hand", "x10": "swing_arg", "x14": "unread_14", "x18": "coll_state"},
    "itSeakNeedleThrown_ItemVars": {
        "xDD4": "spin",
        "xDD8": "tumble_vel",
        "xDDC": "tumble_max",
        "xDE0": "tumble_accel",
        "xDE4": "last_pos",
        "xDF0": "angle",
        "xDF4": "line",
        "xDF8": "line_angle",
        "xDFC": "last_line_angle",
    },
    "itStarRodStar_ItemVars": {"x0": "big", "x4": "spin"},
    "itSword_ItemVars": {
        "x0": "unread_0",
        # The blade's length, width and swing, each eased toward a target: frames left, end
        # frame, step's sign, value, target, step.
        "x4": "length_frames",
        "x8": "length_end",
        "xC": "length_dir",
        "x10": "length",
        "x14": "length_target",
        "x18": "length_step",
        "x1C": "width_frames",
        "x20": "width_end",
        "x24": "width_dir",
        "x28": "width",
        "x2C": "width_target",
        "x30": "width_step",
        "x34": "swing_frames",
        "x38": "swing_end",
        "x3C": "swing_dir",
        "x40": "swing",
        "x44": "swing_target",
        "x48": "swing_step",
        "x4C": "base_width",
        "x50": "unread_50",
        "x54": "unread_54",
    },
    "itTaru_ItemVars": {"xDD4": "hit", "xDD8": "open_frames", "xDDC": "tilt", "xDE0": "turn", "xDE4": "roll_speed", "xDE8": "normal"},
    "itTarucann_ItemVars": {
        "x0": "unread_0",
        "x4": "fire_frames",
        "x8": "unread_8",
        "xC": "turn",
        "x10": "roll_speed",
        "x14": "normal",
        "x20": "rider",
        "x24": "unread_24",
        "x28": "scale",
    },
    "itTomato_ItemVars": {"x4_b0": "picked_up", "x8": "unread_8"},
    "itWstar_ItemVars": {"xDD4": "speed", "xDD8": "hit", "xDDC": "ride_anim"},
    "itYoshiEggLay_ItemVars": {"x0": "damage_mul"},
    "itZeldaDinFireExplode_ItemVars": {"xDD4": "size", "xDD8": "first_scale", "xDDC": "unread_parent", "xDE0": "started"},
    "itZeldaDinFire_ItemVars": {
        "xDD8": "scale",
        "xDDC": "frames",
        "xDE0": "zelda",
        "xDE4": "unread_e4",
        "xDE8": "turn",
        "xDEC": "rot",
        "xDF0": "speed",
        "xDF4": "started",
    },
    "it_2E5A_ItemVars": {"x0": "worth", "x4": "tier", "x8": "pickable", "xC": "flying", "x10": "wait_a", "x14": "wait_b", "x18": "flags"},
}

# Fields nothing reads or writes (the decomp's padding `pad_*`, else `unused_*`, by offset).
UNUSED_ITEM_FIELDS = {
    "ItemHitbox": {"x138_b1_7": "unused_138_b1_7", "x139": "unused_139"},
    "ItemLink": {"x2C_b3": "unused_2c_b3", "x2C_b4": "unused_2c_b4", "x2C_b5": "unused_2c_b5", "x2C_b6": "unused_2c_b6", "x2C_b7": "unused_2c_b7"},
    "Item_FtTrack": {"xB0": "unused_b0", "xB4": "unused_b4", "xB8": "unused_b8", "xBC": "unused_bc", "x144": "unused_144", "x148": "unused_148", "x14C": "unused_14c"},
    "itBombHei_ItemVars": {"_E00": "unused_e00", "_E08": "unused_e08"},
    "itDosei_ItemVars": {"xDF4": "unused_df4"},
    "itDrMarioPill_ItemVars": {"x8": "unused_8", "xC": "unused_c"},
    "itFoxIllusion_ItemVars": {"xDD8": "unused_dd8"},
    "itFreeze_ItemVars": {"x20": "unused_20"},
    "itGShell_ItemVars": {"xDEC_b4": "unused_dec_b4", "xDEC_b5": "unused_dec_b5", "xDEC_b6": "unused_dec_b6", "xDEC_b7": "unused_dec_b7", "xDF0": "unused_df0", "xDF4": "unused_df4", "xDF8": "unused_df8", "xDFC": "unused_dfc", "xE00": "unused_e00", "xE04": "unused_e04", "xE08_b0": "unused_e08_b0", "xE08_b1": "unused_e08_b1", "xE08_b2": "unused_e08_b2", "xE08_b3": "unused_e08_b3", "xE08_b4": "unused_e08_b4", "xE08_b5": "unused_e08_b5", "xE08_b6": "unused_e08_b6", "xE08_b7": "unused_e08_b7", "pad_E09": "pad_35", "xE0C": "unused_e0c", "xE10": "unused_e10", "xE14": "unused_e14", "xE18": "unused_e18", "xE1C_b0": "unused_e1c_b0", "xE1C_b1": "unused_e1c_b1", "xE1C_b2": "unused_e1c_b2", "xE1C_b3": "unused_e1c_b3", "xE1C_b4": "unused_e1c_b4", "xE1C_b5": "unused_e1c_b5", "xE1C_b6": "unused_e1c_b6", "xE1C_b7": "unused_e1c_b7", "pad_E1D": "pad_49"},
    "itHeart_ItemVars": {"xDE0_filler": "unused_de0_filler", "xDE8_float": "unused_de8_float", "xDEC": "unused_dec"},
    "itKirby2F23_ItemVars": {"_pad": "pad_0", "_pad2": "pad_1e0"},
    "itLinkArrow_ItemVars": {"x0": "unused_0", "x4": "unused_4", "x8": "unused_8", "xC": "unused_c", "x10": "unused_10", "x14": "unused_14", "x98": "unused_98", "xDC": "unused_dc"},
    "itLinkBomb_ItemVars": {"x1": "unused_1", "x2": "unused_2", "x3": "unused_3"},
    "itLinkBoomerang_ItemVars": {"xDD4": "unused_dd4"},
    "itLinkHookshot_ItemVars": {"x18": "unused_18", "x1C": "unused_1c", "x20": "unused_20", "x24": "unused_24", "x28": "unused_28", "x2C": "unused_2c", "x30": "unused_30", "x34": "unused_34", "x38": "unused_38", "x3C": "unused_3c", "x40": "unused_40", "x44": "unused_44", "x48": "unused_48", "x4C": "unused_4c", "x50": "unused_50", "x54": "unused_54", "x58": "unused_58", "x5C": "unused_5c", "x60": "unused_60"},
    "itMBall_ItemVars": {"b2": "unused_b2", "b3": "unused_b3", "b4": "unused_b4", "b5": "unused_b5", "b6": "unused_b6", "b7": "unused_b7"},
    "itMsBomb_ItemVars": {"xDD4_b1": "unused_dd4_b1", "xDD4_b2": "unused_dd4_b2", "xDD4_b3": "unused_dd4_b3", "xDD4_b4": "unused_dd4_b4", "xDD4_b5": "unused_dd4_b5", "xDD4_b6": "unused_dd4_b6", "xDD4_b7": "unused_dd4_b7"},
    "itPKFlush_ItemVars": {"xDD4_PKFlash": "unused_dd4_pkflash"},
    "itPKThunder_ItemVars": {"xEF0": "unused_ef0"},
    "itPeachTurnip_ItemVars": {"xDE8_scl": "unused_de8_scl"},
    "itRShell_ItemVars": {"xDF4_b1": "unused_df4_b1", "xDF4_b2": "unused_df4_b2", "xDF4_b3": "unused_df4_b3", "xDF4_b4": "unused_df4_b4", "xDF4_b5": "unused_df4_b5", "xDF4_b6": "unused_df4_b6", "xDF4_b7": "unused_df4_b7", "pad_DF5": "pad_21"},
    "itSamusChargeshot_ItemVars": {"xDD4_pad": "unused_dd4_pad", "pad1": "pad_34", "padding": "pad_64"},
    "itSamusMissile_ItemVars": {"x10": "unused_10"},
    "itScopeBeam_ItemVars": {"x4": "unused_4"},
    "itSeakChain_ItemVars": {"pad_C": "pad_c"},
    "itStar_ItemVars": {"x4": "unused_4", "x8": "unused_8", "xC": "unused_c"},
    "itSword_ItemVars": {"x58": "unused_58"},
    "itZeldaDinFire_ItemVars": {"padDD4": "pad_0"},
    "xB6C_t": {"xB80": "unused_b80"},
}

STEP_2 = {
    "Item": ITEM,
    "flag32_flags": ITEM_FLAGS,
    "ItemHitbox": ITEM_HITBOX,
    "xB6C_t": XB6C,
    "ItemModStruct": ITEM_MOD,
    "Item_FtTrack": FT_TRACK,
    "ItemLink": ITEM_LINK,
    "it_2E5A_SubVars": SUB_2E5A,
    **ITEM_VARS,
}

FIELD_NAMES = {}
for _struct in sorted(set(STEP_2) | set(UNUSED_ITEM_FIELDS)):
    FIELD_NAMES[_struct] = {**STEP_2.get(_struct, {}), **UNUSED_ITEM_FIELDS.get(_struct, {})}

# Step 3 (task 23): the other generated structs, in `names_step3.py`.
from names_step3 import STEP_3  # noqa: E402

for _struct, _names in STEP_3.items():
    FIELD_NAMES.setdefault(_struct, {}).update(_names)

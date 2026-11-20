#!/usr/bin/env python3
"""Transcribes the items' static tables from the executable into a Rust table: each kind's
render function (`it_803F1418` and its siblings), each kind's logic table (`it_803F14C4` and
its siblings: its states and its callbacks), and every state table those name (each state's
animation and three callbacks). Functions are their addresses, which melee-sim dispatches on.

Each kind is named by its `ItKind` variant (melee-sim's `game::it::registry`): the decomp's
`ItemKind` name spelled as a variant (`It_Kind_Hammer_Head` is `HammerHead`), or the name
players know it by where `RENAME_KINDS` gives one (`It_Kind_BombHei` is `BobOmb`). Each state
table is named by the first kind whose logic table names it (`STATES_BOB_OMB`), its symbol and
its old name kept as `#[doc(alias)]`.

Usage: item_tables.py MAIN_DOL SYMBOLS_TXT FORWARD_H OUT_RS
  MAIN_DOL is the disc's main.dol (the decomp extracts it to orig/GALE01/sys/main.dol);
  SYMBOLS_TXT is the decomp's config/GALE01/symbols.txt;
  FORWARD_H is the decomp's src/melee/it/forward.h, whose `ItemKind` names the kinds.
"""

import re
import struct
import sys

# The four groups of kinds, in kind order: (first kind, render table, logic table).
GROUPS = [
    ("COMMON", 0, "it_803F1418", "it_803F14C4"),
    ("CHAR", 43, "it_803F2F28", "it_803F3100"),
    ("POKEMON", 161, "it_803F2310", "it_803F23CC"),
    ("STAGE", 208, "it_803F4CA8", "it_803F4D20"),
]

# State tables code sets itself rather than through a logic table, and their names: an item
# Kirby's inhale holds (`it_802F23EC`), and one Yoshi's tongue or Kirby's copy of it holds
# (`it_802F2BFC`, `it_802F2D98`).
EXTRA_STATES = {
    "it_803F9450": ("KIRBY_INHALED", "an item Kirby's inhale holds"),
    "it_803F9480": ("YOSHI_TONGUED", "an item Yoshi's tongue holds"),
    "it_803F9490": ("KIRBY_TONGUED", "an item Kirby's copy of Yoshi's tongue holds"),
}


def snake(variant):
    """`NessPKFire` as `NESS_PK_FIRE`."""
    return re.sub(r'(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])', '_', variant).upper()

CALLBACKS = [
    "spawned", "destroyed", "picked_up", "dropped", "thrown", "dmg_dealt", "dmg_received",
    "entered_air", "reflected", "clanked", "absorbed", "shield_bounced", "hit_shield", "evt_unk",
]


def load_dol(path):
    b = open(path, 'rb').read()
    offs = struct.unpack('>18I', b[0:72])
    addrs = struct.unpack('>18I', b[72:144])
    sizes = struct.unpack('>18I', b[144:216])
    sections = [(a, o, s) for a, o, s in zip(addrs, offs, sizes) if s]

    def read(addr, n):
        for a, o, s in sections:
            if a <= addr and addr + n <= a + s:
                return b[o + addr - a:o + addr - a + n]
        raise ValueError(f'{addr:#x} not in the DOL')
    return read


# The decomp's markers that share a kind's number, and the names melee-hd changes: the names
# players know the kinds by, where they differ from the decomp's.
SKIP_KINDS = {'It_PKind_Start', 'It_PKind_Terminate', 'It_Kind_Unselected', 'It_Kind_None'}
RENAME_KINDS = {
    'It_Kind_Box': 'Crate',
    'It_Kind_Taru': 'Barrel',
    'It_Kind_Kusudama': 'PartyBall',
    'It_Kind_TaruCann': 'BarrelCannon',
    'It_Kind_BombHei': 'BobOmb',
    'It_Kind_Dosei': 'MrSaturn',
    'It_Kind_Heart': 'HeartContainer',
    'It_Kind_Tomato': 'MaximTomato',
    'It_Kind_Star': 'Starman',
    'It_Kind_Bat': 'HomeRunBat',
    'It_Kind_Sword': 'BeamSword',
    'It_Kind_G_Shell': 'GreenShell',
    'It_Kind_R_Shell': 'RedShell',
    'It_Kind_L_Gun': 'RayGun',
    'It_Kind_Freeze': 'Freezie',
    'It_Kind_Foods': 'Food',
    'It_Kind_MSBomb': 'MotionSensorBomb',
    'It_Kind_S_Scope': 'SuperScope',
    'It_Kind_LipStick': 'LipsStick',
    'It_Kind_Harisen': 'Fan',
    'It_Kind_F_Flower': 'FireFlower',
    'It_Kind_Kinoko': 'SuperMushroom',
    'It_Kind_DKinoko': 'PoisonMushroom',
    'It_Kind_WStar': 'WarpStar',
    'It_Kind_ScBall': 'ScrewAttack',
    'It_Kind_RabbitC': 'BunnyHood',
    'It_Kind_MetalB': 'MetalBox',
    'It_Kind_Spycloak': 'CloakingDevice',
    'It_Kind_M_Ball': 'PokeBall',
    'It_Kind_L_Gun_Ray': 'RayGunShot',
    'It_Kind_LipStick_Spore': 'LipsStickSpore',
    'It_Kind_S_Scope_Beam': 'SuperScopeShot',
    'It_Kind_L_Gun_Beam': 'FireFlowerFlame',
    'It_Kind_F_Flower_Flame': 'LipsStickFlower',
    'It_Kind_EvYoshiEgg': 'EventYoshiEgg',
    'It_Kind_Kuriboh': 'Goomba',
    'It_Kind_Leadead': 'ReDead',
    'It_Kind_Octarock': 'Octorok',
    'It_Kind_Ottosea': 'Topi',
    'It_Kind_Octarock_Stone': 'OctorokStone',
    'It_Kind_Mario_Fire': 'MarioFireball',
    'It_Kind_DrMario_Vitamin': 'DrMarioMegavitamin',
    'It_Kind_Kirby_CBeam': 'KirbyFinalCutterWave',
    'It_Kind_Unk1': 'KirbyCopyStar',
    'It_Kind_Unk2': 'KirbySpitStar',
    'It_Kind_CLink_Bomb': 'YoungLinkBomb',
    'It_Kind_CLink_Boomerang': 'YoungLinkBoomerang',
    'It_Kind_Link_HShot': 'LinkHookshot',
    'It_Kind_CLink_HShot': 'YoungLinkHookshot',
    'It_Kind_CLink_Arrow': 'YoungLinkFireArrow',
    'It_Kind_Ness_PKFire_Flame': 'NessPKFirePillar',
    'It_Kind_Ness_PKFlush': 'NessPKFlash',
    'It_Kind_Ness_PKThunder1': 'NessPKThunderTrail1',
    'It_Kind_Ness_PKThunder2': 'NessPKThunderTrail2',
    'It_Kind_Ness_PKThunder3': 'NessPKThunderTrail3',
    'It_Kind_Ness_PKThunder4': 'NessPKThunderTrail4',
    'It_Kind_CLink_Bow': 'YoungLinkBow',
    'It_Kind_Ness_PKFlush_Explode': 'NessPKFlashExplosion',
    'It_Kind_Seak_NeedleThrow': 'SheikNeedleThrown',
    'It_Kind_Seak_NeedleHeld': 'SheikNeedleHeld',
    'It_Kind_DrMario_Sheet': 'DrMarioSuperSheet',
    'It_Kind_Seak_Vanish': 'SheikVanish',
    'It_Kind_Pikachu_TJolt_Ground': 'PikachuThunderJoltGround',
    'It_Kind_Pikachu_TJolt_Air': 'PikachuThunderJoltAir',
    'It_Kind_Pichu_TJolt_Ground': 'PichuThunderJoltGround',
    'It_Kind_Pichu_TJolt_Air': 'PichuThunderJoltAir',
    'It_Kind_Samus_Charge': 'SamusChargeShot',
    'It_Kind_Samus_GBeam': 'SamusGrappleBeam',
    'It_Kind_Seak_Chain': 'SheikChain',
    'It_Kind_Peach_Explode': 'PeachBomberExplosion',
    'It_Kind_Koopa_Flame': 'BowserFlame',
    'It_Kind_Luigi_Fire': 'LuigiFireball',
    'It_Kind_IceClimber_Ice': 'IceClimbersIceShot',
    'It_Kind_IceClimber_Blizzard': 'IceClimbersBlizzard',
    'It_Kind_Zelda_DinFire': 'ZeldaDinsFire',
    'It_Kind_Zelda_DinFire_Explode': 'ZeldaDinsFireExplosion',
    'It_Kind_IceClimber_GumStrings': 'IceClimbersBelayRope',
    'It_Kind_GameWatch_Greenhouse': 'GameAndWatchSprayer',
    'It_Kind_GameWatch_Manhole': 'GameAndWatchManhole',
    'It_Kind_GameWatch_Fire': 'GameAndWatchTorch',
    'It_Kind_GameWatch_Parachute': 'GameAndWatchParachute',
    'It_Kind_GameWatch_Turtle': 'GameAndWatchTurtle',
    'It_Kind_GameWatch_Breath': 'GameAndWatchSparky',
    'It_Kind_GameWatch_Judge': 'GameAndWatchJudge',
    'It_Kind_GameWatch_Panic': 'GameAndWatchOil',
    'It_Kind_GameWatch_Chef': 'GameAndWatchSausage',
    'It_Kind_CLink_Milk': 'YoungLinkMilk',
    'It_Kind_GameWatch_Rescue': 'GameAndWatchTrampoline',
    'It_Kind_Kirby_MarioFire': 'KirbyMarioFireball',
    'It_Kind_Kirby_DrMarioVitamin': 'KirbyDrMarioMegavitamin',
    'It_Kind_Kirby_LuigiFire': 'KirbyLuigiFireball',
    'It_Kind_Kirby_IceClimberIce': 'KirbyIceClimbersIceShot',
    'It_Kind_Kirby_CLinkArrow': 'KirbyYoungLinkFireArrow',
    'It_Kind_Kirby_CLinkBow': 'KirbyYoungLinkBow',
    'It_Kind_Kirby_NessPKFlush': 'KirbyNessPKFlash',
    'It_Kind_Kirby_NessPKFlush_Explode': 'KirbyNessPKFlashExplosion',
    'It_Kind_Kirby_PikachuTJolt_Ground': 'KirbyPikachuThunderJoltGround',
    'It_Kind_Kirby_PikachuTJolt_Air': 'KirbyPikachuThunderJoltAir',
    'It_Kind_Kirby_PichuTJolt_Ground': 'KirbyPichuThunderJoltGround',
    'It_Kind_Kirby_PichuTJolt_Air': 'KirbyPichuThunderJoltAir',
    'It_Kind_Kirby_SamusCharge': 'KirbySamusChargeShot',
    'It_Kind_Kirby_SeakNeedleThrow': 'KirbySheikNeedleThrown',
    'It_Kind_Kirby_SeakNeedleHeld': 'KirbySheikNeedleHeld',
    'It_Kind_Kirby_KoopaFlame': 'KirbyBowserFlame',
    'It_Kind_Kirby_GameWatchChef': 'KirbyGameAndWatchSausage',
    'It_Kind_Kirby_GameWatchChefPan': 'KirbyGameAndWatchPan',
    'It_Kind_Unk4': 'VsCoin',
    'It_Kind_Coin': 'Trophy',
    'It_PKind_Random': 'StageHurtbox',
    'It_PKind_Tosakinto': 'Goldeen',
    'It_PKind_Chicorita': 'Chikorita',
    'It_PKind_Kabigon': 'Snorlax',
    'It_PKind_Kamex': 'Blastoise',
    'It_PKind_Matadogas': 'Weezing',
    'It_PKind_Lizardon': 'Charizard',
    'It_PKind_Fire': 'Moltres',
    'It_PKind_Thunder': 'Zapdos',
    'It_PKind_Freezer': 'Articuno',
    'It_PKind_Sonans': 'Wobbuffet',
    'It_PKind_Hassam': 'Scizor',
    'It_PKind_Unknown': 'Unown',
    'It_PKind_Suikun': 'Suicune',
    'It_PKind_Kireihana': 'Bellossom',
    'It_PKind_Marumine': 'Electrode',
    'It_PKind_Houou': 'HoOh',
    'It_PKind_Metamon': 'Ditto',
    'It_PKind_Pippi': 'Clefairy',
    'It_PKind_Togepy': 'Togepi',
    'It_PKind_Cerebi': 'Celebi',
    'It_PKind_Hitodeman': 'Staryu',
    'It_PKind_Lucky': 'Chansey',
    'It_PKind_Hinoarashi': 'Cyndaquil',
    'It_PKind_Maril': 'Marill',
    'It_PKind_Fushigibana': 'Venusaur',
    'It_Kind_Chicorita_Leaf': 'ChikoritaLeaf',
    'It_Kind_Kamex_HydroPump': 'BlastoiseHydroPump',
    'It_Kind_Matadogas_Gas1': 'WeezingGas1',
    'It_Kind_Matadogas_Gas2': 'WeezingGas2',
    'It_Kind_Lizardon_Flame1': 'CharizardFlame1',
    'It_Kind_Lizardon_Flame2': 'CharizardFlame2',
    'It_Kind_Lizardon_Flame3': 'CharizardFlame3',
    'It_Kind_Lizardon_Flame4': 'CharizardFlame4',
    'It_Kind_Unknown_Swarm': 'UnownSwarm',
    'It_Kind_Houou_SacredFire': 'HoOhSacredFire',
    'It_Kind_Hitodeman_Star': 'StaryuStar',
    'It_Kind_Lucky_Egg': 'ChanseyEgg',
    'It_Kind_Hinoarashi_Flame': 'CyndaquilFlame',
    'It_Kind_Pokemon_Unk': 'PokemonUnused',
    'It_Kind_Old_Kuri': 'OldGoomba',
    'It_Kind_Mato': 'Target',
    'It_Kind_Heiho': 'ShyGuy',
    'It_Kind_Nokonoko': 'KoopaTroopa',
    'It_Kind_Patapata': 'KoopaParatroopa',
    'It_Kind_Likelike': 'LikeLike',
    'It_Kind_Old_Lead': 'OldReDead',
    'It_Kind_Old_Octa': 'OldOctorok',
    'It_Kind_Old_Otto': 'OldTopi',
    'It_Kind_Whitebea': 'PolarBear',
    'It_Kind_Klap': 'KlapTrap',
    'It_Kind_ZGShell': 'KoopaGreenShell',
    'It_Kind_ZRShell': 'KoopaRedShell',
    'It_Kind_Tincle': 'Tingle',
    'It_Kind_Tools': 'FlatZoneTool',
    'It_Kind_Kyasarin': 'Birdo',
    'It_Kind_Kyasarin_Egg': 'BirdoEgg',
}


def kind_names(path):
    """Each kind's `ItKind` variant, by number, from the decomp's `enum ItemKind`."""
    src = open(path, encoding='utf-8').read()
    body = src[src.index('typedef enum ItemKind {'):src.index('} ItemKind;')]
    body = re.sub(r'//[^\n]*|/\*.*?\*/', '', body, flags=re.S)
    values, names, n = {}, {}, -1
    for m in re.finditer(r'\b(It_P?Kind_\w+)\s*(?:=\s*(-?\w+))?\s*,', body):
        name, value = m.groups()
        n = (int(value, 0) if re.fullmatch(r'-?\d+|0x[0-9A-Fa-f]+', value) else values[value]) if value else n + 1
        values[name] = n
        if name in SKIP_KINDS:
            continue
        s = re.sub(r'^It_P?Kind_', '', name)
        names[n] = RENAME_KINDS.get(name, ''.join(p[:1].upper() + p[1:] for p in s.split('_')))
    return names


def code(a):
    return 'None' if a == 0 else f'Some(CodeRef({a:#x}))'


def main():
    read = load_dol(sys.argv[1])
    kinds = kind_names(sys.argv[3])
    kind_label = lambda k: f'ItKind::{kinds[k]}' if k in kinds else f'kind {k}, no `ItKind`'

    names, objects, by_name = {}, {}, {}
    for line in open(sys.argv[2], encoding='utf-8'):
        m = re.match(r'(\S+) = \.(\w+):0x([0-9A-Fa-f]+); // type:(\w+)(?: size:0x([0-9A-Fa-f]+))?', line)
        if not m:
            continue
        name, _, addr, kind, size = m.groups()
        addr = int(addr, 16)
        if kind == 'function':
            names[addr] = name
        elif size:
            objects[addr] = (name, int(size, 16))
            by_name[name] = (addr, int(size, 16))
    out = [
        '// SPDX-License-Identifier: GPL-3.0-or-later',
        '// Generated by tools/tables/item_tables.py from the executable. Do not edit.',
        '#![allow(clippy::all)]',
        '',
        'use super::tables::{ItemLogic, ItemState};',
        'use crate::game::handles::CodeRef;',
        '',
    ]
    state_tables = set()
    owners = {}  # a state table's address: the kinds whose logic tables name it
    for group, first, render, logic in GROUPS:
        raddr, rsize = by_name[render]
        n = rsize // 4
        laddr, lsize = by_name[logic]
        assert lsize // 0x3C == n, (logic, lsize, n)
        out.append(f'/// `{render}`: each kind\'s render function, from `{kind_label(first)}`.')
        out.append(f'pub static RENDER_{group}: [CodeRef; {n}] = [')
        for i in range(n):
            a = struct.unpack('>I', read(raddr + 4 * i, 4))[0]
            out.append(f'    CodeRef({a:#x}), // {names.get(a, "?")}')
        out.append('];')
        out.append('')
        out.append(f'/// `{logic}`, at {laddr:#x}: each kind\'s logic table, from `{kind_label(first)}`.')
        out.append(f'pub static LOGIC_{group}: [ItemLogic; {n}] = [')
        for i in range(n):
            w = struct.unpack('>15I', read(laddr + 0x3C * i, 0x3C))
            out.append(f'    // {kind_label(first + i)}')
            out.append('    ItemLogic {')
            out.append(f'        states: {w[0]:#x},')
            if w[0]:
                state_tables.add(w[0])
                owners.setdefault(w[0], []).append(first + i)
            for field, a in zip(CALLBACKS, w[1:]):
                out.append(f'        {field}: {code(a)},')
            out.append('    },')
        out.append('];')
        out.append('')
    extra = {}
    for name, (const, what) in EXTRA_STATES.items():
        state_tables.add(by_name[name][0])
        extra[by_name[name][0]] = (const, what)
    entries = []
    for addr in sorted(state_tables):
        # A table can start inside a symbol's object (several kinds share its later states).
        base = max(a for a in objects if a <= addr)
        name, size = objects[base]
        assert addr < base + size, f'{addr:#x}'
        n = (base + size - addr) // 0x10
        if base != addr:
            name = f'{name}+{addr - base:#x}'
        # Named by the first kind whose logic table names it (or the code that sets it), the
        # symbol (and the name it had) kept as aliases.
        old = 'STATES_' + name.upper().replace('+0X', '_')
        if addr in owners:
            ks = owners[addr]
            const = 'STATES_' + snake(kinds[ks[0]])
            what = ', '.join(f'`ItKind::{kinds[k]}`' for k in ks)
            what = f'the states of {what}'
        else:
            const, what = extra[addr]
            const = 'STATES_' + const
            what = f'the states of {what}'
        assert all(const != c for _, c in entries), const
        aliases = f'"{old}", "{name}"' if base == addr else f'"{old}"'
        out.append(f'/// `{name}`: {what}.')
        out.append(f'#[doc(alias({aliases}))]')
        out.append(f'pub static {const}: [ItemState; {n}] = [')
        for i in range(n):
            w = struct.unpack('>4I', read(addr + 0x10 * i, 0x10))
            anim = w[0] if w[0] < 0x80000000 else w[0] - (1 << 32)
            label = ', '.join(names.get(a, '-') if a else '-' for a in w[1:])
            out.append(f'    // {i}: {label}')
            out.append(f'    ItemState {{ anim_id: {anim}, animated: {code(w[1])}, physics_updated: {code(w[2])}, collided: {code(w[3])} }},')
        out.append('];')
        out.append('')
        entries.append((addr, const))
    out.append('/// Every state table, by its address.')
    out.append(f'pub static STATE_TABLES: [(u32, &[ItemState]); {len(entries)}] = [')
    for addr, const in entries:
        out.append(f'    ({addr:#x}, &{const}),')
    out.append('];')
    out.append('')
    open(sys.argv[4], 'w', newline='\n', encoding='utf-8').write('\n'.join(out))
    print(f'{len(entries)} state tables')


if __name__ == '__main__':
    main()

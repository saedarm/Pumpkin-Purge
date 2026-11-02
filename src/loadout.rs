//! Weapon attachments: the catalog, what each one changes, and the math that turns a
//! weapon's base stats + attachments + shop upgrades into the numbers the game uses.
//!
//! Each weapon has five slots: Sight, Barrel, Magazine, Grip, Special. You buy an
//! attachment once per weapon; after that you can swap it on and off for free in the shop.

use crate::weapons::WEAPONS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Sight,
    Barrel,
    Magazine,
    Grip,
    Special,
}

pub const SLOTS: [Slot; 5] = [Slot::Sight, Slot::Barrel, Slot::Magazine, Slot::Grip, Slot::Special];

impl Slot {
    pub fn name(self) -> &'static str {
        match self {
            Slot::Sight => "Sight",
            Slot::Barrel => "Barrel",
            Slot::Magazine => "Magazine",
            Slot::Grip => "Grip",
            Slot::Special => "Special",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Special {
    None,
    /// Bullets go through up to three scarecrows.
    Pierce,
    /// Pellets set scarecrows on fire.
    Burn,
    /// Hits slow scarecrows to half speed.
    Caramel,
    /// Each blast throws out four smaller bomblets.
    Cluster,
}

/// What an attachment changes. Multipliers are 1.0 for "no change".
#[derive(Clone, Copy, Debug)]
pub struct Mods {
    pub damage: f32,
    /// Spread from the hip (lower = tighter).
    pub spread: f32,
    /// Extra spread multiplier only while aiming down sights.
    pub aim_spread: f32,
    pub kick: f32,
    pub mag: f32,
    pub reload: f32,
    /// Aim-down-sights zoom. 0 = keep the default.
    pub zoom: f32,
    /// How fast you raise the sights (scopes are slower).
    pub aim_speed: f32,
    /// Launcher projectile speed.
    pub velocity: f32,
    /// Launcher blast radius.
    pub blast: f32,
    pub special: Special,
}

pub const NO_MODS: Mods = Mods {
    damage: 1.0,
    spread: 1.0,
    aim_spread: 1.0,
    kick: 1.0,
    mag: 1.0,
    reload: 1.0,
    zoom: 0.0,
    aim_speed: 1.0,
    velocity: 1.0,
    blast: 1.0,
    special: Special::None,
};

impl Mods {
    fn and(self, o: Mods) -> Mods {
        Mods {
            damage: self.damage * o.damage,
            spread: self.spread * o.spread,
            aim_spread: self.aim_spread * o.aim_spread,
            kick: self.kick * o.kick,
            mag: self.mag * o.mag,
            reload: self.reload * o.reload,
            zoom: self.zoom.max(o.zoom),
            aim_speed: self.aim_speed * o.aim_speed,
            velocity: self.velocity * o.velocity,
            blast: self.blast * o.blast,
            special: if o.special != Special::None { o.special } else { self.special },
        }
    }
}

pub struct Attachment {
    pub name: &'static str,
    pub desc: &'static str,
    pub cost: u32,
    pub mods: Mods,
    /// Which model to show on the gun (see `weapons.rs`).
    pub look: Look,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    RedDot,
    Scope,
    Barrel,
    Choke,
    Mag,
    Drum,
    Tubes,
    Grip,
    Glow,
}

const RED_DOT: Attachment = Attachment {
    name: "Red Dot",
    desc: "Reflex sight. More zoom and tighter aim when aiming down sights.",
    cost: 120,
    mods: Mods { zoom: 1.45, aim_spread: 0.6, ..NO_MODS },
    look: Look::RedDot,
};

const PISTOL: [&[Attachment]; 5] = [
    &[
        RED_DOT,
        Attachment {
            name: "Corn Cob Scope",
            desc: "2.6x scope for picking off heads at range. Slower to raise.",
            cost: 220,
            mods: Mods { zoom: 2.6, aim_spread: 0.3, aim_speed: 0.6, ..NO_MODS },
            look: Look::Scope,
        },
    ],
    &[Attachment {
        name: "Long Barrel",
        desc: "+25% damage and 40% tighter spread.",
        cost: 180,
        mods: Mods { damage: 1.25, spread: 0.6, ..NO_MODS },
        look: Look::Barrel,
    }],
    &[Attachment {
        name: "Extended Mag",
        desc: "+50% magazine size.",
        cost: 150,
        mods: Mods { mag: 1.5, ..NO_MODS },
        look: Look::Mag,
    }],
    &[Attachment {
        name: "Grip Tape",
        desc: "45% less recoil, slightly tighter hip fire.",
        cost: 120,
        mods: Mods { kick: 0.55, spread: 0.8, ..NO_MODS },
        look: Look::Grip,
    }],
    &[Attachment {
        name: "Candy Corn Piercers",
        desc: "Rounds punch through up to three scarecrows in a line.",
        cost: 350,
        mods: Mods { special: Special::Pierce, ..NO_MODS },
        look: Look::Glow,
    }],
];

const SCATTERGUN: [&[Attachment]; 5] = [
    &[Attachment { mods: Mods { zoom: 1.35, aim_spread: 0.7, ..NO_MODS }, ..RED_DOT }],
    &[Attachment {
        name: "Full Choke",
        desc: "40% tighter pellet spread. Reaches farther.",
        cost: 180,
        mods: Mods { spread: 0.6, ..NO_MODS },
        look: Look::Choke,
    }],
    &[Attachment {
        name: "Speed Loader",
        desc: "40% faster reloads.",
        cost: 160,
        mods: Mods { reload: 0.6, ..NO_MODS },
        look: Look::Mag,
    }],
    &[Attachment {
        name: "Pump Grip",
        desc: "45% less recoil.",
        cost: 140,
        mods: Mods { kick: 0.55, ..NO_MODS },
        look: Look::Grip,
    }],
    &[Attachment {
        name: "Hot Sauce Shells",
        desc: "Pellets set scarecrows on fire. Burning lasts 3 seconds.",
        cost: 350,
        mods: Mods { special: Special::Burn, ..NO_MODS },
        look: Look::Glow,
    }],
];

const SMG: [&[Attachment]; 5] = [
    &[
        RED_DOT,
        Attachment {
            name: "Popcorn Scope",
            desc: "2.3x scope. Slower to raise.",
            cost: 220,
            mods: Mods { zoom: 2.3, aim_spread: 0.35, aim_speed: 0.6, ..NO_MODS },
            look: Look::Scope,
        },
    ],
    &[Attachment {
        name: "Compensator",
        desc: "45% less recoil and 25% tighter spread.",
        cost: 180,
        mods: Mods { kick: 0.55, spread: 0.75, ..NO_MODS },
        look: Look::Barrel,
    }],
    &[Attachment {
        name: "Drum Mag",
        desc: "Double the magazine. Reloads take 25% longer.",
        cost: 170,
        mods: Mods { mag: 2.0, reload: 1.25, ..NO_MODS },
        look: Look::Drum,
    }],
    &[Attachment {
        name: "Foregrip",
        desc: "40% less recoil, slightly tighter hip fire.",
        cost: 140,
        mods: Mods { kick: 0.6, spread: 0.85, ..NO_MODS },
        look: Look::Grip,
    }],
    &[Attachment {
        name: "Caramel Rounds",
        desc: "Hits gum scarecrows up: half speed for 2.5 seconds.",
        cost: 350,
        mods: Mods { special: Special::Caramel, ..NO_MODS },
        look: Look::Glow,
    }],
];

const LAUNCHER: [&[Attachment]; 5] = [
    &[
        Attachment { mods: Mods { zoom: 1.4, ..NO_MODS }, ..RED_DOT },
        Attachment {
            name: "Gourd Scope",
            desc: "2x scope for long lobs. Slower to raise.",
            cost: 220,
            mods: Mods { zoom: 2.0, aim_speed: 0.6, ..NO_MODS },
            look: Look::Scope,
        },
    ],
    &[Attachment {
        name: "Rifled Tube",
        desc: "Gourds fly 50% faster and the blast is 30% wider.",
        cost: 200,
        mods: Mods { velocity: 1.5, blast: 1.3, ..NO_MODS },
        look: Look::Barrel,
    }],
    &[Attachment {
        name: "Quad Tube",
        desc: "Holds four gourds. Reloading all four takes longer.",
        cost: 250,
        mods: Mods { mag: 4.0, reload: 1.5, ..NO_MODS },
        look: Look::Tubes,
    }],
    &[Attachment {
        name: "Stabilizer",
        desc: "Half the recoil.",
        cost: 120,
        mods: Mods { kick: 0.5, ..NO_MODS },
        look: Look::Grip,
    }],
    &[Attachment {
        name: "Cluster Gourds",
        desc: "Every blast throws out four bomblets that explode too.",
        cost: 400,
        mods: Mods { special: Special::Cluster, ..NO_MODS },
        look: Look::Glow,
    }],
];

const CATALOG: [[&[Attachment]; 5]; 4] = [PISTOL, SCATTERGUN, SMG, LAUNCHER];

pub fn options(weapon: usize, slot: usize) -> &'static [Attachment] {
    CATALOG[weapon][slot]
}

/// Cost to unlock each weapon in the shop. The pistol is free.
pub const WEAPON_COST: [u32; 4] = [0, 200, 350, 600];

/// Default aim-down-sights zoom with iron sights.
const IRON_ZOOM: f32 = 1.25;

/// What attachments you own and have on, per weapon.
#[derive(Clone, Debug, PartialEq)]
pub struct Loadout {
    pub owned_weapon: [bool; 4],
    /// Bitmask of owned options, per weapon per slot.
    pub owned: [[u8; 5]; 4],
    pub equipped: [[Option<u8>; 5]; 4],
}

impl Default for Loadout {
    fn default() -> Self {
        Self { owned_weapon: [true, false, false, false], owned: [[0; 5]; 4], equipped: [[None; 5]; 4] }
    }
}

impl Loadout {
    pub fn owns(&self, w: usize, slot: usize, opt: u8) -> bool {
        self.owned[w][slot] & (1 << opt) != 0
    }

    pub fn give(&mut self, w: usize, slot: usize, opt: u8) {
        self.owned[w][slot] |= 1 << opt;
    }

    /// Combined mods of everything equipped on weapon `w`, optionally pretending
    /// `slot` held `swap` instead (for the shop's preview).
    pub fn mods(&self, w: usize, swap: Option<(usize, Option<u8>)>) -> Mods {
        (0..SLOTS.len()).fold(NO_MODS, |acc, s| {
            let opt = match swap {
                Some((ss, o)) if ss == s => o,
                _ => self.equipped[w][s],
            };
            match opt {
                Some(o) => acc.and(options(w, s)[o as usize].mods),
                None => acc,
            }
        })
    }

    pub fn equipped_look(&self, w: usize, slot: usize) -> Option<Look> {
        self.equipped[w][slot].map(|o| options(w, slot)[o as usize].look)
    }
}

/// Everything the game needs to know about a weapon right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub damage: f32,
    pub pellets: u32,
    pub fire_rate: f32,
    pub hip_spread: f32,
    pub aim_spread: f32,
    pub kick: f32,
    pub mag: u32,
    pub reload: f32,
    pub zoom: f32,
    pub aim_speed: f32,
    pub velocity: f32,
    pub blast: f32,
    pub special: Special,
}

/// Base weapon stats, then attachments, then the global shop upgrades.
pub fn stats(w: usize, m: Mods, damage_mult: f32, reload_mult: f32, mag_mult: f32) -> Stats {
    let d = &WEAPONS[w];
    let hip = d.spread * m.spread;
    Stats {
        damage: d.damage * m.damage * damage_mult,
        pellets: d.pellets,
        fire_rate: 1.0 / d.cooldown,
        hip_spread: hip,
        // Aiming down sights tightens everything; sights tighten it more.
        aim_spread: hip * 0.35 * m.aim_spread,
        kick: d.kick * m.kick,
        mag: ((d.mag_size as f32 * m.mag * mag_mult).round() as u32).max(1),
        reload: d.reload_time * m.reload * reload_mult,
        zoom: if m.zoom > 0.0 { m.zoom } else { IRON_ZOOM },
        aim_speed: m.aim_speed,
        velocity: 24.0 * m.velocity,
        blast: 5.0 * m.blast,
        special: m.special,
    }
}

/// Readable stat lines for the shop, with "->" where a preview changes something.
pub fn describe(now: &Stats, preview: Option<&Stats>) -> String {
    let row = |label: &str, a: String, b: Option<String>| match b {
        Some(b) if b != a => format!("{label:<12}{a}  ->  {b}\n"),
        _ => format!("{label:<12}{a}\n"),
    };
    let p = preview;
    let deg = |s: f32| format!("{:.2} deg", s.to_degrees());
    let mut out = String::new();
    let dmg = |s: &Stats| {
        if s.pellets > 1 { format!("{:.2} x{}", s.damage, s.pellets) } else { format!("{:.2}", s.damage) }
    };
    out += &row("Damage", dmg(now), p.map(dmg));
    out += &row("Fire rate", format!("{:.1} / sec", now.fire_rate), p.map(|s| format!("{:.1} / sec", s.fire_rate)));
    out += &row("Hip spread", deg(now.hip_spread), p.map(|s| deg(s.hip_spread)));
    out += &row("Aim spread", deg(now.aim_spread), p.map(|s| deg(s.aim_spread)));
    out += &row("Recoil", format!("{:.0}", now.kick * 1000.0), p.map(|s| format!("{:.0}", s.kick * 1000.0)));
    out += &row("Magazine", now.mag.to_string(), p.map(|s| s.mag.to_string()));
    out += &row("Reload", format!("{:.2}s", now.reload), p.map(|s| format!("{:.2}s", s.reload)));
    out += &row("Aim zoom", format!("{:.2}x", now.zoom), p.map(|s| format!("{:.2}x", s.zoom)));
    let special = |s: &Stats| format!("{:?}", s.special);
    out += &row("Special", special(now), p.map(special));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_fits_in_a_bitmask() {
        for w in 0..4 {
            for s in 0..5 {
                let opts = options(w, s);
                assert!(!opts.is_empty() && opts.len() <= 8, "weapon {w} slot {s}");
            }
        }
    }

    #[test]
    fn attachments_stack_with_upgrades() {
        let mut l = Loadout::default();
        l.equipped[0][1] = Some(0); // pistol long barrel: +25% damage, 0.6 spread
        l.equipped[0][2] = Some(0); // extended mag: x1.5
        let s = stats(0, l.mods(0, None), 1.15, 1.0, 1.25); // plus one Sharper Kernels, one Deep Pockets
        assert!((s.damage - 1.0 * 1.25 * 1.15).abs() < 1e-5);
        assert_eq!(s.mag, (12.0f32 * 1.5 * 1.25).round() as u32);
        assert!((s.hip_spread - 0.004 * 0.6).abs() < 1e-6);
    }

    #[test]
    fn preview_swaps_one_slot_only() {
        let mut l = Loadout::default();
        l.equipped[2][3] = Some(0); // SMG foregrip
        let now = l.mods(2, None);
        let scoped = l.mods(2, Some((0, Some(1))));
        assert_eq!(now.zoom, 0.0);
        assert_eq!(scoped.zoom, 2.3);
        assert_eq!(scoped.kick, now.kick, "grip stays on during the preview");
        let removed = l.mods(2, Some((3, None)));
        assert_eq!(removed.kick, 1.0);
    }

    #[test]
    fn quad_tube_holds_four_gourds() {
        let mut l = Loadout::default();
        l.equipped[3][2] = Some(0);
        assert_eq!(stats(3, l.mods(3, None), 1.0, 1.0, 1.0).mag, 4);
    }

    #[test]
    fn specials_come_from_the_special_slot() {
        let mut l = Loadout::default();
        l.equipped[1][4] = Some(0);
        assert_eq!(l.mods(1, None).special, Special::Burn);
        assert_eq!(Loadout::default().mods(1, None).special, Special::None);
    }

    #[test]
    fn describe_marks_changes() {
        let l = Loadout::default();
        let now = stats(0, l.mods(0, None), 1.0, 1.0, 1.0);
        let with_mag = stats(0, l.mods(0, Some((2, Some(0)))), 1.0, 1.0, 1.0);
        let text = describe(&now, Some(&with_mag));
        assert!(text.contains("Magazine    12  ->  18"), "{text}");
        assert!(!text.contains("Damage      1.00  ->"), "unchanged lines have no arrow");
    }
}

//! The arsenal: four weapons, switching (1-4 or mouse wheel), magazines and reloading,
//! aiming down sights (hold right mouse), the pitchfork (F), candy grenades (G), the
//! on-screen weapon models and their attachments, muzzle flash, explosive gourds, and pickups.
//!
//! Final weapon numbers (damage, spread, magazine, ...) come from `loadout::stats`,
//! which folds in attachments and shop upgrades.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use rand::Rng;

use crate::collision::Obstacles;
use crate::effects::{self, Fx};
use crate::enemies::{self, Enemy};
use crate::loadout::{self, Loadout, Look, Special, Stats};
use crate::settings::Settings;
use crate::player::{Player, Shake};
use crate::powerups::{self, PowerUps};
use crate::sound::{self, Sfx};

pub struct WeaponDef {
    pub name: &'static str,
    pub damage: f32,
    pub cooldown: f32,
    pub pellets: u32,
    pub spread: f32,
    auto: bool,
    /// Rounds per magazine.
    pub mag_size: u32,
    /// Seconds to reload.
    pub reload_time: f32,
    /// Most spare rounds you can carry. 0 means unlimited.
    pub max_reserve: u32,
    start_reserve: u32,
    launcher: bool,
    pub kick: f32,
}

pub const WEAPONS: [WeaponDef; 4] = [
    WeaponDef { name: "Candy Corn Pistol", damage: 1.0, cooldown: 0.28, pellets: 1, spread: 0.004, auto: false, mag_size: 12, reload_time: 1.0, max_reserve: 0, start_reserve: 0, launcher: false, kick: 0.05 },
    WeaponDef { name: "Hayseed Scattergun", damage: 0.75, cooldown: 0.85, pellets: 9, spread: 0.09, auto: false, mag_size: 6, reload_time: 1.8, max_reserve: 36, start_reserve: 12, launcher: false, kick: 0.16 },
    WeaponDef { name: "Kettle Corn SMG", damage: 0.45, cooldown: 0.075, pellets: 1, spread: 0.028, auto: true, mag_size: 40, reload_time: 1.5, max_reserve: 280, start_reserve: 80, launcher: false, kick: 0.025 },
    WeaponDef { name: "Gourd Launcher", damage: 10.0, cooldown: 0.4, pellets: 1, spread: 0.0, auto: false, mag_size: 1, reload_time: 1.2, max_reserve: 10, start_reserve: 3, launcher: true, kick: 0.2 },
];

#[derive(Resource)]
pub struct Arsenal {
    pub current: usize,
    pub mag: [u32; 4],
    pub reserve: [u32; 4],
    /// Seconds left on the current weapon's reload, if reloading.
    pub reloading: Option<f32>,
    reload_total: f32,
    /// Upgrade multipliers (see `shop.rs`).
    pub damage_mult: f32,
    pub reload_mult: f32,
    pub mag_mult: f32,
    cooldown: f32,
    recoil: f32,
    /// Seconds left to show the hit marker; `true` = headshot.
    pub hit_marker: (f32, bool),
    melee_cd: f32,
    /// Seconds left in the pitchfork thrust animation.
    melee_anim: f32,
    /// Which weapons and attachments you own and have equipped.
    pub loadout: Loadout,
    /// 0 = hip fire, 1 = fully aimed down sights.
    pub aim: f32,
    /// Candy grenades carried (G to throw).
    pub grenades: u32,
    grenade_cd: f32,
}

pub const MAX_GRENADES: u32 = 5;
const START_GRENADES: u32 = 2;

impl Default for Arsenal {
    fn default() -> Self {
        Self {
            loadout: Loadout::default(),
            aim: 0.0,
            grenades: START_GRENADES,
            grenade_cd: 0.0,
            current: 0,
            mag: WEAPONS.map(|w| w.mag_size),
            reserve: WEAPONS.map(|w| w.start_reserve),
            reloading: None,
            reload_total: 1.0,
            damage_mult: 1.0,
            reload_mult: 1.0,
            mag_mult: 1.0,
            cooldown: 0.0,
            recoil: 0.0,
            hit_marker: (0.0, false),
            melee_cd: 0.0,
            melee_anim: 0.0,
        }
    }
}

impl Arsenal {
    /// Briefly stop firing, e.g. so the click on a menu button doesn't also shoot.
    pub fn block_fire(&mut self, secs: f32) {
        self.cooldown = self.cooldown.max(secs);
    }

    pub fn infinite(i: usize) -> bool {
        WEAPONS[i].max_reserve == 0
    }

    pub fn has_ammo(&self, i: usize) -> bool {
        self.loadout.owned_weapon[i] && (Self::infinite(i) || self.mag[i] > 0 || self.reserve[i] > 0)
    }

    /// Weapon `i`'s numbers with its attachments and the shop upgrades applied.
    pub fn stats(&self, i: usize) -> Stats {
        self.stats_preview(i, None)
    }

    /// Same, but as if `slot` held `opt` instead (for the shop preview).
    pub fn stats_preview(&self, i: usize, swap: Option<(usize, Option<u8>)>) -> Stats {
        loadout::stats(i, self.loadout.mods(i, swap), self.damage_mult, self.reload_mult, self.mag_mult)
    }

    /// Magazine size after attachments and upgrades.
    pub fn mag_size(&self, i: usize) -> u32 {
        self.stats(i).mag
    }

    /// Current zoom: 1 at the hip, up to the sight's zoom when fully aimed.
    pub fn zoom(&self) -> f32 {
        1.0 + (self.stats(self.current).zoom - 1.0) * self.aim
    }

    /// After attachments change: don't leave more in the magazine than it now holds.
    pub fn clamp_mags(&mut self) {
        for i in 0..WEAPONS.len() {
            let size = self.mag_size(i);
            if self.mag[i] > size {
                let extra = self.mag[i] - size;
                self.mag[i] = size;
                if !Self::infinite(i) {
                    self.reserve[i] = (self.reserve[i] + extra).min(WEAPONS[i].max_reserve);
                }
            }
        }
    }

    fn can_reload(&self, i: usize) -> bool {
        self.mag[i] < self.mag_size(i) && (Self::infinite(i) || self.reserve[i] > 0)
    }

    /// Switching weapons cancels a reload in progress. Locked weapons can't be selected.
    fn select(&mut self, i: usize) {
        if i != self.current && self.loadout.owned_weapon[i] {
            self.current = i;
            self.reloading = None;
        }
    }

    fn start_reload(&mut self, commands: &mut Commands, sfx: &Sfx) {
        if self.reloading.is_none() && self.can_reload(self.current) {
            self.reload_total = self.stats(self.current).reload;
            self.reloading = Some(self.reload_total);
            sound::play(commands, &sfx.reload_start, 0.7);
        }
    }

    fn finish_reload(&mut self) {
        let i = self.current;
        let need = self.mag_size(i).saturating_sub(self.mag[i]);
        let take = if Self::infinite(i) { need } else { need.min(self.reserve[i]) };
        self.mag[i] += take;
        if !Self::infinite(i) {
            self.reserve[i] -= take;
        }
    }
}

#[derive(Component)]
pub struct WeaponModel(usize);

#[derive(Component)]
pub struct MuzzleFlash;

#[derive(Component)]
pub struct PitchforkModel;

const MELEE_RANGE: f32 = 2.4;
const MELEE_DAMAGE: f32 = 2.5;
const MELEE_COOLDOWN: f32 = 0.55;
const MELEE_ANIM: f32 = 0.3;
const MELEE_MAX_TARGETS: usize = 3;
const PITCHFORK_POS: Vec3 = Vec3::new(0.18, -0.24, -0.25);

#[derive(Component)]
pub struct Gourd {
    vel: Vec3,
    life: f32,
    damage: f32,
    radius: f32,
    /// Throws out bomblets when it explodes (Cluster Gourds). Bomblets don't.
    cluster: bool,
}

/// One attachment's model on a gun. Shown when that attachment is equipped.
#[derive(Component)]
pub struct AttachmentVisual {
    weapon: usize,
    slot: usize,
    opt: u8,
}

const VIEW_POS: Vec3 = Vec3::new(0.22, -0.2, -0.42);
const VIEW_SCALE: f32 = 0.6;

/// Where attachments sit on each gun, in the gun's own space:
/// (top of the gun for sights, muzzle Z, under the front for grips, magazine spot).
const MOUNTS: [(f32, f32, Vec3, Vec3); 4] = [
    (0.045, -0.12, Vec3::new(0.0, -0.045, -0.07), Vec3::new(0.0, -0.17, 0.08)),
    (0.035, -0.37, Vec3::new(0.0, -0.075, -0.17), Vec3::new(0.0, -0.06, 0.0)),
    (0.055, -0.33, Vec3::new(0.0, -0.07, -0.13), Vec3::new(0.0, -0.11, -0.03)),
    (0.1, -0.36, Vec3::new(0.0, -0.1, -0.15), Vec3::new(0.0, 0.0, -0.05)),
];

/// Move aim toward `target` by `rate`, stopping exactly on it. Lowering is a bit
/// faster than raising.
fn aim_step(aim: f32, target: f32, rate: f32) -> f32 {
    if aim < target {
        (aim + rate).min(target)
    } else {
        (aim - rate * 1.5).max(target)
    }
}

/// Gun position when fully aimed: centered, with the sight line on the crosshair.
fn aim_pos(weapon: usize, has_sight: bool) -> Vec3 {
    let sight_top = MOUNTS[weapon].0 + if has_sight { 0.035 } else { 0.0 };
    Vec3::new(0.0, -sight_top * VIEW_SCALE - 0.012, -0.32)
}

// ---------------------------------------------------------------- viewmodels

/// Builds the four weapon models as children of the camera. Only the current one is visible.
pub fn spawn_viewmodels(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Single<Entity, With<Player>>,
) {
    let mut mat = |c: Color, e: LinearRgba| {
        materials.add(StandardMaterial { base_color: c, emissive: e, perceptual_roughness: 0.5, ..default() })
    };
    let metal = mat(Color::srgb(0.18, 0.18, 0.2), LinearRgba::BLACK);
    let wood = mat(Color::srgb(0.4, 0.24, 0.12), LinearRgba::BLACK);
    let candy_y = mat(Color::srgb(1.0, 0.85, 0.1), LinearRgba::rgb(0.4, 0.3, 0.0));
    let candy_o = mat(Color::srgb(1.0, 0.45, 0.05), LinearRgba::rgb(0.4, 0.12, 0.0));
    let green = mat(Color::srgb(0.2, 0.45, 0.15), LinearRgba::BLACK);
    let glow = mat(Color::BLACK, LinearRgba::rgb(0.2, 3.0, 0.4));
    let shine = mat(Color::srgb(0.75, 0.75, 0.8), LinearRgba::rgb(0.25, 0.25, 0.28));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cyl = meshes.add(Cylinder::new(0.5, 1.0));
    let tube = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

    // (offset, scale, rotation, material) per part, in weapon-local space; barrel points -Z.
    let part = |o: Vec3, s: Vec3, r: Quat, m: &Handle<StandardMaterial>, mesh: &Handle<Mesh>| {
        (Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::from_translation(o).with_rotation(r).with_scale(s))
    };

    let designs: [Vec<(Mesh3d, MeshMaterial3d<StandardMaterial>, Transform)>; 4] = [
        // Pistol: candy-corn striped slide
        vec![
            part(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.06, 0.07, 0.24), Quat::IDENTITY, &candy_y, &cube),
            part(Vec3::new(0.0, 0.02, -0.07), Vec3::new(0.062, 0.04, 0.08), Quat::IDENTITY, &candy_o, &cube),
            part(Vec3::new(0.0, -0.09, 0.07), Vec3::new(0.055, 0.14, 0.07), Quat::from_rotation_x(-0.25), &metal, &cube),
        ],
        // Scattergun: double barrel, wooden stock
        vec![
            part(Vec3::new(-0.025, 0.0, -0.12), Vec3::new(0.045, 0.5, 0.045), tube, &metal, &cyl),
            part(Vec3::new(0.025, 0.0, -0.12), Vec3::new(0.045, 0.5, 0.045), tube, &metal, &cyl),
            part(Vec3::new(0.0, -0.03, 0.18), Vec3::new(0.08, 0.1, 0.3), Quat::IDENTITY, &wood, &cube),
            part(Vec3::new(0.0, -0.05, -0.05), Vec3::new(0.07, 0.05, 0.18), Quat::IDENTITY, &wood, &cube),
        ],
        // SMG: boxy body, popcorn-bucket drum
        vec![
            part(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.08, 0.09, 0.32), Quat::IDENTITY, &metal, &cube),
            part(Vec3::new(0.0, 0.01, -0.24), Vec3::new(0.03, 0.18, 0.03), tube, &metal, &cyl),
            part(Vec3::new(0.0, -0.11, -0.03), Vec3::new(0.12, 0.1, 0.12), Quat::IDENTITY, &candy_y, &cyl),
            part(Vec3::new(0.0, -0.09, 0.12), Vec3::new(0.05, 0.12, 0.06), Quat::from_rotation_x(-0.3), &wood, &cube),
        ],
        // Launcher: fat green tube with a glowing sight
        vec![
            part(Vec3::new(0.0, 0.0, -0.05), Vec3::new(0.14, 0.55, 0.14), tube, &green, &cyl),
            part(Vec3::new(0.0, 0.0, -0.33), Vec3::new(0.17, 0.05, 0.17), tube, &metal, &cyl),
            part(Vec3::new(0.0, 0.09, 0.0), Vec3::new(0.03, 0.03, 0.06), Quat::IDENTITY, &glow, &cube),
            part(Vec3::new(0.0, -0.1, 0.08), Vec3::new(0.05, 0.12, 0.06), Quat::IDENTITY, &wood, &cube),
        ],
    ];

    let red = mat(Color::BLACK, LinearRgba::rgb(8.0, 0.3, 0.2));
    let lens = mat(Color::srgb(0.2, 0.5, 0.6), LinearRgba::rgb(0.1, 0.4, 0.6));
    let specials = [
        mat(Color::BLACK, LinearRgba::rgb(4.0, 3.0, 0.3)), // piercers: candy-corn yellow
        mat(Color::BLACK, LinearRgba::rgb(6.0, 1.0, 0.1)), // hot sauce: red-orange
        mat(Color::BLACK, LinearRgba::rgb(3.0, 1.6, 0.4)), // caramel: amber
        mat(Color::BLACK, LinearRgba::rgb(0.5, 4.0, 0.6)), // cluster: green
    ];

    commands.entity(*camera).with_children(|cam| {
        for (i, parts) in designs.into_iter().enumerate() {
            cam.spawn((
                WeaponModel(i),
                Transform::from_translation(VIEW_POS).with_scale(Vec3::splat(VIEW_SCALE)),
                if i == 0 { Visibility::Inherited } else { Visibility::Hidden },
            ))
            .with_children(|w| {
                for p in parts {
                    w.spawn(p);
                }
                // Every attachment this gun can take, hidden until equipped.
                let (top, muzzle, under, mag_at) = MOUNTS[i];
                for slot in 0..loadout::SLOTS.len() {
                    for (o, att) in loadout::options(i, slot).iter().enumerate() {
                        let pieces: Vec<(Vec3, Vec3, Quat, &Handle<StandardMaterial>, &Handle<Mesh>)> = match att.look {
                            Look::RedDot => vec![
                                (Vec3::new(0.0, top + 0.02, 0.0), Vec3::new(0.05, 0.04, 0.05), Quat::IDENTITY, &metal, &cube),
                                (Vec3::new(0.0, top + 0.045, 0.0), Vec3::new(0.012, 0.012, 0.01), Quat::IDENTITY, &red, &cube),
                            ],
                            Look::Scope => vec![
                                (Vec3::new(0.0, top + 0.035, 0.0), Vec3::new(0.045, 0.2, 0.045), tube, &metal, &cyl),
                                (Vec3::new(0.0, top + 0.035, -0.1), Vec3::new(0.06, 0.02, 0.06), tube, &lens, &cyl),
                                (Vec3::new(0.0, top + 0.01, 0.0), Vec3::new(0.02, 0.03, 0.06), Quat::IDENTITY, &metal, &cube),
                            ],
                            Look::Barrel => vec![(Vec3::new(0.0, 0.0, muzzle - 0.06), Vec3::new(0.04, 0.14, 0.04), tube, &metal, &cyl)],
                            Look::Choke => vec![(Vec3::new(0.0, 0.0, muzzle - 0.02), Vec3::new(0.12, 0.05, 0.08), Quat::IDENTITY, &shine, &cube)],
                            Look::Mag => vec![(mag_at + Vec3::new(0.0, -0.05, 0.0), Vec3::new(0.05, 0.12, 0.06), Quat::IDENTITY, &candy_o, &cube)],
                            Look::Drum => vec![(mag_at + Vec3::new(0.0, -0.04, 0.0), Vec3::new(0.17, 0.08, 0.17), Quat::IDENTITY, &candy_o, &cyl)],
                            Look::Tubes => [-1.0, 1.0]
                                .into_iter()
                                .map(|side| (mag_at + Vec3::new(0.11 * side, -0.02, 0.0), Vec3::new(0.08, 0.45, 0.08), tube, &green, &cyl))
                                .collect(),
                            Look::Grip => vec![(under + Vec3::new(0.0, -0.04, 0.0), Vec3::new(0.035, 0.1, 0.035), Quat::IDENTITY, &wood, &cyl)],
                            Look::Glow => vec![(Vec3::new(0.033, 0.0, muzzle * 0.4), Vec3::new(0.008, 0.02, muzzle.abs() * 0.7), Quat::IDENTITY, &specials[i], &cube)],
                        };
                        w.spawn((AttachmentVisual { weapon: i, slot, opt: o as u8 }, Transform::default(), Visibility::Hidden))
                            .with_children(|a| {
                                for (o, s, r, m, mesh) in pieces {
                                    a.spawn(part(o, s, r, m, mesh));
                                }
                            });
                    }
                }
            });
        }
        // Pitchfork: only shown during a thrust. Handle, crossbar, three tines; points -Z.
        cam.spawn((PitchforkModel, Transform::from_translation(PITCHFORK_POS), Visibility::Hidden))
            .with_children(|f| {
                f.spawn(part(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.04, 0.7, 0.04), tube, &wood, &cyl));
                f.spawn(part(Vec3::new(0.0, 0.0, -0.37), Vec3::new(0.3, 0.045, 0.045), Quat::IDENTITY, &shine, &cube));
                for x in [-0.13, 0.0, 0.13] {
                    f.spawn(part(Vec3::new(x, 0.0, -0.53), Vec3::new(0.03, 0.32, 0.03), tube, &shine, &cyl));
                }
            });
        cam.spawn((
            MuzzleFlash,
            PointLight { color: Color::srgb(1.0, 0.7, 0.3), intensity: 0.0, range: 12.0, ..default() },
            Transform::from_xyz(0.22, -0.15, -0.8),
        ));
    });
}

pub fn update_viewmodels(
    mut arsenal: ResMut<Arsenal>,
    mut models: Query<(&WeaponModel, &mut Transform, &mut Visibility), Without<PitchforkModel>>,
    mut fork: Single<(&mut Transform, &mut Visibility), With<PitchforkModel>>,
    mut parts: Query<(&AttachmentVisual, &mut Visibility), (Without<WeaponModel>, Without<PitchforkModel>)>,
    mut flash: Single<&mut PointLight, With<MuzzleFlash>>,
    mut camera: Single<(&Player, &mut Projection)>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<Settings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (player, ref mut projection) = *camera;
    arsenal.recoil = (arsenal.recoil - dt * 1.2).max(0.0);
    arsenal.hit_marker.0 = (arsenal.hit_marker.0 - dt).max(0.0);
    flash.intensity = (flash.intensity - dt * 2_000_000.0).max(0.0);

    // Gentle bob while walking
    let t = time.elapsed_secs();
    let bob = if player.moving { Vec3::new((t * 7.0).sin() * 0.012, (t * 14.0).sin().abs() * 0.012, 0.0) } else { Vec3::ZERO };

    // Dip the weapon down and tilt it while reloading.
    let dip = match arsenal.reloading {
        Some(left) => ((1.0 - left / arsenal.reload_total) * std::f32::consts::PI).sin(),
        None => 0.0,
    };

    // Pitchfork thrust: jab forward and back, gun hidden meanwhile.
    arsenal.melee_anim = (arsenal.melee_anim - dt).max(0.0);
    arsenal.melee_cd = (arsenal.melee_cd - dt).max(0.0);
    let stabbing = arsenal.melee_anim > 0.0;
    let (ref mut ftf, ref mut fvis) = *fork;
    **fvis = if stabbing { Visibility::Inherited } else { Visibility::Hidden };
    let thrust = ((1.0 - arsenal.melee_anim / MELEE_ANIM) * std::f32::consts::PI).sin();
    ftf.translation = PITCHFORK_POS + Vec3::new(-0.08 * thrust, 0.06 * thrust, -0.3 * thrust);
    ftf.rotation = Quat::from_rotation_y(0.12 * (1.0 - thrust));

    // Aim down sights: hold right mouse. Not while reloading or stabbing.
    let stats = arsenal.stats(arsenal.current);
    let wants_aim = mouse.pressed(MouseButton::Right)
        && cursor.grab_mode != CursorGrabMode::None
        && arsenal.reloading.is_none()
        && !stabbing;
    let target = if wants_aim { 1.0 } else { 0.0 };
    let rate = 7.0 * stats.aim_speed * dt;
    arsenal.aim = aim_step(arsenal.aim, target, rate);
    let aim = arsenal.aim;

    // Zoom by narrowing the field of view.
    if let Projection::Perspective(persp) = &mut **projection {
        let base = settings.fov.to_radians();
        persp.fov = 2.0 * ((base * 0.5).tan() / arsenal.zoom()).atan();
    }

    // Behind a scope you see the scope overlay instead of the gun.
    let sight = arsenal.loadout.equipped_look(arsenal.current, 0);
    let scoped_in = sight == Some(Look::Scope) && aim > 0.85;

    let ads = aim_pos(arsenal.current, sight.is_some());
    let bob = bob * (1.0 - aim * 0.8);
    for (m, mut tf, mut vis) in &mut models {
        let shown = m.0 == arsenal.current && !stabbing && !scoped_in;
        *vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
        let pos = VIEW_POS.lerp(ads, aim);
        tf.translation = pos + bob + Vec3::new(0.0, -dip * 0.18, arsenal.recoil * 0.35 * (1.0 - aim * 0.5));
        tf.rotation = Quat::from_rotation_x(arsenal.recoil * 2.0 * (1.0 - aim * 0.6) - dip * 0.7) * Quat::from_rotation_z(dip * 0.5);
    }
    for (part, mut vis) in &mut parts {
        let on = arsenal.loadout.equipped[part.weapon][part.slot] == Some(part.opt);
        *vis = if on { Visibility::Inherited } else { Visibility::Hidden };
    }
}

// ---------------------------------------------------------------- switching & firing

pub fn switch_weapon(
    mut arsenal: ResMut<Arsenal>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let keys_map = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    for (i, k) in keys_map.iter().enumerate() {
        if keys.just_pressed(*k) {
            arsenal.select(i);
        }
    }
    if scroll.delta.y != 0.0 && arsenal.aim < 0.5 {
        let step = if scroll.delta.y < 0.0 { 1 } else { WEAPONS.len() - 1 };
        // Skip empty weapons when scrolling.
        let mut next = arsenal.current;
        for _ in 0..WEAPONS.len() {
            next = (next + step) % WEAPONS.len();
            if arsenal.has_ammo(next) {
                break;
            }
        }
        arsenal.select(next);
    }
}

/// R to reload; reloads finish after the weapon's reload time.
pub fn reload(
    mut commands: Commands,
    mut arsenal: ResMut<Arsenal>,
    keys: Res<ButtonInput<KeyCode>>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        arsenal.start_reload(&mut commands, &sfx);
    }
    if let Some(left) = arsenal.reloading {
        let left = left - time.delta_secs();
        if left <= 0.0 {
            arsenal.finish_reload();
            arsenal.reloading = None;
            sound::play(&mut commands, &sfx.reload_end, 0.8);
        } else {
            arsenal.reloading = Some(left);
        }
    }
}

pub fn fire(
    mut commands: Commands,
    mut arsenal: ResMut<Arsenal>,
    mut player: Single<(&mut Player, &Transform)>,
    mut flash: Single<&mut PointLight, With<MuzzleFlash>>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform)>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    obstacles: Res<Obstacles>,
    power: Res<PowerUps>,
    mut shake: ResMut<Shake>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    arsenal.cooldown -= time.delta_secs();
    if cursor.grab_mode == CursorGrabMode::None || time.delta_secs() == 0.0 {
        return;
    }
    let w = &WEAPONS[arsenal.current];
    let st = arsenal.stats(arsenal.current);
    let trigger = if w.auto { mouse.pressed(MouseButton::Left) } else { mouse.just_pressed(MouseButton::Left) };
    if !trigger || arsenal.cooldown > 0.0 || arsenal.reloading.is_some() || arsenal.melee_anim > 0.0 {
        return;
    }
    let i = arsenal.current;
    if arsenal.mag[i] == 0 {
        if arsenal.can_reload(i) {
            arsenal.start_reload(&mut commands, &sfx);
        } else {
            // Completely out: click, then fall back to the pistol.
            sound::play(&mut commands, &sfx.empty, 0.8);
            arsenal.select(0);
        }
        return;
    }
    arsenal.mag[i] -= 1;
    arsenal.cooldown = w.cooldown;
    // Aiming halves the kick on top of whatever attachments do.
    let kick = st.kick * (1.0 - 0.5 * arsenal.aim);
    arsenal.recoil = (arsenal.recoil + kick).min(0.3);
    flash.intensity = 300_000.0;
    let volume = if w.auto { 0.45 } else { 0.8 };
    sound::play(&mut commands, &sfx.shots[i], volume);
    // A little kick on the heavy weapons.
    if w.pellets > 1 || w.launcher {
        shake.add(0.18);
    }

    let (ref mut p, tf) = *player;
    p.pitch = (p.pitch + kick * 0.15).min(1.45);
    let origin = tf.translation;
    let forward = tf.forward().as_vec3();
    let muzzle = origin + tf.rotation * Vec3::new(0.22, -0.17, -0.6);

    if w.launcher {
        commands.spawn((
            Gourd {
                vel: forward * st.velocity + Vec3::Y * 3.0,
                life: 4.0,
                damage: st.damage * power.damage_mult(),
                radius: st.blast,
                cluster: st.special == Special::Cluster,
            },
            Mesh3d(fx.ball.clone()),
            MeshMaterial3d(fx.pumpkin.clone()),
            Transform::from_translation(muzzle).with_scale(Vec3::new(0.16, 0.13, 0.16)),
        ));
        return;
    }

    let right = tf.right().as_vec3();
    let up = tf.up().as_vec3();
    let mut rng = rand::rng();
    let mut hit_sound: Option<bool> = None;
    // Aiming down sights blends from hip spread to the tighter aimed spread.
    let spread = st.hip_spread + (st.aim_spread - st.hip_spread) * arsenal.aim;
    // Piercing rounds go through up to three scarecrows; everything else stops at one.
    let max_targets = if st.special == Special::Pierce { 3 } else { 1 };
    for _ in 0..st.pellets {
        let dir = (forward
            + right * rng.random_range(-1.0..1.0) * spread
            + up * rng.random_range(-1.0..1.0) * spread)
            .normalize();
        // Props block bullets: only count enemy hits in front of any cover.
        let wall = obstacles.raycast(origin, dir, 60.0);
        let hits: Vec<(Entity, f32, bool)> = enemies::raycast_all(origin, dir, enemies.iter().map(|(e, en, t)| (e, t, en.kind)))
            .into_iter()
            .filter(|&(_, d, _)| wall.is_none_or(|w| d < w))
            .take(max_targets)
            .collect();
        match hits.last().copied() {
            Some((_, last_dist, _)) => {
                let point = origin + dir * last_dist;
                effects::tracer(&mut commands, &fx, muzzle, point);
                for &(e, dist, head) in &hits {
                    effects::burst(&mut commands, &fx, origin + dir * dist, 2, &[fx.pumpkin.clone()], 2.5);
                    if let Ok((_, mut enemy, etf)) = enemies.get_mut(e) {
                        let dmg = st.damage * power.damage_mult();
                        if head {
                            enemy.hurt_head(dmg * 2.0);
                        } else {
                            enemy.hurt(dmg);
                        }
                        // Scattergun pellets shove targets back a little.
                        if st.pellets > 1 {
                            enemy.push(dir.xz(), 1.4, etf.scale.x);
                        }
                        match st.special {
                            Special::Burn => enemy.ignite(3.0, 0.9 * arsenal.damage_mult * power.damage_mult()),
                            Special::Caramel => enemy.slow = 2.5,
                            _ => {}
                        }
                    }
                    arsenal.hit_marker = (0.12, head);
                    hit_sound = Some(hit_sound.unwrap_or(false) || head);
                }
            }
            None => {
                // Tracer ends at cover, the ground, or 60m out.
                let mut t = wall.unwrap_or(60.0);
                if dir.y < -0.001 {
                    t = t.min(-origin.y / dir.y);
                }
                let end = origin + dir * t;
                effects::tracer(&mut commands, &fx, muzzle, end);
                if wall.is_some_and(|w| w <= t) {
                    effects::burst(&mut commands, &fx, end, 2, &[fx.straw.clone()], 2.0);
                }
            }
        }
    }
    match hit_sound {
        Some(true) => sound::play(&mut commands, &sfx.headshot, 0.6),
        Some(false) => sound::play(&mut commands, &sfx.hit, 0.5),
        None => {}
    }
}

pub fn update_gourds(
    mut commands: Commands,
    mut gourds: Query<(Entity, &mut Gourd, &mut Transform), (Without<Enemy>, Without<Player>)>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut arsenal: ResMut<Arsenal>,
    obstacles: Res<Obstacles>,
    player: Single<&Transform, With<Player>>,
    mut shake: ResMut<Shake>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut g, mut tf) in &mut gourds {
        g.vel.y -= 12.0 * dt;
        g.life -= dt;
        tf.translation += g.vel * dt;
        tf.rotate_x(8.0 * dt);

        let touching = enemies
            .iter()
            .any(|(en, etf)| enemies::hit_spheres(etf, en.kind).any(|(c, r, _)| c.distance(tf.translation) < r + 0.15));
        if !(touching || tf.translation.y < 0.1 || g.life <= 0.0 || obstacles.contains(tf.translation, 0.1)) {
            continue;
        }

        let at = tf.translation;
        let big = g.cluster || g.radius > 3.0;
        let chunks = [fx.pumpkin.clone(), fx.seed.clone(), fx.straw.clone()];
        if explode(&mut commands, &fx, &sfx, &mut shake, &mut enemies, at, g.radius, g.damage, big, &chunks, player.translation) {
            arsenal.hit_marker = (0.15, false);
        }

        // Cluster Gourds: four bomblets pop out and land around the blast.
        if g.cluster {
            let mut rng = rand::rng();
            for k in 0..4 {
                let a = k as f32 / 4.0 * std::f32::consts::TAU + rng.random_range(-0.4..0.4);
                let vel = Vec3::new(a.sin() * 4.5, 6.0, a.cos() * 4.5);
                commands.spawn((
                    Gourd { vel, life: 2.0, damage: g.damage * 0.4, radius: 2.5, cluster: false },
                    Mesh3d(fx.ball.clone()),
                    MeshMaterial3d(fx.stem.clone()),
                    Transform::from_translation(at + Vec3::Y * 0.5).with_scale(Vec3::splat(0.09)),
                ));
            }
        }
        commands.entity(e).despawn();
    }
}

/// Blast damage around `at`, falling off with distance, plus the boom, flash, and shake.
/// Shared by gourds, bomblets, and candy grenades. Returns whether it hit anything.
fn explode(
    commands: &mut Commands,
    fx: &Fx,
    sfx: &Sfx,
    shake: &mut Shake,
    enemies: &mut Query<(&mut Enemy, &Transform)>,
    at: Vec3,
    radius: f32,
    damage: f32,
    big: bool,
    chunks: &[Handle<StandardMaterial>],
    listener: Vec3,
) -> bool {
    let mut hit_any = false;
    for (mut enemy, etf) in enemies.iter_mut() {
        let d = (etf.translation + Vec3::Y * etf.scale.y).distance(at);
        if d < radius {
            enemy.hurt(damage * (1.0 - d / radius).max(0.25));
            hit_any = true;
        }
    }
    effects::burst(commands, fx, at, if big { 30 } else { 12 }, chunks, if big { 8.0 } else { 5.0 });
    effects::light_flash(commands, at + Vec3::Y, Color::srgb(1.0, 0.55, 0.15), if big { 2_000_000.0 } else { 600_000.0 }, 18.0, 0.4);
    sound::play_with_speed(commands, &sfx.explosion, if big { 1.0 } else { 0.5 }, if big { 1.0 } else { 1.5 });
    shake.add_at(if big { 0.6 } else { 0.2 }, at, listener);
    hit_any
}

// ---------------------------------------------------------------- candy grenades

const GRENADE_FUSE: f32 = 1.8;
const GRENADE_RADIUS: f32 = 4.5;
const GRENADE_DAMAGE: f32 = 6.0;

/// A thrown candy grenade: bounces, then pops after its fuse runs out.
#[derive(Component)]
pub struct Grenade {
    vel: Vec3,
    fuse: f32,
    damage: f32,
}

/// G: lob a candy grenade in an arc. It bounces off the ground and props and pops
/// after 1.8 seconds, damaging everything within 4.5m. No damage to you.
pub fn throw_grenade(
    mut commands: Commands,
    mut arsenal: ResMut<Arsenal>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    player: Single<&Transform, With<Player>>,
    power: Res<PowerUps>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    arsenal.grenade_cd = (arsenal.grenade_cd - time.delta_secs()).max(0.0);
    if !keys.just_pressed(KeyCode::KeyG) || cursor.grab_mode == CursorGrabMode::None || arsenal.grenade_cd > 0.0 {
        return;
    }
    if arsenal.grenades == 0 {
        sound::play(&mut commands, &sfx.empty, 0.8);
        return;
    }
    arsenal.grenades -= 1;
    arsenal.grenade_cd = 0.6;
    let tf = *player;
    let from = tf.translation + tf.rotation * Vec3::new(-0.25, -0.2, -0.5);
    let vel = tf.forward().as_vec3() * 15.0 + Vec3::Y * 4.0;
    commands
        .spawn((
            Grenade { vel, fuse: GRENADE_FUSE, damage: GRENADE_DAMAGE * arsenal.damage_mult * power.damage_mult() },
            Transform::from_translation(from),
            Visibility::default(),
        ))
        .with_children(|g| {
            // A little wrapped candy with a glowing fuse spark.
            g.spawn((Mesh3d(fx.cube.clone()), MeshMaterial3d(fx.candy.clone()), Transform::from_scale(Vec3::new(0.2, 0.11, 0.11))));
            for dx in [-0.13, 0.13] {
                g.spawn((
                    Mesh3d(fx.cube.clone()),
                    MeshMaterial3d(fx.pumpkin.clone()),
                    Transform::from_xyz(dx, 0.0, 0.0).with_scale(Vec3::new(0.06, 0.13, 0.13)),
                ));
            }
            g.spawn((Mesh3d(fx.ball.clone()), MeshMaterial3d(fx.seed.clone()), Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::splat(0.035))));
        });
    sound::play_with_speed(&mut commands, &sfx.throw, 0.7, 1.4);
}

pub fn update_grenades(
    mut commands: Commands,
    mut grenades: Query<(Entity, &mut Grenade, &mut Transform), (Without<Enemy>, Without<Player>)>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut arsenal: ResMut<Arsenal>,
    mut shake: ResMut<Shake>,
    obstacles: Res<Obstacles>,
    player: Single<&Transform, With<Player>>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut g, mut tf) in &mut grenades {
        g.fuse -= dt;
        g.vel.y -= 18.0 * dt;
        let next = tf.translation + g.vel * dt;
        if obstacles.contains(next, 0.08) {
            // Bounce off props: reverse the sideways motion, lose some energy.
            g.vel.x *= -0.45;
            g.vel.z *= -0.45;
            g.vel.y *= 0.6;
        } else {
            tf.translation = next;
        }
        if tf.translation.y < 0.1 {
            tf.translation.y = 0.1;
            g.vel.y = -g.vel.y * 0.4;
            g.vel.x *= 0.6;
            g.vel.z *= 0.6;
        }
        tf.rotate_x(9.0 * dt * g.vel.length().min(10.0) / 10.0);
        if g.fuse > 0.0 {
            continue;
        }
        let chunks = [fx.candy.clone(), fx.pumpkin.clone(), fx.seed.clone()];
        if explode(&mut commands, &fx, &sfx, &mut shake, &mut enemies, tf.translation, GRENADE_RADIUS, g.damage, true, &chunks, player.translation) {
            arsenal.hit_marker = (0.15, false);
        }
        commands.entity(e).despawn();
    }
}

// ---------------------------------------------------------------- pitchfork

/// F: jab with the pitchfork. Hits up to three enemies in front of you
/// and knocks them back. Always available, no ammo needed.
pub fn melee(
    mut commands: Commands,
    mut arsenal: ResMut<Arsenal>,
    mut enemies: Query<(&mut Enemy, &Transform), Without<Player>>,
    player: Single<&Transform, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    power: Res<PowerUps>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let pressed = keys.just_pressed(KeyCode::KeyF);
    if !pressed || arsenal.melee_cd > 0.0 || cursor.grab_mode == CursorGrabMode::None || time.delta_secs() == 0.0 {
        return;
    }
    arsenal.melee_cd = MELEE_COOLDOWN;
    arsenal.melee_anim = MELEE_ANIM;
    sound::play_with_speed(&mut commands, &sfx.throw, 0.6, 1.8);

    let origin = player.translation.xz();
    let forward = player.forward().as_vec3().xz().normalize_or_zero();
    let mut targets: Vec<(f32, Mut<Enemy>, &Transform)> = enemies
        .iter_mut()
        .filter_map(|(enemy, tf)| {
            let to = tf.translation.xz() - origin;
            let reach = to.length() - 0.45 * tf.scale.x;
            // Bats flying overhead are out of reach; a grounded (stunned) bat isn't.
            let low_enough = tf.translation.y < player.translation.y + 0.3;
            (reach < MELEE_RANGE && low_enough && to.normalize_or_zero().dot(forward) > 0.45).then_some((reach, enemy, tf))
        })
        .collect();
    targets.sort_by(|a, b| a.0.total_cmp(&b.0));

    let dmg = MELEE_DAMAGE * arsenal.damage_mult * power.damage_mult();
    let mut hit = false;
    for (_, mut enemy, tf) in targets.into_iter().take(MELEE_MAX_TARGETS) {
        enemy.hurt(dmg);
        enemy.push(tf.translation.xz() - origin, 9.0, tf.scale.x);
        effects::burst(&mut commands, &fx, tf.translation + Vec3::Y * 1.2 * tf.scale.y, 5, &[fx.straw.clone(), fx.pumpkin.clone()], 3.0);
        hit = true;
    }
    if hit {
        arsenal.hit_marker = (0.15, false);
        sound::play_with_speed(&mut commands, &sfx.splat, 0.7, 1.3);
    }
}

// ---------------------------------------------------------------- pickups

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// Wrapped candy: restores health.
    Health,
    /// Crate: tops up the ammo weapons.
    Ammo,
    /// Red orb (see `powerups.rs`).
    DoubleDamage,
    /// Blue orb.
    Speed,
    /// Gold orb.
    Magnet,
}

impl PickupKind {
    pub fn random_power_up() -> Self {
        [PickupKind::DoubleDamage, PickupKind::Speed, PickupKind::Magnet][rand::rng().random_range(0..3)]
    }
}

#[derive(Component)]
pub struct Pickup {
    kind: PickupKind,
    life: f32,
}

pub fn spawn_pickup(commands: &mut Commands, fx: &Fx, kind: PickupKind, at: Vec3) {
    let mut root = commands.spawn((Pickup { kind, life: 25.0 }, Transform::from_translation(at + Vec3::Y * 0.5), Visibility::default()));
    match kind {
        PickupKind::Health => {
            // Wrapped candy: red middle, orange twisted ends.
            root.with_children(|p| {
                p.spawn((Mesh3d(fx.cube.clone()), MeshMaterial3d(fx.candy.clone()), Transform::from_scale(Vec3::new(0.45, 0.25, 0.25))));
                for dx in [-0.3, 0.3] {
                    p.spawn((
                        Mesh3d(fx.cube.clone()),
                        MeshMaterial3d(fx.pumpkin.clone()),
                        Transform::from_xyz(dx, 0.0, 0.0).with_scale(Vec3::new(0.15, 0.3, 0.3)),
                    ));
                }
            });
        }
        PickupKind::Ammo => {
            // Straw-colored crate with a green band.
            root.with_children(|p| {
                p.spawn((Mesh3d(fx.cube.clone()), MeshMaterial3d(fx.straw.clone()), Transform::from_scale(Vec3::new(0.5, 0.35, 0.35))));
                p.spawn((Mesh3d(fx.cube.clone()), MeshMaterial3d(fx.stem.clone()), Transform::from_scale(Vec3::new(0.52, 0.1, 0.37))));
            });
        }
        PickupKind::DoubleDamage | PickupKind::Speed | PickupKind::Magnet => {
            // Glowing orb with its own light so you can spot it across the field.
            let (mat, color) = match kind {
                PickupKind::DoubleDamage => (fx.power_damage.clone(), Color::srgb(1.0, 0.2, 0.15)),
                PickupKind::Speed => (fx.power_speed.clone(), Color::srgb(0.2, 0.6, 1.0)),
                _ => (fx.power_magnet.clone(), Color::srgb(1.0, 0.8, 0.2)),
            };
            root.insert(PointLight { color, intensity: 60_000.0, range: 6.0, ..default() });
            root.with_children(|p| {
                p.spawn((Mesh3d(fx.ball.clone()), MeshMaterial3d(mat), Transform::from_scale(Vec3::splat(0.28))));
                p.spawn((
                    Mesh3d(fx.ring.clone()),
                    MeshMaterial3d(fx.tracer.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(1.2)).with_scale(Vec3::new(0.42, 0.4, 0.42)),
                ));
            });
        }
    }
}

pub fn update_pickups(
    mut commands: Commands,
    mut pickups: Query<(Entity, &mut Pickup, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut arsenal: ResMut<Arsenal>,
    mut power: ResMut<PowerUps>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let (ref mut p, ptf) = *player;
    for (e, mut pickup, mut tf) in &mut pickups {
        pickup.life -= dt;
        tf.rotate_y(2.0 * dt);
        tf.translation.y = 0.5 + (t * 3.0).sin() * 0.12;
        if pickup.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if tf.translation.xz().distance(ptf.translation.xz()) > 1.4 {
            continue;
        }
        let mut pitch = 1.0;
        match pickup.kind {
            PickupKind::Health => {
                if p.health >= p.max_health {
                    continue;
                }
                p.health = (p.health + 25).min(p.max_health);
            }
            PickupKind::Ammo => {
                for (i, w) in WEAPONS.iter().enumerate() {
                    if w.max_reserve > 0 && arsenal.loadout.owned_weapon[i] {
                        let gain = (w.max_reserve / 4).max(2);
                        arsenal.reserve[i] = (arsenal.reserve[i] + gain).min(w.max_reserve);
                    }
                }
                arsenal.grenades = (arsenal.grenades + 1).min(MAX_GRENADES);
            }
            PickupKind::DoubleDamage => {
                power.damage = powerups::DURATION;
                pitch = 1.4;
            }
            PickupKind::Speed => {
                power.speed = powerups::DURATION;
                pitch = 1.6;
            }
            PickupKind::Magnet => {
                power.magnet = powerups::DURATION;
                pitch = 1.25;
            }
        }
        sound::play_with_speed(&mut commands, &sfx.pickup, 0.7, pitch);
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_pulls_from_reserve() {
        let mut a = Arsenal::default();
        a.loadout.owned_weapon[1] = true;
        a.current = 1; // scattergun: 6-round mag, 12 in reserve
        a.mag[1] = 2;
        a.finish_reload();
        assert_eq!(a.mag[1], 6);
        assert_eq!(a.reserve[1], 8);
    }

    #[test]
    fn partial_reload_when_reserve_is_low() {
        let mut a = Arsenal::default();
        a.current = 2;
        a.mag[2] = 0;
        a.reserve[2] = 15;
        a.finish_reload();
        assert_eq!((a.mag[2], a.reserve[2]), (15, 0));
        assert!(!a.can_reload(2));
    }

    #[test]
    fn pistol_never_runs_dry() {
        let mut a = Arsenal::default();
        a.mag[0] = 0;
        assert!(a.can_reload(0));
        a.finish_reload();
        assert_eq!(a.mag[0], a.mag_size(0));
        assert!(a.has_ammo(0));
    }

    #[test]
    fn aim_holds_steady_once_raised() {
        // Even with big steps (a slow frame), aim must settle on the target, not bounce.
        let mut aim = 0.0;
        for _ in 0..10 {
            aim = aim_step(aim, 1.0, 0.7);
        }
        assert_eq!(aim, 1.0);
        assert_eq!(aim_step(1.0, 1.0, 0.7), 1.0);
        assert_eq!(aim_step(0.0, 0.0, 0.7), 0.0);
        assert_eq!(aim_step(1.0, 0.0, 0.7), 0.0);
    }

    #[test]
    fn locked_weapons_cant_be_selected() {
        let mut a = Arsenal::default();
        a.select(2);
        assert_eq!(a.current, 0);
        a.loadout.owned_weapon[2] = true;
        a.select(2);
        assert_eq!(a.current, 2);
    }

    #[test]
    fn removing_an_extended_mag_returns_the_extra_rounds() {
        let mut a = Arsenal::default();
        a.loadout.owned_weapon[2] = true;
        a.loadout.equipped[2][2] = Some(0); // drum mag: 80
        a.mag[2] = 80;
        a.reserve[2] = 10;
        a.loadout.equipped[2][2] = None;
        a.clamp_mags();
        assert_eq!((a.mag[2], a.reserve[2]), (40, 50));
    }

    #[test]
    fn switching_cancels_reload() {
        let mut a = Arsenal::default();
        a.loadout.owned_weapon[2] = true;
        a.reloading = Some(0.5);
        a.select(2);
        assert!(a.reloading.is_none());
    }
}

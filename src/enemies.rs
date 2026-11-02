//! Pumpkin-headed scarecrows: four enemy types, a wave spawner, walking animation,
//! melee and thrown-seed attacks, and death/drops.
//!
//! Each enemy is a root entity (with `Enemy`) whose children are the body parts.
//! Arms and legs are `Limb` pivots placed at the shoulder/hip so they swing naturally.

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::bat::{self, Bat};
use crate::boss::{self, Golem, King, Witch};
use crate::collision::Obstacles;
use crate::effects::{self, Fx};
use crate::sound::{self, Sfx};
use crate::festival::ARENA_HALF;
use crate::looks;
use crate::pathfinding::NavGrid;
use crate::settings::Settings;
use crate::player::{HurtFlash, Player, Shake};
use crate::weapons::{PickupKind, spawn_pickup};
use crate::Score;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Standard shambler.
    Walker,
    /// Small, fast, fragile.
    Runner,
    /// Huge, slow, hits hard, soaks bullets.
    Brute,
    /// Keeps its distance and lobs flaming seeds.
    Lobber,
    /// Flying pumpkin with bat wings. Circles overhead, then dive-bombs. See `bat.rs`.
    Bat,
    /// Boss, wave 5 (and every 15 after). See `boss.rs`.
    King,
    /// Boss, wave 10 (and every 15 after). Floats, teleports, fires homing wisps.
    Witch,
    /// Boss, wave 15 (and every 15 after). Charges, crashes, splits into Gourdlings.
    Golem,
    /// Half of a Gourd Golem after it splits.
    Gourdling,
}

pub fn is_boss(kind: Kind) -> bool {
    matches!(kind, Kind::King | Kind::Witch | Kind::Golem | Kind::Gourdling)
}

pub fn boss_name(kind: Kind) -> &'static str {
    match kind {
        Kind::King => "THE PUMPKIN KING",
        Kind::Witch => "THE SCARECROW WITCH",
        Kind::Golem | Kind::Gourdling => "THE GOURD GOLEM",
        _ => "",
    }
}

pub struct Stats {
    pub health: f32,
    speed: f32,
    scale: f32,
    pub damage: i32,
    points: u32,
    shirt: Color,
}

impl Kind {
    pub fn stats(self) -> Stats {
        match self {
            Kind::Walker => Stats { health: 3.0, speed: 2.2, scale: 1.0, damage: 7, points: 10, shirt: Color::srgb(0.55, 0.15, 0.12) },
            Kind::Runner => Stats { health: 1.8, speed: 5.0, scale: 0.8, damage: 5, points: 15, shirt: Color::srgb(0.15, 0.40, 0.15) },
            Kind::Brute => Stats { health: 16.0, speed: 1.5, scale: 1.55, damage: 25, points: 50, shirt: Color::srgb(0.25, 0.20, 0.18) },
            Kind::Lobber => Stats { health: 4.0, speed: 2.0, scale: 1.0, damage: 8, points: 25, shirt: Color::srgb(0.35, 0.15, 0.45) },
            Kind::Bat => Stats { health: 1.6, speed: 7.0, scale: 0.75, damage: 9, points: 20, shirt: Color::srgb(0.32, 0.12, 0.4) },
            Kind::King => Stats { health: 150.0, speed: 1.7, scale: 2.8, damage: 30, points: 500, shirt: Color::srgb(0.32, 0.05, 0.40) },
            Kind::Witch => Stats { health: 130.0, speed: 2.4, scale: 1.7, damage: 15, points: 600, shirt: Color::srgb(0.08, 0.06, 0.1) },
            Kind::Golem => Stats { health: 220.0, speed: 1.4, scale: 3.0, damage: 35, points: 400, shirt: Color::srgb(0.18, 0.28, 0.12) },
            Kind::Gourdling => Stats { health: 60.0, speed: 2.2, scale: 1.7, damage: 18, points: 150, shirt: Color::srgb(0.18, 0.28, 0.12) },
        }
    }
}

#[derive(Component)]
pub struct Enemy {
    pub kind: Kind,
    pub health: f32,
    pub max_health: f32,
    pub flash: f32,
    speed: f32,
    pub damage: i32,
    points: u32,
    pub walk_t: f32,
    attack_cd: f32,
    groan_cd: f32,
    pub swing: f32,
    /// Set by boss behavior to hold an enemy in place (winding up, charging, stunned).
    pub frozen: bool,
    /// Damage taken is multiplied by this (a stunned Golem takes extra).
    pub vulnerable: f32,
    /// Shown next to a boss's name on its health bar, e.g. "STUNNED".
    pub status: &'static str,
    /// Knockback velocity on the ground plane, decays quickly.
    pub knock: Vec2,
    /// Where pathfinding is currently steering this enemy (ground x/z).
    pub waypoint: Vec2,
    /// Seconds left on fire (Hot Sauce Shells), and damage per second while burning.
    pub burn: f32,
    burn_dps: f32,
    /// Seconds left gummed up by Caramel Rounds (half speed).
    pub slow: f32,
    /// Damage taken this frame, turned into a floating number by `hud.rs`.
    pub recent: f32,
    pub recent_crit: bool,
    /// Seconds since this enemy was last hit (the health bar hides after a while).
    pub since_hit: f32,
    /// The floating health bar above this enemy, once it's been hurt.
    pub bar: Option<Entity>,
    face: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    shirt: Handle<StandardMaterial>,
}

impl Enemy {
    pub fn hurt(&mut self, dmg: f32) {
        let dealt = dmg * self.vulnerable;
        self.health -= dealt;
        self.flash = 0.08;
        self.recent += dealt;
        self.since_hit = 0.0;
    }

    /// A headshot: same as `hurt`, but the damage number shows as a crit.
    pub fn hurt_head(&mut self, dmg: f32) {
        self.hurt(dmg);
        self.recent_crit = true;
    }

    /// Set on fire for `secs`. Re-igniting refreshes the timer; the stronger burn wins.
    pub fn ignite(&mut self, secs: f32, dps: f32) {
        self.burn = self.burn.max(secs);
        self.burn_dps = self.burn_dps.max(dps);
    }

    /// Shove the enemy away along `dir`. Big enemies barely budge.
    pub fn push(&mut self, dir: Vec2, strength: f32, scale: f32) {
        self.knock += dir.normalize_or_zero() * strength / (scale * scale);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LimbKind {
    Arm,
    Leg,
    Wing,
}

#[derive(Component)]
pub struct Limb {
    kind: LimbKind,
    side: f32,
}

impl Limb {
    pub fn new(kind: LimbKind, side: f32) -> Self {
        Self { kind, side }
    }
}

/// A flaming seed thrown by a Lobber.
#[derive(Component)]
pub struct Seed {
    vel: Vec3,
    pub damage: i32,
}

/// Hit spheres in the enemy's local space (before scale): (center, radius, is_head).
const PARTS: [(Vec3, f32, bool); 3] = [
    (Vec3::new(0.0, 1.85, 0.0), 0.42, true),
    (Vec3::new(0.0, 1.2, 0.0), 0.42, false),
    (Vec3::new(0.0, 0.5, 0.0), 0.38, false),
];

/// World-space hit spheres for an enemy. Bats are only a head (with a slightly
/// generous radius, since they're small and fast).
pub fn hit_spheres(tf: &Transform, kind: Kind) -> impl Iterator<Item = (Vec3, f32, bool)> + '_ {
    let parts: &[(Vec3, f32, bool)] = if kind == Kind::Bat { &PARTS[..1] } else { &PARTS };
    let grow = if kind == Kind::Bat { 1.3 } else { 1.0 };
    parts
        .iter()
        .map(move |&(c, r, head)| (tf.translation + tf.rotation * (c * tf.scale), r * tf.scale.x * grow, head))
}

/// Closest enemy hit by a ray: (entity, distance, headshot).
#[cfg(test)]
pub fn raycast<'a>(
    origin: Vec3,
    dir: Vec3,
    enemies: impl Iterator<Item = (Entity, &'a Transform, Kind)>,
) -> Option<(Entity, f32, bool)> {
    raycast_all(origin, dir, enemies).into_iter().next()
}

/// Every enemy a ray passes through, nearest first, one entry each (its closest body part).
pub fn raycast_all<'a>(
    origin: Vec3,
    dir: Vec3,
    enemies: impl Iterator<Item = (Entity, &'a Transform, Kind)>,
) -> Vec<(Entity, f32, bool)> {
    let mut hits: Vec<(Entity, f32, bool)> = enemies
        .filter_map(|(e, tf, kind)| {
            hit_spheres(tf, kind)
                .filter_map(|(c, r, head)| ray_sphere(origin, dir, c, r).map(|d| (e, d, head)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
        })
        .collect();
    hits.sort_by(|a, b| a.1.total_cmp(&b.1));
    hits
}

fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t > 0.0).then_some(t)
}

// ---------------------------------------------------------------- waves

#[derive(Resource)]
pub struct Waves {
    pub wave: u32,
    pub intermission: f32,
    queue: Vec<Kind>,
    spawn_timer: f32,
    /// Enemies to spawn at a specific spot (the King's summons).
    pub summons: Vec<(Kind, Vec3)>,
}

impl Default for Waves {
    fn default() -> Self {
        Self { wave: 0, intermission: 3.0, queue: Vec::new(), spawn_timer: 0.0, summons: Vec::new() }
    }
}

/// Seconds between waves. This is when the upgrade shop is open.
pub const SHOP_TIME: f32 = 15.0;

pub fn is_boss_wave(n: u32) -> bool {
    n > 0 && n % 5 == 0
}

fn build_wave(n: u32) -> Vec<Kind> {
    if is_boss_wave(n) {
        // The King plus a smaller escort. He's pushed last so he spawns first.
        let mut q = Vec::new();
        q.extend(std::iter::repeat_n(Kind::Walker, (2 + n / 2) as usize));
        q.extend(std::iter::repeat_n(Kind::Lobber, (n / 5) as usize));
        q.extend(std::iter::repeat_n(Kind::Bat, (n / 5) as usize));
        q.shuffle(&mut rand::rng());
        q.push(boss::boss_for_wave(n));
        return q;
    }
    let mut q = Vec::new();
    q.extend(std::iter::repeat_n(Kind::Walker, (4 + 2 * n) as usize));
    if n >= 2 {
        q.extend(std::iter::repeat_n(Kind::Runner, (n + 1) as usize));
    }
    if n >= 3 {
        q.extend(std::iter::repeat_n(Kind::Lobber, (n - 2) as usize));
    }
    if n >= 4 {
        q.extend(std::iter::repeat_n(Kind::Brute, (n - 3) as usize));
    }
    if n >= 3 {
        q.extend(std::iter::repeat_n(Kind::Bat, (n / 2) as usize));
    }
    q.shuffle(&mut rand::rng());
    q
}

pub fn run_waves(
    mut commands: Commands,
    mut waves: ResMut<Waves>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    settings: Res<Settings>,
    enemies: Query<(), With<Enemy>>,
    player: Single<&Transform, With<Player>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let hp = settings.enemy_health_mult();

    // The King's summoned Runners appear right next to him.
    let summons = std::mem::take(&mut waves.summons);
    for (kind, pos) in summons {
        spawn_enemy(&mut commands, &mut meshes, &mut materials, &fx, kind, pos, hp);
    }

    if waves.intermission > 0.0 {
        waves.intermission -= dt;
        if waves.intermission <= 0.0 {
            waves.wave += 1;
            waves.queue = build_wave(waves.wave);
            waves.spawn_timer = 0.0;
            sound::play(&mut commands, &sfx.bell, 0.9);
            if is_boss_wave(waves.wave) {
                // A second, deeper toll for the King.
                sound::play_with_speed(&mut commands, &sfx.bell, 1.0, 0.6);
            }
        }
        return;
    }

    if waves.queue.is_empty() {
        if enemies.is_empty() {
            waves.intermission = SHOP_TIME;
        }
        return;
    }

    waves.spawn_timer -= dt;
    if waves.spawn_timer > 0.0 {
        return;
    }
    waves.spawn_timer = (1.3 - waves.wave as f32 * 0.08).max(0.35);
    let kind = waves.queue.pop().unwrap();

    let mut rng = rand::rng();
    let pos = loop {
        let along = rng.random_range(-ARENA_HALF..ARENA_HALF);
        let edge = ARENA_HALF - 1.0;
        let p = match rng.random_range(0..4) {
            0 => Vec3::new(along, 0.0, edge),
            1 => Vec3::new(along, 0.0, -edge),
            2 => Vec3::new(edge, 0.0, along),
            _ => Vec3::new(-edge, 0.0, along),
        };
        if p.xz().distance(player.translation.xz()) > 15.0 {
            break p;
        }
    };
    let health_mult = hp * if is_boss(kind) { boss::boss_health(kind, waves.wave) / kind.stats().health } else { 1.0 };
    spawn_enemy(&mut commands, &mut meshes, &mut materials, &fx, kind, pos, health_mult);
}

fn spawn_enemy(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fx: &Fx,
    kind: Kind,
    pos: Vec3,
    health_mult: f32,
) {
    let s = kind.stats();
    let health = s.health * health_mult;
    let mut rng = rand::rng();
    let look = looks::roll(kind, s.shirt, &mut rng);

    // Per-enemy materials so each one can flash and glow on its own.
    let skin = materials.add(StandardMaterial { base_color: look.skin, perceptual_roughness: 0.6, ..default() });
    let glow = looks::face_glow(kind) * 4.0;
    let face = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(glow.x, glow.y, glow.z),
        ..default()
    });
    let shirt = materials.add(StandardMaterial { base_color: look.shirt, perceptual_roughness: 1.0, ..default() });

    let torso = meshes.add(Cuboid::new(0.62, 0.72, 0.34));
    let arm = meshes.add(Cuboid::new(0.15, 0.62, 0.15));
    let hand = meshes.add(Cuboid::new(0.12, 0.18, 0.12));
    let leg = meshes.add(Cuboid::new(0.2, 0.82, 0.2));

    let mut root = commands.spawn((
        Enemy {
            kind,
            health,
            max_health: health,
            flash: 0.0,
            speed: s.speed * rng.random_range(0.85..1.15),
            damage: s.damage,
            points: s.points,
            walk_t: rng.random_range(0.0..6.0),
            attack_cd: 1.0,
            groan_cd: rng.random_range(2.0..8.0),
            swing: 0.0,
            frozen: false,
            vulnerable: 1.0,
            status: "",
            knock: Vec2::ZERO,
            waypoint: pos.xz(),
            burn: 0.0,
            burn_dps: 0.0,
            slow: 0.0,
            recent: 0.0,
            recent_crit: false,
            since_hit: f32::MAX,
            bar: None,
            face: face.clone(),
            skin: skin.clone(),
            shirt: shirt.clone(),
        },
        Transform::from_translation(pos).with_scale(Vec3::splat(s.scale)),
        Visibility::default(),
    ));
    match kind {
        Kind::King => {
            root.insert(King::default());
        }
        Kind::Witch => {
            root.insert(Witch::default());
        }
        Kind::Golem => {
            root.insert(Golem::new(false));
        }
        Kind::Gourdling => {
            root.insert(Golem::new(true));
        }
        Kind::Bat => {
            root.insert(Bat::default());
        }
        _ => {}
    }

    root.with_children(|p| {
        looks::spawn_head(p, meshes, materials, fx, &look, &skin, &face, &shirt);

        // Bats are just a head with wings.
        if kind == Kind::Bat {
            bat::spawn_wings(p, meshes, &shirt);
            return;
        }

        // Torso and a straw collar
        p.spawn((Mesh3d(torso), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, 1.2, 0.0)));
        p.spawn((
            Mesh3d(fx.cube.clone()),
            MeshMaterial3d(fx.straw.clone()),
            Transform::from_xyz(0.0, 1.55, 0.0).with_scale(Vec3::new(0.5, 0.08, 0.3)),
        ));

        // Arms: pivot at the shoulder, mesh hangs below it, straw hand at the end.
        for side in [-1.0, 1.0] {
            p.spawn((Limb { kind: LimbKind::Arm, side }, Transform::from_xyz(0.4 * side, 1.5, 0.0), Visibility::default()))
                .with_children(|a| {
                    a.spawn((Mesh3d(arm.clone()), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, -0.3, 0.0)));
                    a.spawn((Mesh3d(hand.clone()), MeshMaterial3d(fx.hands.clone()), Transform::from_xyz(0.0, -0.68, 0.0)));
                });
        }
        // Legs
        for side in [-1.0, 1.0] {
            p.spawn((Limb { kind: LimbKind::Leg, side }, Transform::from_xyz(0.15 * side, 0.84, 0.0), Visibility::default()))
                .with_children(|l| {
                    l.spawn((Mesh3d(leg.clone()), MeshMaterial3d(fx.pants.clone()), Transform::from_xyz(0.0, -0.41, 0.0)));
                });
        }
    });
}

// ---------------------------------------------------------------- behavior

pub fn move_and_attack(
    mut commands: Commands,
    mut enemies: Query<(Entity, &mut Enemy, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut hurt: ResMut<HurtFlash>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    obstacles: Res<Obstacles>,
    nav: Res<NavGrid>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (ref mut p, ptf) = *player;
    let target = ptf.translation.xz();
    let player_feet = ptf.translation.y - crate::player::EYE_HEIGHT;

    // Snapshot positions so enemies can push apart instead of stacking up.
    let positions: Vec<(Entity, Vec2, f32)> = enemies
        .iter()
        .map(|(e, _, tf)| (e, tf.translation.xz(), 0.45 * tf.scale.x))
        .collect();

    for (e, mut enemy, mut tf) in &mut enemies {
        let pos = tf.translation.xz();
        let to_player = target - pos;
        let dist = to_player.length().max(0.001);
        let dir = to_player / dist;
        let reach = 1.1 + 0.5 * tf.scale.x;

        enemy.attack_cd -= dt;
        enemy.swing = (enemy.swing - dt * 3.0).max(0.0);

        let wants_range = (enemy.kind == Kind::Lobber && dist < 16.0) || (enemy.kind == Kind::Witch && dist < 18.0);
        let winding_up = enemy.frozen;
        let moving = dist > reach && !wants_range && !winding_up;
        // Pathfinding: head for the next waypoint around the props (straight at you
        // when nothing's in the way).
        enemy.waypoint = nav.field_for(tf.scale.x).waypoint(pos, target);
        let heading = (enemy.waypoint - pos).normalize_or(dir);
        // Burning: damage over time and a trail of sparks. Caramel: half speed.
        if enemy.burn > 0.0 {
            enemy.burn -= dt;
            enemy.health -= enemy.burn_dps * dt;
            if rand::rng().random::<f32>() < dt * 8.0 {
                let at = tf.translation + Vec3::Y * rand::rng().random_range(0.8..2.0) * tf.scale.y;
                effects::burst(&mut commands, &fx, at, 2, &[fx.seed.clone(), fx.power_damage.clone()], 1.5);
            }
            if enemy.burn <= 0.0 {
                enemy.burn_dps = 0.0;
            }
        }
        enemy.slow = (enemy.slow - dt).max(0.0);
        let speed = enemy.speed * if enemy.slow > 0.0 { 0.5 } else { 1.0 };

        let mut step = Vec2::ZERO;
        if moving {
            step += heading * speed * dt;
            enemy.walk_t += dt * speed * 2.2;
        }

        // Knockback from the pitchfork and scattergun
        step += enemy.knock * dt;
        enemy.knock *= (1.0 - 6.0 * dt).max(0.0);

        // Bats fly: `bat.rs` moves them, so skip crowding and prop collision.
        let flying = enemy.kind == Kind::Bat;

        // Separation
        for &(other, opos, orad) in positions.iter().filter(|_| !flying) {
            if other == e {
                continue;
            }
            let d = pos - opos;
            let min = 0.45 * tf.scale.x + orad;
            let len = d.length();
            if len > 0.0 && len < min {
                step += d / len * (min - len) * 0.5;
            }
        }
        if !flying {
            tf.translation.x = (tf.translation.x + step.x).clamp(-ARENA_HALF, ARENA_HALF);
            tf.translation.z = (tf.translation.z + step.y).clamp(-ARENA_HALF, ARENA_HALF);
            // Walk around props instead of through them.
            let (resolved, _) = obstacles.resolve(Vec3::new(tf.translation.x, 0.0, tf.translation.z), 0.4 * tf.scale.x, 2.0 * tf.scale.y);
            tf.translation.x = resolved.x;
            tf.translation.z = resolved.z;
        }

        enemy.groan_cd -= dt;
        if enemy.groan_cd <= 0.0 {
            enemy.groan_cd = rand::rng().random_range(5.0..12.0);
            if dist < 20.0 {
                sound::play_at(&mut commands, &sfx.groan, tf.translation, ptf.translation, 0.5 * tf.scale.x);
            }
        }
        // Face where they're walking; face you once they've stopped to attack.
        if !winding_up {
            let face = if moving { heading } else { dir };
            let want = Quat::from_rotation_y((-face.x).atan2(-face.y));
            tf.rotation = tf.rotation.slerp(want, (10.0 * dt).min(1.0));
        }

        // Attacks
        if enemy.attack_cd <= 0.0 && !winding_up {
            if enemy.kind == Kind::Lobber && wants_range {
                enemy.attack_cd = 2.6;
                enemy.swing = 1.0;
                let from = tf.translation + Vec3::Y * 2.2 * tf.scale.y;
                throw_seed(&mut commands, &fx, from, ptf.translation - Vec3::Y * 0.6, enemy.damage);
                sound::play_at(&mut commands, &sfx.throw, from, ptf.translation, 0.6);
            } else if dist <= reach && player_feet < 1.2 * tf.scale.y {
                // Standing on a tall enough stack keeps you out of reach (except from Brutes).
                enemy.attack_cd = 1.4;
                enemy.swing = 1.0;
                p.take_damage(enemy.damage, tf.translation);
                hurt.0 = 0.35;
                sound::play(&mut commands, &sfx.hurt, 0.8);
            }
        }

        // Faces glow hotter as they close in; bodies flash white when shot.
        let closeness = (1.0 - dist / 25.0).clamp(0.0, 1.0);
        if let Some(mut m) = materials.get_mut(&enemy.face) {
            let g = looks::face_glow(enemy.kind) * (2.0 + closeness * 10.0);
            m.emissive = LinearRgba::rgb(g.x, g.y, g.z);
        }
        enemy.flash = (enemy.flash - dt).max(0.0);
        let flashing = enemy.flash > 0.0;
        let white = LinearRgba::rgb(3.0, 3.0, 3.0);
        // Heads keep a faint glow so they read as orange in the dark.
        if let Some(mut m) = materials.get_mut(&enemy.skin) {
            // Bats glow more so you can spot them against the night sky.
            let idle = if enemy.kind == Kind::Bat { LinearRgba::rgb(0.6, 0.22, 0.0) } else { LinearRgba::rgb(0.12, 0.04, 0.0) };
            m.emissive = if flashing { white } else { idle };
        }
        if let Some(mut m) = materials.get_mut(&enemy.shirt) {
            let idle = if enemy.kind == Kind::Bat { LinearRgba::rgb(0.25, 0.05, 0.4) } else { LinearRgba::BLACK };
            m.emissive = if flashing { white } else { idle };
        }
    }
}

/// Swing legs while walking; arms reach forward like a zombie and slash when attacking.
pub fn animate_limbs(
    enemies: Query<(&Enemy, &Children)>,
    mut limbs: Query<(&Limb, &mut Transform)>,
) {
    for (enemy, children) in &enemies {
        let phase = enemy.walk_t.sin();
        for child in children.iter() {
            if let Ok((limb, mut tf)) = limbs.get_mut(child) {
                tf.rotation = match limb.kind {
                    LimbKind::Leg => Quat::from_rotation_x(phase * 0.6 * limb.side),
                    LimbKind::Arm => {
                        let base = 1.25 + phase * 0.15 * limb.side;
                        Quat::from_rotation_x(base + enemy.swing * 1.0 - (enemy.swing * std::f32::consts::PI).sin() * 1.4)
                    }
                    // Wings flap up and down around the shoulder.
                    LimbKind::Wing => Quat::from_rotation_z(limb.side * (0.15 + phase * 0.75)),
                };
            }
        }
    }
}

pub fn throw_seed(commands: &mut Commands, fx: &Fx, from: Vec3, to: Vec3, damage: i32) {
    // Solve a ballistic arc that lands on the target after `t` seconds.
    let g = 12.0;
    let flat = (to - from).xz().length();
    let t = (flat / 11.0).clamp(0.6, 2.0);
    let mut vel = (to - from) / t;
    vel.y = (to.y - from.y + 0.5 * g * t * t) / t;
    commands.spawn((
        Seed { vel, damage },
        Mesh3d(fx.ball.clone()),
        MeshMaterial3d(fx.seed.clone()),
        Transform::from_translation(from).with_scale(Vec3::new(0.14, 0.2, 0.14)),
    ));
}

pub fn update_seeds(
    mut commands: Commands,
    mut seeds: Query<(Entity, &mut Seed, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut hurt: ResMut<HurtFlash>,
    obstacles: Res<Obstacles>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (ref mut p, ptf) = *player;
    for (e, mut seed, mut tf) in &mut seeds {
        seed.vel.y -= 12.0 * dt;
        tf.translation += seed.vel * dt;
        tf.rotate_x(10.0 * dt);
        let body = ptf.translation - Vec3::Y * 0.6;
        if tf.translation.distance(body) < 0.9 {
            // Point the damage indicator back along the seed's arc.
            p.take_damage(seed.damage, tf.translation - seed.vel.normalize_or_zero() * 3.0);
            hurt.0 = 0.3;
            sound::play(&mut commands, &sfx.hurt, 0.8);
            effects::burst(&mut commands, &fx, tf.translation, 5, &[fx.seed.clone()], 3.0);
            commands.entity(e).despawn();
        } else if tf.translation.y < 0.05 || obstacles.contains(tf.translation, 0.05) {
            effects::burst(&mut commands, &fx, tf.translation, 4, &[fx.seed.clone()], 2.0);
            commands.entity(e).despawn();
        }
    }
}

pub fn handle_deaths(
    mut commands: Commands,
    enemies: Query<(Entity, &Enemy, &Transform)>,
    mut score: ResMut<Score>,
    mut waves: ResMut<Waves>,
    mut shake: ResMut<Shake>,
    player: Single<&Transform, With<Player>>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
) {
    let mut rng = rand::rng();
    for (e, enemy, tf) in &enemies {
        if enemy.health > 0.0 {
            continue;
        }
        score.points += enemy.points;
        score.candy += enemy.points;
        score.kills += 1;
        let head = tf.translation + Vec3::Y * 1.85 * tf.scale.y;
        let n = (16.0 * tf.scale.x) as usize;
        effects::burst(&mut commands, &fx, head, n, &[fx.pumpkin.clone(), enemy.skin.clone()], 5.0);
        effects::burst(&mut commands, &fx, tf.translation + Vec3::Y, n / 2, &[fx.straw.clone(), enemy.shirt.clone()], 3.5);
        effects::light_flash(&mut commands, head, Color::srgb(1.0, 0.5, 0.1), 80_000.0, 8.0, 0.25);
        sound::play_at(&mut commands, &sfx.splat, head, player.translation, 0.9 * tf.scale.x);
        // Goo on the ground where it fell (straight down for bats killed in the air).
        effects::goo(&mut commands, &fx, &enemy.skin, tf.translation, 0.55 * tf.scale.x);

        if is_boss(enemy.kind) {
            // Bosses go out with a bang and a pile of loot, including a power-up.
            let big = enemy.kind != Kind::Gourdling;
            let g = looks::face_glow(enemy.kind);
            effects::burst(&mut commands, &fx, head, if big { 60 } else { 25 }, &[fx.pumpkin.clone(), enemy.skin.clone(), fx.seed.clone()], 10.0);
            effects::light_flash(&mut commands, head, Color::srgb(g.x, g.y, g.z), 3_000_000.0, 30.0, 0.8);
            sound::play(&mut commands, &sfx.explosion, 1.0);
            shake.add(if big { 0.6 } else { 0.3 });
            let drops: &[PickupKind] = if big {
                &[PickupKind::Health, PickupKind::Ammo, PickupKind::Health, PickupKind::Ammo, PickupKind::Health, PickupKind::random_power_up()]
            } else {
                &[PickupKind::Health, PickupKind::Ammo]
            };
            for (k, &kind) in drops.iter().enumerate() {
                let a = k as f32 / drops.len() as f32 * std::f32::consts::TAU;
                let at = Vec3::new(tf.translation.x + a.sin() * 2.5, 0.0, tf.translation.z + a.cos() * 2.5);
                spawn_pickup(&mut commands, &fx, kind, at);
            }
            // The Golem splits in two.
            if enemy.kind == Kind::Golem {
                let side = Vec3::X * 2.5;
                waves.summons.push((Kind::Gourdling, Vec3::new(tf.translation.x, 0.0, tf.translation.z) + side));
                waves.summons.push((Kind::Gourdling, Vec3::new(tf.translation.x, 0.0, tf.translation.z) - side));
            }
            commands.entity(e).despawn();
            continue;
        }
        if rng.random::<f32>() < 0.06 {
            spawn_pickup(&mut commands, &fx, PickupKind::random_power_up(), Vec3::new(tf.translation.x, 0.0, tf.translation.z));
            commands.entity(e).despawn();
            continue;
        }
        let drop_chance = if enemy.kind == Kind::Brute { 1.0 } else { 0.22 };
        if rng.random::<f32>() < drop_chance {
            let kind = if rng.random::<f32>() < 0.45 { PickupKind::Health } else { PickupKind::Ammo };
            spawn_pickup(&mut commands, &fx, kind, Vec3::new(tf.translation.x, 0.0, tf.translation.z));
        }
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_level_shot_is_a_headshot() {
        let tf = Transform::from_xyz(0.0, 0.0, -10.0);
        let e = Entity::from_raw_u32(1).unwrap();
        let hit = raycast(Vec3::new(0.0, 1.85, 0.0), Vec3::NEG_Z, [(e, &tf, Kind::Walker)].into_iter());
        let (_, dist, head) = hit.expect("should hit");
        assert!(head);
        assert!((dist - 9.58).abs() < 0.05);
    }

    #[test]
    fn low_shot_hits_body_and_misses_wide() {
        let tf = Transform::from_xyz(0.0, 0.0, -10.0);
        let e = Entity::from_raw_u32(1).unwrap();
        let body = raycast(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z, [(e, &tf, Kind::Walker)].into_iter());
        assert!(matches!(body, Some((_, _, false))));
        let miss = raycast(Vec3::new(2.0, 1.7, 0.0), Vec3::NEG_Z, [(e, &tf, Kind::Walker)].into_iter());
        assert!(miss.is_none());
    }

    #[test]
    fn brute_hitbox_scales_up() {
        let tf = Transform::from_xyz(0.0, 0.0, -10.0).with_scale(Vec3::splat(1.55));
        let e = Entity::from_raw_u32(1).unwrap();
        // Brute's head sits ~2.87m up, above a normal walker's head.
        let hit = raycast(Vec3::new(0.0, 2.85, 0.0), Vec3::NEG_Z, [(e, &tf, Kind::Walker)].into_iter());
        assert!(matches!(hit, Some((_, _, true))));
    }

    #[test]
    fn piercing_ray_lists_enemies_nearest_first() {
        let near = Transform::from_xyz(0.0, 0.0, -5.0);
        let far = Transform::from_xyz(0.0, 0.0, -12.0);
        let off = Transform::from_xyz(4.0, 0.0, -8.0);
        let (a, b, c) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap(), Entity::from_raw_u32(3).unwrap());
        let hits = raycast_all(Vec3::new(0.0, 1.85, 0.0), Vec3::NEG_Z, [(b, &far, Kind::Walker), (c, &off, Kind::Walker), (a, &near, Kind::Walker)].into_iter());
        let order: Vec<Entity> = hits.iter().map(|h| h.0).collect();
        assert_eq!(order, vec![a, b], "the off-line enemy isn't hit; the near one comes first");
    }

    #[test]
    fn enemy_dies_after_enough_damage() {
        let s = Kind::Walker.stats();
        let mut hp = s.health;
        hp -= 1.0 * 2.0; // pistol headshot
        hp -= 1.0; // pistol body shot
        assert!(hp <= 0.0);
    }
}

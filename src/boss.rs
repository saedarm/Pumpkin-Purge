//! Bosses. One shows up every 5th wave, rotating through three:
//!
//! - **Wave 5: The Pumpkin King.** Giant with a crown. Ground slam (jump the shockwave
//!   ring), five-seed volley at range, summons Runners.
//! - **Wave 10: The Scarecrow Witch.** Floats, keeps her distance, fires homing wisps,
//!   teleports away when you hurt her, and summons Lobbers.
//! - **Wave 15: The Gourd Golem.** Winds up and charges in a straight line. If it crashes
//!   into a prop it's stunned for longer and takes 50% extra damage. Splits into two
//!   Gourdlings when it dies.
//!
//! Then the cycle repeats (King on 20, Witch on 25, ...) with more health each time.

use bevy::prelude::*;
use rand::Rng;

use crate::collision::Obstacles;
use crate::effects::{self, Fx};
use crate::enemies::{self, Enemy, Kind, Waves};
use crate::festival::ARENA_HALF;
use crate::player::{EYE_HEIGHT, HurtFlash, Player, Shake};
use crate::sound::{self, Sfx};

pub fn boss_for_wave(wave: u32) -> Kind {
    match (wave / 5).saturating_sub(1) % 3 {
        0 => Kind::King,
        1 => Kind::Witch,
        _ => Kind::Golem,
    }
}

/// Bosses get 60% more health each time the rotation comes back around.
pub fn boss_health(kind: Kind, wave: u32) -> f32 {
    let cycle = (wave / 5).saturating_sub(1) / 3;
    kind.stats().health * (1.0 + 0.6 * cycle as f32)
}

// ================================================================ Pumpkin King

const SLAM_RANGE: f32 = 8.0;
const SLAM_WINDUP: f32 = 0.9;
const SHOCKWAVE_SPEED: f32 = 12.0;
const SHOCKWAVE_MAX: f32 = 10.0;
const SHOCKWAVE_DAMAGE: i32 = 25;

#[derive(Component)]
pub struct King {
    special_cd: f32,
    summon_cd: f32,
    /// Seconds until the slam lands, while winding up.
    slam: Option<f32>,
}

impl Default for King {
    fn default() -> Self {
        Self { special_cd: 4.0, summon_cd: 10.0, slam: None }
    }
}

#[derive(Component)]
pub struct Shockwave {
    radius: f32,
    hit: bool,
}

pub fn king_attacks(
    mut commands: Commands,
    mut kings: Query<(&mut King, &mut Enemy, &Transform)>,
    player: Single<&Transform, With<Player>>,
    mut waves: ResMut<Waves>,
    mut shake: ResMut<Shake>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut king, mut enemy, tf) in &mut kings {
        let to_player = player.translation - tf.translation;
        let dist = to_player.xz().length();
        enemy.frozen = king.slam.is_some();

        // Winding up: arms stay raised until the slam lands.
        if let Some(left) = king.slam {
            let left = left - dt;
            if left > 0.0 {
                king.slam = Some(left);
                enemy.swing = 1.0;
            } else {
                king.slam = None;
                enemy.swing = 0.95; // let the arms come crashing down
                let at = Vec3::new(tf.translation.x, 0.05, tf.translation.z);
                commands.spawn((
                    Shockwave { radius: 1.0, hit: false },
                    Mesh3d(fx.ring.clone()),
                    MeshMaterial3d(fx.tracer.clone()),
                    Transform::from_translation(at),
                ));
                effects::burst(&mut commands, &fx, at + Vec3::Y * 0.2, 24, &[fx.straw.clone(), fx.pumpkin.clone()], 6.0);
                effects::light_flash(&mut commands, at + Vec3::Y, Color::srgb(1.0, 0.5, 0.1), 1_500_000.0, 16.0, 0.4);
                sound::play_at(&mut commands, &sfx.explosion, at, player.translation, 1.0);
                shake.add_at(0.7, at, player.translation);
            }
            continue;
        }

        king.special_cd -= dt;
        if king.special_cd <= 0.0 {
            if dist < SLAM_RANGE {
                king.slam = Some(SLAM_WINDUP);
                king.special_cd = 5.0;
                sound::play_with_speed(&mut commands, &sfx.groan, 1.0, 0.6);
            } else {
                // Seed volley: five seeds fanned out around where you're standing.
                king.special_cd = 3.5;
                enemy.swing = 1.0;
                let from = tf.translation + Vec3::Y * 5.5;
                let target = player.translation - Vec3::Y * 0.6;
                let side = to_player.cross(Vec3::Y).normalize_or_zero();
                for k in -2..=2 {
                    enemies::throw_seed(&mut commands, &fx, from, target + side * (k as f32 * 2.2), 12);
                }
                sound::play_at(&mut commands, &sfx.throw, from, player.translation, 0.9);
            }
        }

        king.summon_cd -= dt;
        if king.summon_cd <= 0.0 {
            king.summon_cd = 14.0;
            summon_pair(&mut waves, Kind::Runner, tf.translation, to_player);
            sound::play_with_speed(&mut commands, &sfx.groan, 0.9, 0.75);
        }
    }
}

/// Expanding ring that hurts you as it passes, unless you're in the air or up on a prop.
pub fn update_shockwaves(
    mut commands: Commands,
    mut waves: Query<(Entity, &mut Shockwave, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut hurt: ResMut<HurtFlash>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let (ref mut p, ptf) = *player;
    let feet = ptf.translation.y - EYE_HEIGHT;
    for (e, mut wave, mut tf) in &mut waves {
        wave.radius += SHOCKWAVE_SPEED * time.delta_secs();
        tf.scale = Vec3::new(wave.radius, 1.0, wave.radius);
        let dist = ptf.translation.xz().distance(tf.translation.xz());
        if !wave.hit && (dist - wave.radius).abs() < 0.8 && feet < 0.35 {
            wave.hit = true;
            p.take_damage(SHOCKWAVE_DAMAGE, tf.translation);
            hurt.0 = 0.5;
            sound::play(&mut commands, &sfx.hurt, 1.0);
        }
        if wave.radius > SHOCKWAVE_MAX {
            commands.entity(e).despawn();
        }
    }
}

fn summon_pair(waves: &mut Waves, kind: Kind, at: Vec3, to_player: Vec3) {
    let side = to_player.cross(Vec3::Y).normalize_or_zero() * 3.0;
    for s in [-1.0, 1.0] {
        let p = Vec3::new(at.x, 0.0, at.z) + side * s;
        waves.summons.push((kind, Vec3::new(p.x.clamp(-ARENA_HALF, ARENA_HALF), 0.0, p.z.clamp(-ARENA_HALF, ARENA_HALF))));
    }
}

// ================================================================ Scarecrow Witch

const HOVER: f32 = 1.2;
const WISP_SPEED: f32 = 6.5;
const WISP_TURN: f32 = 1.8;
const WISP_DAMAGE: i32 = 8;

#[derive(Component)]
pub struct Witch {
    wisp_cd: f32,
    teleport_cd: f32,
    summon_cd: f32,
    /// Health fraction at the last teleport; dropping 20% below it triggers another.
    last_health: f32,
}

impl Default for Witch {
    fn default() -> Self {
        Self { wisp_cd: 3.0, teleport_cd: 10.0, summon_cd: 8.0, last_health: 1.0 }
    }
}

/// A homing ghost-light. Steers toward you, pops on props.
#[derive(Component)]
pub struct Wisp {
    vel: Vec3,
    life: f32,
}

pub fn witch_attacks(
    mut commands: Commands,
    mut witches: Query<(&mut Witch, &mut Enemy, &mut Transform), Without<Player>>,
    player: Single<&Transform, With<Player>>,
    mut waves: ResMut<Waves>,
    obstacles: Res<Obstacles>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut witch, mut enemy, mut tf) in &mut witches {
        // Float above the ground with a slow bob.
        tf.translation.y = HOVER + (t * 2.0).sin() * 0.25;
        let to_player = player.translation - tf.translation;

        witch.wisp_cd -= dt;
        if witch.wisp_cd <= 0.0 {
            witch.wisp_cd = 3.2;
            enemy.swing = 1.0;
            let from = tf.translation + Vec3::Y * 3.0;
            let side = to_player.cross(Vec3::Y).normalize_or_zero();
            for k in -1..=1 {
                // Launch them fanned out and upward; they curve in toward you.
                let vel = (side * k as f32 * 0.8 + Vec3::Y * 0.6).normalize_or_zero() * WISP_SPEED;
                commands.spawn((
                    Wisp { vel, life: 6.0 },
                    Mesh3d(fx.ball.clone()),
                    MeshMaterial3d(fx.wisp.clone()),
                    Transform::from_translation(from).with_scale(Vec3::splat(0.22)),
                    PointLight { color: Color::srgb(0.7, 0.3, 1.0), intensity: 40_000.0, range: 5.0, ..default() },
                ));
            }
            sound::play_at(&mut commands, &sfx.throw, from, player.translation, 0.8);
        }

        // Teleport on a timer, or when you've knocked off another 20% of her health.
        witch.teleport_cd -= dt;
        let frac = enemy.health / enemy.max_health;
        if witch.teleport_cd <= 0.0 || frac < witch.last_health - 0.2 {
            witch.teleport_cd = 9.0;
            witch.last_health = frac;
            let mut rng = rand::rng();
            let to = loop {
                let p = Vec3::new(
                    rng.random_range(-ARENA_HALF + 3.0..ARENA_HALF - 3.0),
                    tf.translation.y,
                    rng.random_range(-ARENA_HALF + 3.0..ARENA_HALF - 3.0),
                );
                if p.xz().distance(player.translation.xz()) > 12.0 && !obstacles.contains(Vec3::new(p.x, 0.5, p.z), 1.5) {
                    break p;
                }
            };
            for at in [tf.translation, to] {
                effects::burst(&mut commands, &fx, at + Vec3::Y * 2.0, 18, &[fx.wisp.clone(), fx.black.clone()], 4.0);
                effects::light_flash(&mut commands, at + Vec3::Y * 2.0, Color::srgb(0.7, 0.2, 1.0), 400_000.0, 10.0, 0.4);
            }
            sound::play_with_speed(&mut commands, &sfx.throw, 0.9, 0.6);
            tf.translation = to;
        }

        witch.summon_cd -= dt;
        if witch.summon_cd <= 0.0 {
            witch.summon_cd = 16.0;
            summon_pair(&mut waves, Kind::Lobber, tf.translation, to_player);
            sound::play_with_speed(&mut commands, &sfx.groan, 0.8, 1.3);
        }
    }
}

pub fn update_wisps(
    mut commands: Commands,
    mut wisps: Query<(Entity, &mut Wisp, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut hurt: ResMut<HurtFlash>,
    obstacles: Res<Obstacles>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (ref mut p, ptf) = *player;
    let target = ptf.translation - Vec3::Y * 0.5;
    for (e, mut wisp, mut tf) in &mut wisps {
        wisp.life -= dt;
        let desired = (target - tf.translation).normalize_or_zero() * WISP_SPEED;
        wisp.vel = wisp.vel.lerp(desired, (WISP_TURN * dt).min(1.0));
        tf.translation += wisp.vel * dt;

        let hit_player = tf.translation.distance(target) < 0.8;
        if hit_player {
            // Point the damage indicator back along the wisp's path.
            p.take_damage(WISP_DAMAGE, tf.translation - wisp.vel.normalize_or_zero() * 3.0);
            hurt.0 = 0.3;
            sound::play(&mut commands, &sfx.hurt, 0.8);
        }
        if hit_player || wisp.life <= 0.0 || tf.translation.y < 0.1 || obstacles.contains(tf.translation, 0.1) {
            effects::burst(&mut commands, &fx, tf.translation, 6, &[fx.wisp.clone()], 2.5);
            commands.entity(e).despawn();
        }
    }
}

// ================================================================ Gourd Golem

const CHARGE_WINDUP: f32 = 1.1;
const CHARGE_SPEED: f32 = 15.0;
const CHARGE_TIME: f32 = 1.4;
const STUN_TIME: f32 = 2.0;
const CRASH_STUN_TIME: f32 = 3.5;

enum Phase {
    Stalking,
    WindingUp(f32),
    Charging { dir: Vec2, left: f32, hit: bool },
    Stunned(f32),
}

#[derive(Component)]
pub struct Golem {
    phase: Phase,
    charge_cd: f32,
    /// Gourdlings: smaller, faster, and they don't split again.
    minor: bool,
}

impl Golem {
    pub fn new(minor: bool) -> Self {
        Self { phase: Phase::Stalking, charge_cd: if minor { 2.5 } else { 4.0 }, minor }
    }
}

pub fn golem_attacks(
    mut commands: Commands,
    mut golems: Query<(&mut Golem, &mut Enemy, &mut Transform), Without<Player>>,
    mut player: Single<(&mut Player, &Transform)>,
    mut hurt: ResMut<HurtFlash>,
    mut shake: ResMut<Shake>,
    obstacles: Res<Obstacles>,
    fx: Res<Fx>,
    sfx: Res<Sfx>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (ref mut p, ptf) = *player;
    for (mut golem, mut enemy, mut tf) in &mut golems {
        let to_player = ptf.translation.xz() - tf.translation.xz();
        let dist = to_player.length();

        golem.phase = match golem.phase {
            Phase::Stalking => {
                enemy.frozen = false;
                enemy.vulnerable = 1.0;
                enemy.status = "";
                golem.charge_cd -= dt;
                if golem.charge_cd <= 0.0 && dist > 5.0 && dist < 28.0 {
                    sound::play_with_speed(&mut commands, &sfx.groan, 1.0, if golem.minor { 0.8 } else { 0.5 });
                    Phase::WindingUp(CHARGE_WINDUP)
                } else {
                    Phase::Stalking
                }
            }
            Phase::WindingUp(left) => {
                // Stand still, face you, arms up, and paw the ground.
                enemy.frozen = true;
                enemy.swing = 1.0;
                enemy.status = "CHARGING";
                tf.rotation = Quat::from_rotation_y((-to_player.x).atan2(-to_player.y));
                if left - dt <= 0.0 {
                    Phase::Charging { dir: to_player.normalize_or_zero(), left: CHARGE_TIME, hit: false }
                } else {
                    Phase::WindingUp(left - dt)
                }
            }
            Phase::Charging { dir, left, mut hit } => {
                enemy.frozen = true;
                let speed = if golem.minor { CHARGE_SPEED * 1.15 } else { CHARGE_SPEED };
                let want = tf.translation.xz() + dir * speed * dt;
                let clamped = want.clamp(Vec2::splat(-ARENA_HALF), Vec2::splat(ARENA_HALF));
                let (resolved, _) = obstacles.resolve(Vec3::new(clamped.x, 0.0, clamped.y), 0.4 * tf.scale.x, 2.0 * tf.scale.y);
                let crashed = resolved.xz().distance(want) > 0.05;
                tf.translation.x = resolved.x;
                tf.translation.z = resolved.z;
                enemy.walk_t += dt * 14.0;

                // Trample: one hit per charge if you're in the way and on the ground.
                let feet = ptf.translation.y - EYE_HEIGHT;
                if !hit && dist < 0.9 + 0.6 * tf.scale.x && feet < 0.6 * tf.scale.y {
                    hit = true;
                    p.take_damage(if golem.minor { 20 } else { 35 }, tf.translation);
                    hurt.0 = 0.5;
                    sound::play(&mut commands, &sfx.hurt, 1.0);
                }

                if crashed {
                    let at = tf.translation + Vec3::Y * 2.0;
                    effects::burst(&mut commands, &fx, at, 20, &[fx.straw.clone(), fx.stem.clone()], 6.0);
                    sound::play_at(&mut commands, &sfx.explosion, at, ptf.translation, 0.7);
                    shake.add_at(0.55, at, ptf.translation);
                    Phase::Stunned(CRASH_STUN_TIME)
                } else if left - dt <= 0.0 {
                    Phase::Stunned(STUN_TIME)
                } else {
                    Phase::Charging { dir, left: left - dt, hit }
                }
            }
            Phase::Stunned(left) => {
                // Dazed: wobbling in place and taking extra damage.
                enemy.frozen = true;
                enemy.vulnerable = 1.5;
                enemy.status = "STUNNED - hit it!";
                enemy.swing = 0.0;
                tf.rotate_y((time.elapsed_secs() * 9.0).sin() * 1.5 * dt);
                if left - dt <= 0.0 {
                    golem.charge_cd = if golem.minor { 3.0 } else { 5.0 };
                    Phase::Stalking
                } else {
                    Phase::Stunned(left - dt)
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bosses_rotate_king_witch_golem() {
        assert_eq!(boss_for_wave(5), Kind::King);
        assert_eq!(boss_for_wave(10), Kind::Witch);
        assert_eq!(boss_for_wave(15), Kind::Golem);
        assert_eq!(boss_for_wave(20), Kind::King);
        assert_eq!(boss_for_wave(25), Kind::Witch);
    }

    #[test]
    fn bosses_get_tougher_each_rotation() {
        assert_eq!(boss_health(Kind::King, 5), 150.0);
        assert_eq!(boss_health(Kind::King, 20), 240.0);
        assert_eq!(boss_health(Kind::Golem, 15), 220.0);
        assert!(boss_health(Kind::Witch, 25) > boss_health(Kind::Witch, 10));
    }
}

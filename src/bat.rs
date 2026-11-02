//! Bat pumpkins: small flying jack-o'-lanterns with bat wings. Wave 3 onward.
//!
//! 1. **Circle** overhead at about 4.5m, orbiting you.
//! 2. **Wind up:** hover in place, wings flapping hard, and screech. That's your warning.
//! 3. **Dive** in a straight line at where you were standing.
//!    - Hit you: you take damage and the bat climbs back up to try again.
//!    - Miss and hit the ground (or a prop): it's **stunned** on the ground for 1.6s,
//!      takes 50% extra damage, and the pitchfork can reach it.
//! 4. **Climb** back to altitude and start circling again.
//!
//! Bats ignore pathfinding and props while flying. Their hitbox is just the head.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use rand::Rng;

use crate::collision::Obstacles;
use crate::effects::{self, Fx};
use crate::enemies::{Enemy, Limb, LimbKind};
use crate::festival::ARENA_HALF;
use crate::player::{EYE_HEIGHT, HurtFlash, Player, Shake};
use crate::sound::{self, Sfx};

/// Height of the head above the enemy root, before scale (matches `looks.rs`).
const HEAD: f32 = 1.85;
const CRUISE_ALT: f32 = 4.5;
const ORBIT_RADIUS: f32 = 8.0;
const WINDUP: f32 = 0.7;
const DIVE_SPEED: f32 = 16.0;
const DIVE_TIME: f32 = 1.6;
const STUN_TIME: f32 = 1.6;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Circling,
    WindUp,
    Dive(Vec3),
    Climb,
    Stunned,
}

#[derive(Component)]
pub struct Bat {
    pub phase: Phase,
    timer: f32,
    /// Orbit angle around the player, and which way round it goes.
    angle: f32,
    spin: f32,
    /// False until the first frame, when the bat lifts off from its spawn point.
    launched: bool,
}

impl Default for Bat {
    fn default() -> Self {
        let mut rng = rand::rng();
        Self {
            phase: Phase::Climb,
            timer: rng.random_range(2.5..5.0),
            angle: rng.random_range(0.0..std::f32::consts::TAU),
            spin: if rng.random::<bool>() { 1.0 } else { -1.0 },
            launched: false,
        }
    }
}

/// Two wings, each a pivot at the side of the head with two flat panels.
pub fn spawn_wings(p: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, wing_mat: &Handle<StandardMaterial>) {
    let inner = meshes.add(Cuboid::new(0.55, 0.03, 0.42));
    let outer = meshes.add(Cuboid::new(0.5, 0.03, 0.3));
    for side in [-1.0f32, 1.0] {
        p.spawn((Limb::new(LimbKind::Wing, side), Transform::from_xyz(0.4 * side, HEAD, 0.05), Visibility::default()))
            .with_children(|w| {
                w.spawn((Mesh3d(inner.clone()), MeshMaterial3d(wing_mat.clone()), Transform::from_xyz(0.3 * side, 0.0, 0.0)));
                // The outer panel angles down a little, like a bat's wing tip.
                w.spawn((
                    Mesh3d(outer.clone()),
                    MeshMaterial3d(wing_mat.clone()),
                    Transform::from_xyz(0.75 * side, -0.08, 0.06).with_rotation(Quat::from_rotation_z(-0.35 * side)),
                ));
            });
    }
}

/// Pure movement step for one bat, separated out so it can be unit tested.
/// Returns the new head position and, if the dive connected this frame, `true`.
pub fn step(bat: &mut Bat, head: Vec3, target: Vec3, speed_mult: f32, dt: f32, blocked: impl Fn(Vec3) -> bool) -> (Vec3, bool) {
    let mut head = head;
    let mut hit = false;
    let toward = |head: Vec3, goal: Vec3, speed: f32| {
        let d = goal - head;
        let len = d.length();
        if len < 1e-3 { goal } else { head + d / len * (speed * dt).min(len) }
    };
    bat.timer -= dt;
    match bat.phase {
        Phase::Circling => {
            bat.angle += bat.spin * 0.9 * dt;
            let orbit = Vec3::new(bat.angle.cos(), 0.0, bat.angle.sin()) * ORBIT_RADIUS;
            let goal = Vec3::new(target.x, CRUISE_ALT, target.z) + orbit;
            head = toward(head, goal, 7.0 * speed_mult);
            if bat.timer <= 0.0 {
                bat.phase = Phase::WindUp;
                bat.timer = WINDUP;
            }
        }
        Phase::WindUp => {
            if bat.timer <= 0.0 {
                let dir = (target - head).normalize_or(Vec3::NEG_Y);
                bat.phase = Phase::Dive(dir * DIVE_SPEED * speed_mult.max(0.6));
                bat.timer = DIVE_TIME;
            }
        }
        Phase::Dive(vel) => {
            head += vel * dt;
            if head.distance(target) < 0.95 {
                hit = true;
                bat.phase = Phase::Climb;
            } else if head.y < 0.45 || blocked(head) {
                head.y = head.y.max(0.45);
                bat.phase = Phase::Stunned;
                bat.timer = STUN_TIME;
            } else if bat.timer <= 0.0 {
                bat.phase = Phase::Climb;
            }
        }
        Phase::Climb => {
            // Straight up and a little away from you, then back to circling.
            let away = (head - target).with_y(0.0).normalize_or(Vec3::X);
            let goal = head.with_y(CRUISE_ALT + 0.5) + away * 2.0;
            head = toward(head, goal, 6.0 * speed_mult);
            if head.y > CRUISE_ALT - 0.3 {
                bat.phase = Phase::Circling;
                bat.timer = rand::rng().random_range(2.5..5.0);
                bat.angle = (head.z - target.z).atan2(head.x - target.x);
            }
        }
        Phase::Stunned => {
            if bat.timer <= 0.0 {
                bat.phase = Phase::Climb;
            }
        }
    }
    head.x = head.x.clamp(-ARENA_HALF, ARENA_HALF);
    head.z = head.z.clamp(-ARENA_HALF, ARENA_HALF);
    (head, hit)
}

pub fn bat_behavior(
    mut commands: Commands,
    mut bats: Query<(&mut Bat, &mut Enemy, &mut Transform), Without<Player>>,
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
    // Aim for the chest, a little below eye level.
    let target = ptf.translation - Vec3::Y * (EYE_HEIGHT * 0.35);
    for (mut bat, mut enemy, mut tf) in &mut bats {
        // `move_and_attack` leaves frozen enemies alone; bats move themselves.
        enemy.frozen = true;
        let lift = HEAD * tf.scale.y;
        let mut head = tf.translation + Vec3::Y * lift;
        if !bat.launched {
            bat.launched = true;
            head.y = 1.5;
        }

        let before = bat.phase;
        let slow = if enemy.slow > 0.0 { 0.5 } else { 1.0 };
        let (new_head, hit) = step(&mut bat, head, target, slow, dt, |h| obstacles.contains(h, 0.15));
        head = new_head;

        // Reactions to phase changes
        match (before, bat.phase) {
            (Phase::Circling, Phase::WindUp) => sound::play_with_speed(&mut commands, &sfx.groan, 0.5, 2.4),
            (Phase::Dive(_), Phase::Stunned) => {
                effects::burst(&mut commands, &fx, head, 10, &[fx.straw.clone(), fx.pumpkin.clone()], 4.0);
                sound::play_at(&mut commands, &sfx.splat, head, ptf.translation, 0.6);
            }
            _ => {}
        }
        if hit {
            p.take_damage(enemy.damage, head);
            hurt.0 = 0.35;
            shake.add(0.2);
            sound::play(&mut commands, &sfx.hurt, 0.8);
        }

        // Flap speed and stun state
        let flap = match bat.phase {
            Phase::WindUp => 40.0,
            Phase::Dive(_) => 4.0,
            Phase::Stunned => 0.0,
            _ => 22.0,
        };
        enemy.walk_t += dt * flap;
        let stunned = bat.phase == Phase::Stunned;
        enemy.vulnerable = if stunned { 1.5 } else { 1.0 };

        // Face where it's going, or at you while winding up.
        let look = match bat.phase {
            Phase::Dive(v) => v,
            Phase::WindUp | Phase::Stunned | Phase::Circling => target - head,
            Phase::Climb => (head - target).with_y(0.0),
        };
        if look.xz().length_squared() > 1e-4 {
            let yaw = Quat::from_rotation_y((-look.x).atan2(-look.z));
            let tilt = if matches!(bat.phase, Phase::Dive(_)) { Quat::from_rotation_x(-0.6) } else { Quat::IDENTITY };
            let want = yaw * tilt;
            tf.rotation = tf.rotation.slerp(want, (10.0 * dt).min(1.0));
        }
        tf.translation = head - Vec3::Y * lift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(bat: &mut Bat, mut head: Vec3, target: Vec3, secs: f32, blocked: impl Fn(Vec3) -> bool + Copy) -> (Vec3, bool) {
        let mut any_hit = false;
        let dt = 1.0 / 60.0;
        for _ in 0..(secs / dt) as usize {
            let (h, hit) = step(bat, head, target, 1.0, dt, blocked);
            head = h;
            any_hit |= hit;
        }
        (head, any_hit)
    }

    #[test]
    fn climbs_then_circles_overhead() {
        let mut bat = Bat::default();
        let (head, _) = run(&mut bat, Vec3::new(20.0, 1.5, 0.0), Vec3::new(0.0, 1.1, 0.0), 1.2, |_| false);
        assert!(head.y > 3.5, "should be up at cruising height, got {head:?}");
        assert!(matches!(bat.phase, Phase::Circling | Phase::WindUp | Phase::Dive(_)));
    }

    #[test]
    fn dive_hits_a_player_who_stands_still() {
        let mut bat = Bat { phase: Phase::WindUp, timer: 0.0, ..Bat::default() };
        let (_, hit) = run(&mut bat, Vec3::new(6.0, 4.5, 0.0), Vec3::new(0.0, 1.1, 0.0), 1.0, |_| false);
        assert!(hit, "a dive at a stationary player should connect");
        assert_eq!(bat.phase, Phase::Climb);
    }

    #[test]
    fn dive_that_misses_crashes_and_stuns() {
        // Wind up aiming at a spot, then the "player" isn't there: the bat hits the ground.
        let mut bat = Bat { phase: Phase::WindUp, timer: 0.0, ..Bat::default() };
        let (head, _) = step(&mut bat, Vec3::new(6.0, 4.5, 0.0), Vec3::new(0.0, 0.0, 0.0), 1.0, 0.016, |_| false);
        let moved_away = Vec3::new(30.0, 1.1, 30.0);
        let mut h = head;
        let mut stunned = false;
        for _ in 0..120 {
            // The dive direction is locked in; the target moving doesn't steer it.
            let (nh, hit) = step(&mut bat, h, moved_away, 1.0, 1.0 / 60.0, |_| false);
            assert!(!hit);
            h = nh;
            if bat.phase == Phase::Stunned {
                stunned = true;
                break;
            }
        }
        assert!(stunned, "should crash into the ground, ended at {h:?} in {:?}", bat.phase);
        assert!(h.y >= 0.45);
    }

    #[test]
    fn props_stop_a_dive() {
        let mut bat = Bat { phase: Phase::Dive(Vec3::new(-16.0, 0.0, 0.0)), timer: 1.0, ..Bat::default() };
        let (_, _) = step(&mut bat, Vec3::new(2.0, 2.0, 0.0), Vec3::new(-10.0, 2.0, 0.0), 1.0, 0.1, |h| h.x < 1.0);
        assert_eq!(bat.phase, Phase::Stunned);
    }
}

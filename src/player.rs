//! First-person player: mouse look, WASD movement, sprint, jump, health,
//! screen shake, and mouse locking / pausing.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::collision::Obstacles;
use crate::powerups::PowerUps;
use crate::settings::Settings;
use crate::weapons::Arsenal;
use crate::festival::ARENA_HALF;

pub const EYE_HEIGHT: f32 = 1.7;
pub const MAX_HEALTH: i32 = 100;
const MOVE_SPEED: f32 = 7.0;
const SPRINT_MULT: f32 = 1.6;
const JUMP_SPEED: f32 = 7.0;
const GRAVITY: f32 = 20.0;
const MOUSE_SENS: f32 = 0.0025;
/// Biggest camera rotation from shake at full trauma, in radians.
const SHAKE_ANGLE: f32 = 0.07;
const RADIUS: f32 = 0.4;

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub health: i32,
    pub max_health: i32,
    /// Fraction of incoming damage blocked (from the Armor upgrade).
    pub armor: f32,
    /// Difficulty scaling on damage taken (Easy < 1, Hard > 1). Kept in sync with settings.
    pub damage_taken: f32,
    pub moving: bool,
    vy: f32,
    /// Damage taken since the HUD last looked: (where it came from, amount).
    /// Drives the damage direction indicator and screen shake.
    pub hits: Vec<(Vec3, i32)>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            health: MAX_HEALTH,
            max_health: MAX_HEALTH,
            armor: 0.0,
            damage_taken: 1.0,
            moving: false,
            vy: 0.0,
            hits: Vec::new(),
        }
    }

    /// Take a hit from something at `from` (a world position).
    pub fn take_damage(&mut self, dmg: i32, from: Vec3) {
        if self.damage_taken <= 0.0 {
            // God mode: still show where the hit came from.
            self.hits.push((from, 0));
            return;
        }
        let reduced = (dmg as f32 * self.damage_taken * (1.0 - self.armor)).round().max(1.0) as i32;
        self.health -= reduced;
        self.hits.push((from, reduced));
    }
}

/// Screen shake. Add "trauma" (0-1) on impacts; the camera shakes by trauma squared,
/// so small bumps stay subtle and big ones really rattle. Trauma fades over about a second.
#[derive(Resource, Default)]
pub struct Shake(pub f32);

impl Shake {
    pub fn add(&mut self, amount: f32) {
        self.0 = (self.0 + amount).min(1.0);
    }

    /// Add shake from something happening at `at`, weaker the farther it is from `listener`.
    pub fn add_at(&mut self, amount: f32, at: Vec3, listener: Vec3) {
        self.add(amount * (1.0 - at.distance(listener) / 25.0).clamp(0.0, 1.0));
    }
}

/// Seconds left on the red damage flash.
#[derive(Resource, Default)]
pub struct HurtFlash(pub f32);

pub fn spawn_player(mut commands: Commands, settings: Res<Settings>) {
    commands.spawn((
        Player::new(),
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: settings.fov.to_radians(),
            near: 0.05,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.04, 0.03, 0.07),
            falloff: FogFalloff::Linear { start: 14.0, end: 60.0 },
            ..default()
        },
        Transform::from_xyz(0.0, EYE_HEIGHT, 10.0),
    ));
}

/// Esc toggles the pause menu. While playing, a click re-locks the mouse if it got
/// freed some other way (alt-tab). Menu buttons handle everything else; see `settings.rs`.
pub fn grab_cursor(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    shop: Res<crate::shop::ShopScreen>,
    mut time: ResMut<Time<Virtual>>,
) {
    // The shop handles its own Esc and mouse.
    if *state.get() != GameState::Playing || shop.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        if time.is_paused() {
            time.unpause();
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        } else {
            time.pause();
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    } else if mouse.just_pressed(MouseButton::Left) && !time.is_paused() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

pub fn look_and_move(
    mut player: Single<(&mut Player, &mut Transform)>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    obstacles: Res<Obstacles>,
    power: Res<PowerUps>,
    settings: Res<Settings>,
    arsenal: Res<Arsenal>,
    god: Res<crate::save::GodMode>,
    mut shake: ResMut<Shake>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let (ref mut p, ref mut tf) = *player;
    p.damage_taken = if god.0 { 0.0 } else { settings.damage_taken_mult() };

    if cursor.grab_mode != CursorGrabMode::None && dt > 0.0 {
        // Turn slower while zoomed in so aiming stays steady.
        let sens = MOUSE_SENS * settings.sensitivity / arsenal.zoom();
        let invert = if settings.invert_y { -1.0 } else { 1.0 };
        p.yaw -= motion.delta.x * sens;
        p.pitch = (p.pitch - motion.delta.y * sens * invert).clamp(-1.45, 1.45);
    }

    // Shake wobbles the view on top of where you're aiming, without moving your aim.
    shake.0 = (shake.0 - dt * 1.4).max(0.0);
    let s = shake.0 * shake.0 * settings.shake_amount() * SHAKE_ANGLE;
    let t = time.elapsed_secs();
    let wobble = |freq: f32, phase: f32| (t * freq + phase).sin() * 0.6 + (t * freq * 2.3 + phase).sin() * 0.4;
    tf.rotation = Quat::from_euler(
        EulerRot::YXZ,
        p.yaw + s * wobble(31.0, 0.0),
        p.pitch + s * wobble(27.0, 1.7),
        s * 0.6 * wobble(23.0, 3.1),
    );

    // Move on the ground plane only, regardless of where we're looking.
    let forward = Vec3::new(-p.yaw.sin(), 0.0, -p.yaw.cos());
    let right = Vec3::new(p.yaw.cos(), 0.0, -p.yaw.sin());
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    // Sprinting doesn't work while aiming; aiming slows you to 60%.
    let sprint = keys.pressed(KeyCode::ShiftLeft) && arsenal.aim < 0.3;
    let base = if sprint { MOVE_SPEED * SPRINT_MULT } else { MOVE_SPEED };
    let speed = power.speed_mult() * base * (1.0 - 0.4 * arsenal.aim);
    p.moving = dir != Vec3::ZERO;

    // Move in small steps so a slow frame can't carry you through a thin prop.
    let total = dir.normalize_or_zero() * speed * dt;
    let steps = (total.length() / 0.15).ceil().max(1.0) as usize;
    let mut feet = tf.translation - Vec3::Y * EYE_HEIGHT;
    let mut ground = 0.0;
    for _ in 0..steps {
        feet += total / steps as f32;
        feet.x = feet.x.clamp(-ARENA_HALF, ARENA_HALF);
        feet.z = feet.z.clamp(-ARENA_HALF, ARENA_HALF);
        // Bump into props, or stand on top of them.
        (feet, ground) = obstacles.resolve(feet, RADIUS, 1.8);
    }
    tf.translation.x = feet.x;
    tf.translation.z = feet.z;

    // Jump and fall
    let grounded = feet.y <= ground + 0.001;
    if grounded && keys.just_pressed(KeyCode::Space) {
        p.vy = JUMP_SPEED;
    }
    p.vy -= GRAVITY * dt;
    let mut y = feet.y + p.vy * dt;
    if y < ground {
        y = ground;
        p.vy = 0.0;
    }
    tf.translation.y = y + EYE_HEIGHT;
}

pub fn check_death(
    player: Single<&Player>,
    mut next: ResMut<NextState<GameState>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if player.health <= 0 {
        next.set(GameState::GameOver);
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

//! Short-lived visual effects: flying chunks, bullet tracers, and light flashes.
//! Shared meshes and materials live in `Fx` so effects don't create new assets every shot.

use bevy::prelude::*;
use rand::Rng;

#[derive(Resource)]
pub struct Fx {
    pub cube: Handle<Mesh>,
    pub ball: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub disc: Handle<Mesh>,
    pub pumpkin: Handle<StandardMaterial>,
    pub straw: Handle<StandardMaterial>,
    pub seed: Handle<StandardMaterial>,
    pub tracer: Handle<StandardMaterial>,
    pub pants: Handle<StandardMaterial>,
    pub hands: Handle<StandardMaterial>,
    pub stem: Handle<StandardMaterial>,
    pub candy: Handle<StandardMaterial>,
    pub black: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub wisp: Handle<StandardMaterial>,
    pub power_damage: Handle<StandardMaterial>,
    pub power_speed: Handle<StandardMaterial>,
    pub power_magnet: Handle<StandardMaterial>,
}

pub fn setup_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mat = |base: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: base,
            emissive,
            perceptual_roughness: 0.8,
            ..default()
        })
    };
    commands.insert_resource(Fx {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        ball: meshes.add(Sphere::new(1.0)),
        ring: meshes.add(Torus::new(0.9, 1.0)),
        disc: meshes.add(Circle::new(1.0)),
        pumpkin: mat(Color::srgb(0.95, 0.45, 0.08), LinearRgba::rgb(0.6, 0.2, 0.0)),
        straw: mat(Color::srgb(0.85, 0.72, 0.35), LinearRgba::BLACK),
        seed: mat(Color::srgb(0.9, 0.85, 0.6), LinearRgba::rgb(3.0, 1.5, 0.2)),
        tracer: mat(Color::BLACK, LinearRgba::rgb(8.0, 5.0, 1.5)),
        pants: mat(Color::srgb(0.18, 0.22, 0.35), LinearRgba::BLACK),
        hands: mat(Color::srgb(0.78, 0.66, 0.38), LinearRgba::BLACK),
        stem: mat(Color::srgb(0.20, 0.35, 0.10), LinearRgba::BLACK),
        candy: mat(Color::srgb(0.9, 0.1, 0.25), LinearRgba::rgb(1.5, 0.1, 0.25)),
        black: mat(Color::srgb(0.04, 0.04, 0.05), LinearRgba::BLACK),
        metal: mat(Color::srgb(0.45, 0.47, 0.5), LinearRgba::BLACK),
        wisp: mat(Color::srgb(0.6, 0.3, 0.9), LinearRgba::rgb(3.0, 0.8, 6.0)),
        power_damage: mat(Color::srgb(1.0, 0.2, 0.15), LinearRgba::rgb(6.0, 0.6, 0.4)),
        power_speed: mat(Color::srgb(0.2, 0.6, 1.0), LinearRgba::rgb(0.5, 2.5, 7.0)),
        power_magnet: mat(Color::srgb(1.0, 0.8, 0.2), LinearRgba::rgb(6.0, 4.5, 0.6)),
    });
}

#[derive(Component)]
pub struct Chunk {
    vel: Vec3,
    life: f32,
}

/// A splat of pumpkin goo on the ground. Shrinks away over its last few seconds.
#[derive(Component)]
pub struct Goo {
    life: f32,
    size: f32,
}

const GOO_LIFE: f32 = 30.0;
/// Most splats on the ground at once; the oldest go first.
const MAX_GOO: usize = 90;

/// A main splat plus a few droplets, flat on the ground under `at`.
/// `mat` is the dead pumpkin's own skin, so ghost pumpkins leave pale goo.
pub fn goo(commands: &mut Commands, fx: &Fx, mat: &Handle<StandardMaterial>, at: Vec3, size: f32) {
    let mut rng = rand::rng();
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let mut splat = |p: Vec2, s: f32, stretch: f32| {
        // Tiny height offsets so overlapping splats don't flicker against each other.
        let y = 0.012 + rng.random_range(0.0..0.006);
        let yaw = Quat::from_rotation_y(rng.random_range(0.0..std::f32::consts::TAU));
        commands.spawn((
            Goo { life: GOO_LIFE, size: s },
            Mesh3d(fx.disc.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(p.x, y, p.y).with_rotation(yaw * flat).with_scale(Vec3::new(s * stretch, s, 1.0)),
        ));
    };
    let center = at.xz();
    splat(center, size * rand::rng().random_range(0.8..1.2), rand::rng().random_range(1.0..1.5));
    for _ in 0..rand::rng().random_range(3..7) {
        let a = rand::rng().random_range(0.0..std::f32::consts::TAU);
        let d = size * rand::rng().random_range(0.9..2.2);
        let s = size * rand::rng().random_range(0.15..0.35);
        splat(center + Vec2::new(a.cos(), a.sin()) * d, s, 1.0);
    }
    // Some straw bits and seeds left in the mess.
    for _ in 0..4 {
        let off = Vec2::new(rand::rng().random_range(-1.0..1.0), rand::rng().random_range(-1.0..1.0)) * size;
        let p = center + off;
        commands.spawn((
            Goo { life: GOO_LIFE, size: 0.06 },
            Mesh3d(fx.cube.clone()),
            MeshMaterial3d(fx.straw.clone()),
            Transform::from_xyz(p.x, 0.03, p.y)
                .with_rotation(Quat::from_rotation_y(rand::rng().random_range(0.0..3.0)))
                .with_scale(Vec3::new(0.18, 0.03, 0.04)),
        ));
    }
}

/// Despawns after `life` seconds. Used for tracers.
#[derive(Component)]
pub struct Fade(pub f32);

/// A point light that fades out and despawns.
#[derive(Component)]
pub struct LightFlash {
    life: f32,
    max: f32,
    intensity: f32,
}

pub fn burst(commands: &mut Commands, fx: &Fx, at: Vec3, count: usize, mats: &[Handle<StandardMaterial>], power: f32) {
    let mut rng = rand::rng();
    for i in 0..count {
        let vel = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(0.5..1.6),
            rng.random_range(-1.0..1.0),
        ) * power;
        commands.spawn((
            Chunk { vel, life: rng.random_range(0.8..1.6) },
            Mesh3d(fx.cube.clone()),
            MeshMaterial3d(mats[i % mats.len()].clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(rng.random_range(0.08..0.22))),
        ));
    }
}

pub fn tracer(commands: &mut Commands, fx: &Fx, from: Vec3, to: Vec3) {
    let len = from.distance(to);
    if len < 0.01 {
        return;
    }
    commands.spawn((
        Fade(0.04),
        Mesh3d(fx.cube.clone()),
        MeshMaterial3d(fx.tracer.clone()),
        Transform::from_translation((from + to) / 2.0)
            .looking_at(to, Vec3::Y)
            .with_scale(Vec3::new(0.025, 0.025, len)),
    ));
}

pub fn light_flash(commands: &mut Commands, at: Vec3, color: Color, intensity: f32, range: f32, life: f32) {
    commands.spawn((
        LightFlash { life, max: life, intensity },
        PointLight {
            color,
            intensity,
            range,
            ..default()
        },
        Transform::from_translation(at),
    ));
}

pub fn update_goo(mut commands: Commands, mut goo: Query<(Entity, &mut Goo, &mut Transform)>, time: Res<Time>) {
    let dt = time.delta_secs();
    let mut alive: Vec<(f32, Entity)> = Vec::new();
    for (e, mut g, mut tf) in &mut goo {
        g.life -= dt;
        if g.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Shrink away over the last 4 seconds.
        if g.life < 4.0 && g.size > 0.07 {
            let k = (g.life / 4.0).max(0.01);
            let stretch = tf.scale.x / tf.scale.y.max(1e-4);
            tf.scale = Vec3::new(g.size * k * stretch, g.size * k, 1.0);
        }
        alive.push((g.life, e));
    }
    if alive.len() > MAX_GOO {
        alive.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(_, e) in &alive[..alive.len() - MAX_GOO] {
            commands.entity(e).despawn();
        }
    }
}

pub fn update_effects(
    mut commands: Commands,
    mut chunks: Query<(Entity, &mut Chunk, &mut Transform)>,
    mut fades: Query<(Entity, &mut Fade)>,
    mut flashes: Query<(Entity, &mut LightFlash, &mut PointLight)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut c, mut tf) in &mut chunks {
        c.life -= dt;
        if c.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        c.vel.y -= 18.0 * dt;
        tf.translation += c.vel * dt;
        if tf.translation.y < 0.05 {
            tf.translation.y = 0.05;
            c.vel.y *= -0.3;
            c.vel.x *= 0.6;
            c.vel.z *= 0.6;
        }
        tf.rotate_x(6.0 * dt);
    }
    for (e, mut f) in &mut fades {
        f.0 -= dt;
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    for (e, mut f, mut light) in &mut flashes {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        light.intensity = f.intensity * (f.life / f.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world_with_fx() -> World {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.insert_resource(Time::<()>::default());
        world.run_system_once(setup_fx).unwrap();
        world
    }

    #[test]
    fn a_kill_leaves_a_splat_and_droplets_on_the_ground() {
        let mut world = world_with_fx();
        world
            .run_system_once(|mut commands: Commands, fx: Res<Fx>| {
                let mat = fx.pumpkin.clone();
                goo(&mut commands, &fx, &mat, Vec3::new(3.0, 1.7, -2.0), 0.55);
            })
            .unwrap();
        let mut q = world.query::<(&Goo, &Transform)>();
        let splats: Vec<_> = q.iter(&world).collect();
        // One main splat, 3-6 droplets, 4 straw bits.
        assert!((8..=11).contains(&splats.len()), "got {}", splats.len());
        assert!(splats.iter().all(|(_, tf)| tf.translation.y < 0.05), "flat on the ground");
        assert!(splats.iter().all(|(_, tf)| tf.translation.xz().distance(Vec2::new(3.0, -2.0)) < 2.0));
    }

    #[test]
    fn old_goo_is_cleaned_up_past_the_cap() {
        let mut world = world_with_fx();
        for _ in 0..20 {
            world
                .run_system_once(|mut commands: Commands, fx: Res<Fx>| {
                    let mat = fx.pumpkin.clone();
                    goo(&mut commands, &fx, &mat, Vec3::ZERO, 0.5);
                })
                .unwrap();
        }
        world.run_system_once(update_goo).unwrap();
        let count = world.query::<&Goo>().iter(&world).count();
        assert_eq!(count, MAX_GOO);
    }
}

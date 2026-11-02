//! The festival grounds: ground, corn-maze boundary, hay bales, booths, string lights.
//! Everything is built from primitive shapes for now. To swap in real assets later,
//! replace a `spawn_*` helper with a `SceneRoot(asset_server.load("models/x.glb#Scene0"))`.
//! Solid props are also registered as collision boxes in `Obstacles`.

use bevy::prelude::*;

use crate::collision::{Box3, Obstacles};

/// Spawn a box-shaped prop and register it for collision.
fn solid(
    commands: &mut Commands,
    obstacles: &mut Vec<Box3>,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    tf: Transform,
    size: Vec3,
) {
    obstacles.push(Box3::from_transform(&tf, size));
    commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), tf));
}

/// Half-width of the square play area. The corn wall sits just outside this.
pub const ARENA_HALF: f32 = 30.0;

pub fn setup_festival(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ARENA_HALF * 2.0 + 10.0, ARENA_HALF * 2.0 + 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.16, 0.07),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));

    // Moonlight
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.55, 0.6, 0.85),
            illuminance: 1_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-20.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    spawn_corn_wall(&mut commands, &mut meshes, &mut materials);
    let mut obstacles = Vec::new();

    // Hay bales scattered around for cover
    let hay = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.62, 0.28),
        perceptual_roughness: 1.0,
        ..default()
    });
    let bale_size = Vec3::new(1.6, 0.9, 0.9);
    let bale = meshes.add(Cuboid::from_size(bale_size));
    for &(x, z, rot) in &[
        (6.0, 4.0, 0.3),
        (-8.0, 7.0, -0.5),
        (10.0, -9.0, 1.2),
        (-12.0, -6.0, 0.1),
        (2.0, -14.0, 0.8),
        (-3.0, 15.0, -1.0),
        (16.0, 12.0, 0.4),
        (-18.0, 16.0, 0.9),
    ] {
        let tf = Transform::from_xyz(x, 0.45, z).with_rotation(Quat::from_rotation_y(rot));
        solid(&mut commands, &mut obstacles, &bale, &hay, tf, bale_size);
    }

    // Stacks you can jump up: one bale, then a second one on top beside it.
    for &(x, z, rot) in &[(-6.0, -2.0, 0.0), (12.0, 3.0, 1.57), (-14.0, 22.0, 0.4)] {
        let r = Quat::from_rotation_y(rot);
        for (offset, y) in [(Vec3::ZERO, 0.45), (Vec3::new(0.0, 0.0, 0.9), 0.45), (Vec3::new(0.0, 0.0, 0.9), 1.35)] {
            let tf = Transform::from_translation(Vec3::new(x, y, z) + r * offset).with_rotation(r);
            solid(&mut commands, &mut obstacles, &bale, &hay, tf, bale_size);
        }
    }

    // Booths along the north side
    let booth_colors = [
        Color::srgb(0.55, 0.12, 0.10),
        Color::srgb(0.25, 0.12, 0.40),
        Color::srgb(0.85, 0.40, 0.08),
    ];
    for (i, &x) in [-14.0, -2.0, 10.0].iter().enumerate() {
        spawn_booth(
            &mut commands,
            &mut obstacles,
            &mut meshes,
            &mut materials,
            Vec3::new(x, 0.0, -24.0),
            booth_colors[i],
        );
    }

    spawn_string_lights(&mut commands, &mut meshes, &mut materials);
    spawn_lamps_and_graves(&mut commands, &mut obstacles, &mut meshes, &mut materials);
    commands.insert_resource(Obstacles(obstacles));
}

/// Lamp posts so you can see what's coming, plus a little graveyard in one corner.
fn spawn_lamps_and_graves(
    commands: &mut Commands,
    obstacles: &mut Vec<Box3>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let iron = materials.add(StandardMaterial { base_color: Color::srgb(0.1, 0.1, 0.12), ..default() });
    let lamp_glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(4.0, 2.4, 0.8),
        ..default()
    });
    let pole = meshes.add(Cylinder::new(0.08, 4.0));
    let bulb = meshes.add(Sphere::new(0.25));
    for (x, z) in [(-16.0, -12.0), (16.0, -12.0), (-16.0, 14.0), (16.0, 14.0), (0.0, 0.0)] {
        commands.spawn((Mesh3d(pole.clone()), MeshMaterial3d(iron.clone()), Transform::from_xyz(x, 2.0, z)));
        obstacles.push(Box3::from_transform(&Transform::from_xyz(x, 2.0, z), Vec3::new(0.2, 4.0, 0.2)));
        commands.spawn((Mesh3d(bulb.clone()), MeshMaterial3d(lamp_glow.clone()), Transform::from_xyz(x, 4.1, z)));
        commands.spawn((
            PointLight { color: Color::srgb(1.0, 0.75, 0.45), intensity: 250_000.0, range: 16.0, ..default() },
            Transform::from_xyz(x, 3.9, z),
        ));
    }

    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.38, 0.4),
        perceptual_roughness: 1.0,
        ..default()
    });
    let (slab_size, v_size, h_size) = (Vec3::new(0.7, 1.0, 0.18), Vec3::new(0.15, 1.2, 0.15), Vec3::new(0.6, 0.15, 0.15));
    let slab = meshes.add(Cuboid::from_size(slab_size));
    let cross_v = meshes.add(Cuboid::from_size(v_size));
    let cross_h = meshes.add(Cuboid::from_size(h_size));
    for row in 0..3 {
        for col in 0..4 {
            let x = 18.0 + col as f32 * 2.2;
            let z = -26.0 + row as f32 * 2.5;
            let tilt = Quat::from_rotation_z(((row * 4 + col) as f32 * 1.7).sin() * 0.12);
            if (row + col) % 3 == 0 {
                solid(commands, obstacles, &cross_v, &stone, Transform::from_xyz(x, 0.6, z).with_rotation(tilt), v_size);
                solid(commands, obstacles, &cross_h, &stone, Transform::from_xyz(x, 0.85, z).with_rotation(tilt), h_size);
            } else {
                solid(commands, obstacles, &slab, &stone, Transform::from_xyz(x, 0.5, z).with_rotation(tilt), slab_size);
            }
        }
    }
}

fn spawn_corn_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let corn = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.50, 0.22),
        perceptual_roughness: 1.0,
        ..default()
    });
    let len = ARENA_HALF * 2.0 + 2.0;
    let wall = meshes.add(Cuboid::new(len, 3.0, 1.0));
    let edge = ARENA_HALF + 1.0;
    for (pos, rot) in [
        (Vec3::new(0.0, 1.5, edge), 0.0),
        (Vec3::new(0.0, 1.5, -edge), 0.0),
        (Vec3::new(edge, 1.5, 0.0), std::f32::consts::FRAC_PI_2),
        (Vec3::new(-edge, 1.5, 0.0), std::f32::consts::FRAC_PI_2),
    ] {
        commands.spawn((
            Mesh3d(wall.clone()),
            MeshMaterial3d(corn.clone()),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(rot)),
        ));
    }
}

fn spawn_booth(
    commands: &mut Commands,
    obstacles: &mut Vec<Box3>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    awning: Color,
) {
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.12),
        perceptual_roughness: 0.9,
        ..default()
    });
    let roof = materials.add(StandardMaterial {
        base_color: awning,
        perceptual_roughness: 0.8,
        ..default()
    });
    // Counter
    let counter_size = Vec3::new(4.0, 1.1, 1.2);
    let counter = meshes.add(Cuboid::from_size(counter_size));
    solid(commands, obstacles, &counter, &wood, Transform::from_translation(at + Vec3::new(0.0, 0.55, 0.0)), counter_size);
    // Posts
    let post_size = Vec3::new(0.15, 2.8, 0.15);
    let post = meshes.add(Cuboid::from_size(post_size));
    for dx in [-1.9, 1.9] {
        solid(commands, obstacles, &post, &wood, Transform::from_translation(at + Vec3::new(dx, 1.4, 0.5)), post_size);
    }
    // Awning
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(4.4, 0.15, 2.0))),
        MeshMaterial3d(roof),
        Transform::from_translation(at + Vec3::new(0.0, 2.85, 0.2))
            .with_rotation(Quat::from_rotation_x(-0.2)),
    ));
    // A warm light under each booth
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.65, 0.3),
            intensity: 60_000.0,
            range: 10.0,
            ..default()
        },
        Transform::from_translation(at + Vec3::new(0.0, 2.3, 1.0)),
    ));
}

fn spawn_string_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let bulb = meshes.add(Sphere::new(0.08));
    let colors = [
        LinearRgba::rgb(2.5, 0.7, 0.05),
        LinearRgba::rgb(1.2, 0.1, 2.2),
        LinearRgba::rgb(0.15, 2.0, 0.2),
    ];
    let mats: Vec<_> = colors
        .iter()
        .map(|&c| {
            materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: c,
                ..default()
            })
        })
        .collect();

    // Two strands crossing the middle of the festival, sagging a little.
    for strand in 0..2 {
        for i in 0..40 {
            let t = i as f32 / 39.0;
            let along = -ARENA_HALF + t * ARENA_HALF * 2.0;
            let sag = 4.5 - 1.2 * (1.0 - (2.0 * t - 1.0).powi(2));
            let pos = if strand == 0 {
                Vec3::new(along, sag, along * 0.3)
            } else {
                Vec3::new(along * 0.3, sag, -along)
            };
            commands.spawn((
                Mesh3d(bulb.clone()),
                MeshMaterial3d(mats[i % mats.len()].clone()),
                Transform::from_translation(pos),
            ));
        }
    }
}

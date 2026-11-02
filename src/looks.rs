//! How each scarecrow looks: pumpkin color and shape, carved face, and hat.
//! Regular enemies roll a random look so no two match; bosses always look the same.
//!
//! Shirt color stays tied to enemy type (red Walker, green Runner, ...) so you can still
//! tell them apart at a glance; only its brightness varies.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use rand::Rng;

use crate::effects::Fx;
use crate::enemies::Kind;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hat {
    None,
    Straw,
    Top,
    Bucket,
    Witch,
    Crown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Eyes {
    Diamond,
    Angry,
    Round,
    Mismatched,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mouth {
    Grin,
    Zigzag,
    Shocked,
    Smile,
}

pub struct Look {
    pub skin: Color,
    pub head_scale: Vec3,
    pub eyes: Eyes,
    pub mouth: Mouth,
    pub hat: Hat,
    pub shirt: Color,
    pub stem_tilt: f32,
}

/// Base color of the glow inside the carved face, before distance brightening.
pub fn face_glow(kind: Kind) -> Vec3 {
    match kind {
        Kind::King => Vec3::new(0.1, 1.0, 0.12),
        Kind::Witch => Vec3::new(0.7, 0.15, 1.0),
        Kind::Golem | Kind::Gourdling => Vec3::new(1.0, 0.12, 0.03),
        Kind::Bat => Vec3::new(1.0, 0.2, 0.05),
        _ => Vec3::new(1.0, 0.4, 0.03),
    }
}

pub fn roll(kind: Kind, base_shirt: Color, rng: &mut impl Rng) -> Look {
    let jitter = rng.random_range(0.75..1.25);
    let c = base_shirt.to_srgba();
    let shirt = Color::srgb(c.red * jitter, c.green * jitter, c.blue * jitter);
    match kind {
        Kind::King => Look {
            skin: Color::srgb(0.95, 0.42, 0.05),
            head_scale: Vec3::new(1.15, 0.9, 1.15),
            eyes: Eyes::Diamond,
            mouth: Mouth::Grin,
            hat: Hat::Crown,
            shirt: base_shirt,
            stem_tilt: 0.0,
        },
        Kind::Witch => Look {
            skin: Color::srgb(0.92, 0.9, 0.82), // pale "ghost" pumpkin
            head_scale: Vec3::new(1.0, 1.05, 1.0),
            eyes: Eyes::Angry,
            mouth: Mouth::Zigzag,
            hat: Hat::Witch,
            shirt: base_shirt,
            stem_tilt: 0.0,
        },
        Kind::Bat => Look {
            skin: [Color::srgb(0.95, 0.42, 0.05), Color::srgb(0.88, 0.34, 0.04), Color::srgb(0.9, 0.88, 0.8)][rng.random_range(0..3)],
            head_scale: Vec3::new(1.0, 0.9, 1.0),
            eyes: Eyes::Angry,
            mouth: Mouth::Zigzag, // fangs
            hat: Hat::None,
            shirt: base_shirt,
            stem_tilt: rng.random_range(-0.4..0.4),
        },
        Kind::Golem | Kind::Gourdling => Look {
            skin: Color::srgb(0.3, 0.45, 0.12), // warty green gourd
            head_scale: Vec3::new(1.4, 0.95, 1.3),
            eyes: Eyes::Angry,
            mouth: Mouth::Smile,
            hat: Hat::None,
            shirt: base_shirt,
            stem_tilt: 0.6,
        },
        _ => {
            let skin = match rng.random_range(0..100) {
                0..8 => Color::srgb(0.9, 0.88, 0.8),   // ghost pumpkin
                8..14 => Color::srgb(0.35, 0.5, 0.15), // unripe green
                n => [
                    Color::srgb(0.95, 0.42, 0.05),
                    Color::srgb(0.88, 0.34, 0.04),
                    Color::srgb(1.0, 0.52, 0.12),
                    Color::srgb(0.82, 0.45, 0.12),
                ][n as usize % 4],
            };
            let hat = if kind == Kind::Lobber {
                Hat::Witch
            } else {
                match rng.random_range(0..10) {
                    0..4 => Hat::None,
                    4..7 => Hat::Straw,
                    7..9 => Hat::Top,
                    _ => Hat::Bucket,
                }
            };
            Look {
                skin,
                head_scale: Vec3::new(rng.random_range(1.0..1.3), rng.random_range(0.8..1.0), rng.random_range(1.0..1.2)),
                eyes: [Eyes::Diamond, Eyes::Angry, Eyes::Round, Eyes::Mismatched][rng.random_range(0..4)],
                mouth: [Mouth::Grin, Mouth::Zigzag, Mouth::Shocked, Mouth::Smile][rng.random_range(0..4)],
                hat,
                shirt,
                stem_tilt: rng.random_range(-0.5..0.5),
            }
        }
    }
}

const HEAD_Y: f32 = 1.85;
const HEAD_R: f32 = 0.42;

/// Spawns the pumpkin head, carved face, stem, and hat as children of an enemy.
pub fn spawn_head(
    p: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fx: &Fx,
    look: &Look,
    skin: &Handle<StandardMaterial>,
    face: &Handle<StandardMaterial>,
    shirt: &Handle<StandardMaterial>,
) {
    let hs = look.head_scale;
    p.spawn((
        Mesh3d(fx.ball.clone()),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.0, HEAD_Y, 0.0).with_scale(hs * HEAD_R),
    ));

    // Face parts sit just in front of the head's surface; x and y spread with the head's shape.
    let front = -HEAD_R * hs.z - 0.04;
    let at = |x: f32, y: f32| Vec3::new(x * hs.x, HEAD_Y + y * hs.y, front);
    let mut carve = |pos: Vec3, scale: Vec3, rot: f32, round: bool| {
        p.spawn((
            Mesh3d(if round { fx.ball.clone() } else { fx.cube.clone() }),
            MeshMaterial3d(face.clone()),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_z(rot)).with_scale(scale),
        ));
    };

    match look.eyes {
        Eyes::Diamond => {
            for x in [-0.17, 0.17] {
                carve(at(x, 0.1), Vec3::new(0.14, 0.14, 0.05), 0.785, false);
            }
        }
        Eyes::Angry => {
            // Slanted down toward the middle
            carve(at(-0.17, 0.12), Vec3::new(0.19, 0.08, 0.05), -0.45, false);
            carve(at(0.17, 0.12), Vec3::new(0.19, 0.08, 0.05), 0.45, false);
        }
        Eyes::Round => {
            for x in [-0.17, 0.17] {
                carve(at(x, 0.1), Vec3::new(0.08, 0.08, 0.03), 0.0, true);
            }
        }
        Eyes::Mismatched => {
            carve(at(-0.17, 0.11), Vec3::new(0.1, 0.1, 0.03), 0.0, true);
            carve(at(0.17, 0.08), Vec3::new(0.09, 0.09, 0.05), 0.785, false);
        }
    }

    match look.mouth {
        Mouth::Grin => {
            carve(at(0.0, -0.13), Vec3::new(0.36, 0.08, 0.05), 0.0, false);
            // Two dark teeth in front of the glow
            for x in [-0.08, 0.1] {
                p.spawn((
                    Mesh3d(fx.cube.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_translation(at(x, -0.11) - Vec3::Z * 0.01).with_scale(Vec3::new(0.07, 0.07, 0.06)),
                ));
            }
        }
        Mouth::Zigzag => {
            for k in 0..5 {
                let x = -0.18 + k as f32 * 0.09;
                let rot = if k % 2 == 0 { 0.6 } else { -0.6 };
                carve(at(x, -0.14), Vec3::new(0.1, 0.045, 0.05), rot, false);
            }
        }
        Mouth::Shocked => carve(at(0.0, -0.15), Vec3::new(0.09, 0.11, 0.03), 0.0, true),
        Mouth::Smile => {
            carve(at(0.0, -0.18), Vec3::new(0.16, 0.06, 0.05), 0.0, false);
            carve(at(-0.15, -0.13), Vec3::new(0.13, 0.06, 0.05), -0.45, false);
            carve(at(0.15, -0.13), Vec3::new(0.13, 0.06, 0.05), 0.45, false);
        }
    }

    // Top of the head, where hats sit.
    let top = HEAD_Y + HEAD_R * hs.y;
    let mut part = |mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, tf: Transform| {
        p.spawn((Mesh3d(mesh), MeshMaterial3d(mat), tf));
    };

    match look.hat {
        Hat::None => {
            part(
                meshes.add(Cylinder::new(0.06, 0.25)),
                fx.stem.clone(),
                Transform::from_xyz(0.0, top + 0.08, 0.0).with_rotation(Quat::from_rotation_z(look.stem_tilt)),
            );
        }
        Hat::Straw => {
            part(meshes.add(Cylinder::new(0.58, 0.03)), fx.straw.clone(), Transform::from_xyz(0.0, top - 0.04, 0.0));
            part(meshes.add(Cylinder::new(0.27, 0.18)), fx.straw.clone(), Transform::from_xyz(0.0, top + 0.06, 0.0));
        }
        Hat::Top => {
            part(meshes.add(Cylinder::new(0.38, 0.03)), fx.black.clone(), Transform::from_xyz(0.0, top - 0.03, 0.0));
            part(meshes.add(Cylinder::new(0.23, 0.45)), fx.black.clone(), Transform::from_xyz(0.0, top + 0.2, 0.0));
            part(meshes.add(Cylinder::new(0.235, 0.06)), fx.candy.clone(), Transform::from_xyz(0.0, top + 0.03, 0.0));
        }
        Hat::Bucket => {
            part(
                meshes.add(Cylinder::new(0.3, 0.34)),
                fx.metal.clone(),
                Transform::from_xyz(0.05, top + 0.08, 0.0).with_rotation(Quat::from_rotation_z(-0.25)),
            );
        }
        Hat::Witch => {
            part(meshes.add(Cylinder::new(0.52, 0.04)), shirt.clone(), Transform::from_xyz(0.0, top - 0.02, 0.0));
            part(
                meshes.add(Cone { radius: 0.29, height: 0.55 }),
                shirt.clone(),
                Transform::from_xyz(0.0, top + 0.25, 0.03).with_rotation(Quat::from_rotation_x(-0.3)),
            );
        }
        Hat::Crown => {
            let gold = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.75, 0.15),
                emissive: LinearRgba::rgb(0.6, 0.4, 0.05),
                metallic: 0.8,
                perceptual_roughness: 0.3,
                ..default()
            });
            part(meshes.add(Cylinder::new(0.36, 0.12)), gold.clone(), Transform::from_xyz(0.0, top - 0.02, 0.0));
            let point = meshes.add(Cone { radius: 0.07, height: 0.22 });
            for k in 0..5 {
                let a = k as f32 / 5.0 * std::f32::consts::TAU;
                part(point.clone(), gold.clone(), Transform::from_xyz(a.sin() * 0.3, top + 0.14, a.cos() * 0.3));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_enemies_vary_but_keep_their_type() {
        let mut rng = rand::rng();
        let looks: Vec<Look> = (0..60).map(|_| roll(Kind::Walker, Color::srgb(0.55, 0.15, 0.12), &mut rng)).collect();
        // Plenty of variety across 60 rolls...
        assert!(looks.iter().any(|l| l.hat != looks[0].hat));
        assert!(looks.iter().any(|l| l.eyes != looks[0].eyes));
        assert!(looks.iter().any(|l| l.mouth != looks[0].mouth));
        // ...but shirts stay red-dominant so Walkers still read as Walkers.
        assert!(looks.iter().all(|l| {
            let c = l.shirt.to_srgba();
            c.red > c.green && c.red > c.blue
        }));
    }

    #[test]
    fn lobbers_always_wear_witch_hats_and_bosses_are_fixed() {
        let mut rng = rand::rng();
        for _ in 0..20 {
            assert_eq!(roll(Kind::Lobber, Color::BLACK, &mut rng).hat, Hat::Witch);
            assert_eq!(roll(Kind::King, Color::BLACK, &mut rng).hat, Hat::Crown);
            assert_eq!(roll(Kind::Witch, Color::BLACK, &mut rng).hat, Hat::Witch);
        }
    }
}

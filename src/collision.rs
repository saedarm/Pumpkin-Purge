//! Static collision for the festival props (hay bales, booths, lamp posts, gravestones).
//! Every prop is an oriented box. Circles (player, enemies) get pushed out of them,
//! rays (bullets) stop at them, and the player can stand on top of low ones.

use bevy::prelude::*;

#[derive(Clone, Copy)]
pub struct Box3 {
    center: Vec3,
    rot: Quat,
    inv: Quat,
    half: Vec3,
}

impl Box3 {
    pub fn from_transform(tf: &Transform, size: Vec3) -> Self {
        Self { center: tf.translation, rot: tf.rotation, inv: tf.rotation.inverse(), half: size / 2.0 }
    }

    fn top(&self) -> f32 {
        self.center.y + self.half.y
    }

    fn bottom(&self) -> f32 {
        self.center.y - self.half.y
    }
}

#[derive(Resource, Default)]
pub struct Obstacles(pub Vec<Box3>);

/// How far below a box's top your feet can be and still land on it.
const STEP: f32 = 0.25;

impl Obstacles {
    /// Push a vertical cylinder (feet at `pos.y`) out of every box it overlaps.
    /// Returns the corrected position and the highest surface under it to stand on.
    pub fn resolve(&self, mut pos: Vec3, radius: f32, height: f32) -> (Vec3, f32) {
        let mut ground = 0.0f32;
        for b in &self.0 {
            // Ignore boxes entirely above our head (booth awnings).
            if b.bottom() > pos.y + height {
                continue;
            }
            let local = b.inv * (pos - b.center);
            let closest = Vec2::new(local.x.clamp(-b.half.x, b.half.x), local.z.clamp(-b.half.z, b.half.z));
            let d = Vec2::new(local.x, local.z) - closest;
            let len = d.length();
            if len >= radius {
                continue;
            }
            // Standing on top of it?
            if pos.y >= b.top() - STEP {
                ground = ground.max(b.top());
                continue;
            }
            let push = if len > 1e-4 {
                d / len * (radius - len)
            } else {
                // Center is inside the box: shove out along the shallowest side.
                let px = b.half.x - local.x.abs() + radius;
                let pz = b.half.z - local.z.abs() + radius;
                if px < pz { Vec2::new(px * local.x.signum(), 0.0) } else { Vec2::new(0.0, pz * local.z.signum()) }
            };
            pos += b.rot * Vec3::new(push.x, 0.0, push.y);
        }
        (pos, ground)
    }

    /// Distance to the first box a ray hits, if any within `max`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for b in &self.0 {
            let o = b.inv * (origin - b.center);
            let d = b.inv * dir;
            let mut tmin = 0.0f32;
            let mut tmax = max;
            let mut hit = true;
            for axis in 0..3 {
                let (oa, da, h) = (o[axis], d[axis], b.half[axis]);
                if da.abs() < 1e-6 {
                    if oa.abs() > h {
                        hit = false;
                        break;
                    }
                } else {
                    let t1 = (-h - oa) / da;
                    let t2 = (h - oa) / da;
                    tmin = tmin.max(t1.min(t2));
                    tmax = tmax.min(t1.max(t2));
                    if tmin > tmax {
                        hit = false;
                        break;
                    }
                }
            }
            if hit && best.is_none_or(|bt| tmin < bt) {
                best = Some(tmin);
            }
        }
        best
    }

    /// Does any prop cover this spot on the ground (within `pad`)? Ignores height,
    /// except for things entirely above head height. Used to build the pathfinding grid.
    pub fn blocks_ground(&self, p: Vec2, pad: f32) -> bool {
        self.0.iter().any(|b| {
            if b.bottom() > 2.0 {
                return false;
            }
            let l = b.inv * (Vec3::new(p.x, b.center.y, p.y) - b.center);
            l.x.abs() < b.half.x + pad && l.z.abs() < b.half.z + pad
        })
    }

    /// Is the point inside any box (with some padding)?
    pub fn contains(&self, p: Vec3, pad: f32) -> bool {
        self.0.iter().any(|b| {
            let l = b.inv * (p - b.center);
            l.x.abs() < b.half.x + pad && l.y.abs() < b.half.y + pad && l.z.abs() < b.half.z + pad
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bale() -> Obstacles {
        Obstacles(vec![Box3::from_transform(&Transform::from_xyz(0.0, 0.45, 0.0), Vec3::new(1.6, 0.9, 0.9))])
    }

    #[test]
    fn walking_into_a_bale_pushes_you_out() {
        let (p, ground) = bale().resolve(Vec3::new(0.0, 0.0, 0.6), 0.4, 1.8);
        assert!(p.z >= 0.45 + 0.4 - 1e-3, "pushed to the bale's edge, got {p:?}");
        assert_eq!(ground, 0.0);
    }

    #[test]
    fn you_can_stand_on_top_of_a_bale() {
        let (p, ground) = bale().resolve(Vec3::new(0.0, 0.85, 0.2), 0.4, 1.8);
        assert_eq!(p.z, 0.2, "not pushed sideways");
        assert!((ground - 0.9).abs() < 1e-4);
    }

    #[test]
    fn bale_blocks_a_bullet() {
        let obs = bale();
        let hit = obs.raycast(Vec3::new(0.0, 0.5, 5.0), Vec3::NEG_Z, 50.0);
        assert!((hit.unwrap() - 4.55).abs() < 1e-3);
        // Shooting over it misses.
        assert!(obs.raycast(Vec3::new(0.0, 1.5, 5.0), Vec3::NEG_Z, 50.0).is_none());
    }

    #[test]
    fn rotated_box_collides_on_its_real_shape() {
        let tf = Transform::from_xyz(0.0, 0.45, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        let obs = Obstacles(vec![Box3::from_transform(&tf, Vec3::new(1.6, 0.9, 0.9))]);
        // Rotated 90 degrees, the long side now runs along Z.
        let (p, _) = obs.resolve(Vec3::new(0.0, 0.0, 1.0), 0.4, 1.8);
        assert!(p.z >= 0.8 + 0.4 - 1e-3);
    }
}

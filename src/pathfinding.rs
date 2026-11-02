//! Pathfinding for the scarecrows: a flow field.
//!
//! The arena is a grid of half-meter cells. Cells under props are blocked, padded out
//! by an enemy's radius so they don't clip corners. A few times a second we run
//! Dijkstra outward from the player's cell, which gives every open cell its walking
//! distance to the player around the props. Each enemy then just walks downhill.
//!
//! One calculation serves every enemy, so it stays cheap with big waves. There are two
//! fields: one padded for normal-size scarecrows and one padded wider for Brutes and
//! bosses, who need more room to get around things.
//!
//! Press F3 in game to see the blocked cells and each enemy's current waypoint.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use bevy::prelude::*;

use crate::collision::Obstacles;
use crate::enemies::Enemy;
use crate::festival::ARENA_HALF;
use crate::player::Player;

pub const CELL: f32 = 0.5;
const SIZE: usize = (ARENA_HALF * 2.0 / CELL) as usize;
const UNREACHED: u32 = u32::MAX;
/// Step costs: 10 straight, 14 diagonal (about 10 * sqrt 2).
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
/// How far ahead along the path an enemy aims, in cells. Larger = smoother corners.
const LOOKAHEAD: usize = 8;
/// Enemies at or below this scale use the small field; bigger ones use the large one.
const LARGE_SCALE: f32 = 1.2;

pub struct Field {
    blocked: Vec<bool>,
    dist: Vec<u32>,
}

impl Field {
    pub fn new(obstacles: &Obstacles, pad: f32) -> Self {
        let mut blocked = vec![false; SIZE * SIZE];
        for z in 0..SIZE {
            for x in 0..SIZE {
                blocked[z * SIZE + x] = obstacles.blocks_ground(center(x, z), pad);
            }
        }
        Self { blocked, dist: vec![UNREACHED; SIZE * SIZE] }
    }

    fn is_blocked(&self, x: usize, z: usize) -> bool {
        self.blocked[z * SIZE + x]
    }

    fn dist_at(&self, x: usize, z: usize) -> u32 {
        self.dist[z * SIZE + x]
    }

    /// Recompute walking distances to `target` for every open cell.
    pub fn rebuild(&mut self, target: Vec2) {
        self.dist.fill(UNREACHED);
        let mut heap = BinaryHeap::new();
        let (tx, tz) = cell_of(target);
        if !self.is_blocked(tx, tz) {
            self.dist[tz * SIZE + tx] = 0;
            heap.push(Reverse((0, tx, tz)));
        } else {
            // The player is standing on a prop (a bale stack). Head for the open cells
            // around it instead, so enemies gather at its edge.
            let r = (3.0 / CELL) as isize;
            for dz in -r..=r {
                for dx in -r..=r {
                    let (x, z) = (tx as isize + dx, tz as isize + dz);
                    if !in_bounds(x, z) || self.is_blocked(x as usize, z as usize) {
                        continue;
                    }
                    let (x, z) = (x as usize, z as usize);
                    let cost = (center(x, z).distance(target) / CELL * STRAIGHT as f32) as u32;
                    if cost < self.dist[z * SIZE + x] {
                        self.dist[z * SIZE + x] = cost;
                        heap.push(Reverse((cost, x, z)));
                    }
                }
            }
        }

        let (blocked, dist) = (&self.blocked, &mut self.dist);
        while let Some(Reverse((d, x, z))) = heap.pop() {
            if d > dist[z * SIZE + x] {
                continue;
            }
            for (nx, nz, step) in neighbors(blocked, x, z) {
                let nd = d + step;
                if nd < dist[nz * SIZE + nx] {
                    dist[nz * SIZE + nx] = nd;
                    heap.push(Reverse((nd, nx, nz)));
                }
            }
        }
    }

    fn neighbors(&self, x: usize, z: usize) -> impl Iterator<Item = (usize, usize, u32)> + '_ {
        neighbors(&self.blocked, x, z)
    }

    /// The lowest-distance neighbor, if it's closer to the player than this cell.
    fn downhill(&self, x: usize, z: usize) -> Option<(usize, usize)> {
        let here = self.dist_at(x, z);
        self.neighbors(x, z)
            .map(|(nx, nz, _)| (self.dist_at(nx, nz), nx, nz))
            .filter(|&(d, _, _)| d < here)
            .min()
            .map(|(_, nx, nz)| (nx, nz))
    }

    /// Is the straight line from `a` to `b` free of blocked cells?
    ///
    /// Walks every cell the line passes through (a grid "DDA" walk), so it can't skip
    /// over a corner. The start cell is ignored.
    pub fn clear_line(&self, a: Vec2, b: Vec2) -> bool {
        self.clear_line_with_slack(a, b, 0.0)
    }

    /// Same, but blocked cells within `slack` meters of `b` don't count. Used when `b`
    /// is the player, so someone standing flush against a prop still counts as visible.
    fn clear_line_with_slack(&self, a: Vec2, b: Vec2, slack: f32) -> bool {
        let (mut x, mut z) = cell_of(a);
        let (bx, bz) = cell_of(b);
        let d = b - a;
        let step_x: isize = if d.x > 0.0 { 1 } else { -1 };
        let step_z: isize = if d.y > 0.0 { 1 } else { -1 };
        // Distance along the line (as a 0..1 fraction) to the next vertical / horizontal cell edge.
        let edge = |v: f32, cell: usize, step: isize| {
            let base = cell as f32 * CELL - ARENA_HALF;
            if step > 0 { base + CELL - v } else { v - base }
        };
        let mut t_x = if d.x.abs() > 1e-6 { edge(a.x, x, step_x) / d.x.abs() } else { f32::INFINITY };
        let mut t_z = if d.y.abs() > 1e-6 { edge(a.y, z, step_z) / d.y.abs() } else { f32::INFINITY };
        let dt_x = if d.x.abs() > 1e-6 { CELL / d.x.abs() } else { f32::INFINITY };
        let dt_z = if d.y.abs() > 1e-6 { CELL / d.y.abs() } else { f32::INFINITY };

        let near_goal = |x: usize, z: usize| center(x, z).distance(b) < slack;
        for _ in 0..(SIZE * 2) {
            if (x, z) == (bx, bz) {
                return true;
            }
            if t_x < t_z {
                x = (x as isize + step_x).clamp(0, SIZE as isize - 1) as usize;
                t_x += dt_x;
            } else {
                z = (z as isize + step_z).clamp(0, SIZE as isize - 1) as usize;
                t_z += dt_z;
            }
            if self.is_blocked(x, z) && !near_goal(x, z) {
                return false;
            }
        }
        true
    }

    /// Where an enemy at `from` should head next to reach `goal`.
    pub fn waypoint(&self, from: Vec2, goal: Vec2) -> Vec2 {
        if self.clear_line_with_slack(from, goal, CELL * 1.5) {
            return goal;
        }
        let (mut x, mut z) = cell_of(from);

        // Pushed into a padded zone (knockback, crowding)? Step out toward the
        // nearest open, reachable cell first.
        if self.is_blocked(x, z) || self.dist_at(x, z) == UNREACHED {
            let best = (-2isize..=2)
                .flat_map(|dz| (-2isize..=2).map(move |dx| (dx, dz)))
                .filter_map(|(dx, dz)| {
                    let (nx, nz) = (x as isize + dx, z as isize + dz);
                    (in_bounds(nx, nz) && !self.is_blocked(nx as usize, nz as usize)).then(|| (nx as usize, nz as usize))
                })
                .filter(|&(nx, nz)| self.dist_at(nx, nz) != UNREACHED)
                .min_by_key(|&(nx, nz)| self.dist_at(nx, nz));
            return match best {
                Some((nx, nz)) => center(nx, nz),
                None => goal,
            };
        }

        // Walk downhill, aiming at the farthest cell we can still see in a straight
        // line. That rounds corners smoothly instead of zig-zagging cell to cell.
        let here = center(x, z);
        let mut target = None;
        for _ in 0..LOOKAHEAD {
            let Some((nx, nz)) = self.downhill(x, z) else { break };
            let c = center(nx, nz);
            if !self.clear_line(from, c) {
                break;
            }
            target = Some(c);
            (x, z) = (nx, nz);
        }
        // Can't even see the next cell from here (we're off-center near a corner):
        // step to the middle of our own cell first.
        target.unwrap_or(if from.distance(here) > 0.05 { here } else { goal })
    }

    /// Walking distance in meters from `p` to the player, if reachable.
    #[cfg(test)]
    fn meters(&self, p: Vec2) -> Option<f32> {
        let (x, z) = cell_of(p);
        let d = self.dist_at(x, z);
        (d != UNREACHED).then(|| d as f32 / STRAIGHT as f32 * CELL)
    }
}

/// Open neighbors of a cell. Diagonals are only allowed when both side cells are
/// open too, so paths never cut across the corner of a prop.
fn neighbors(blocked: &[bool], x: usize, z: usize) -> impl Iterator<Item = (usize, usize, u32)> + '_ {
    const DIRS: [(isize, isize); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
    let is_blocked = move |x: usize, z: usize| blocked[z * SIZE + x];
    DIRS.iter().filter_map(move |&(dx, dz)| {
        let (nx, nz) = (x as isize + dx, z as isize + dz);
        if !in_bounds(nx, nz) || is_blocked(nx as usize, nz as usize) {
            return None;
        }
        if dx != 0 && dz != 0 {
            if is_blocked(nx as usize, z) || is_blocked(x, nz as usize) {
                return None;
            }
            return Some((nx as usize, nz as usize, DIAGONAL));
        }
        Some((nx as usize, nz as usize, STRAIGHT))
    })
}

fn in_bounds(x: isize, z: isize) -> bool {
    x >= 0 && z >= 0 && (x as usize) < SIZE && (z as usize) < SIZE
}

pub fn cell_of(p: Vec2) -> (usize, usize) {
    let f = |v: f32| (((v + ARENA_HALF) / CELL) as isize).clamp(0, SIZE as isize - 1) as usize;
    (f(p.x), f(p.y))
}

pub fn center(x: usize, z: usize) -> Vec2 {
    Vec2::new((x as f32 + 0.5) * CELL - ARENA_HALF, (z as f32 + 0.5) * CELL - ARENA_HALF)
}

// ---------------------------------------------------------------- Bevy side

#[derive(Resource)]
pub struct NavGrid {
    pub small: Field,
    pub large: Field,
    last_cell: Option<(usize, usize)>,
    since_rebuild: f32,
}

impl NavGrid {
    pub fn field_for(&self, scale: f32) -> &Field {
        if scale > LARGE_SCALE { &self.large } else { &self.small }
    }
}

pub fn setup_nav(mut commands: Commands, obstacles: Res<Obstacles>) {
    commands.insert_resource(NavGrid {
        small: Field::new(&obstacles, 0.45),
        large: Field::new(&obstacles, 1.0),
        last_cell: None,
        since_rebuild: f32::MAX,
    });
}

/// Rebuild the fields when the player moves to a new cell (at most ~6 times a second),
/// and at least once a second regardless.
pub fn update_nav(mut nav: ResMut<NavGrid>, player: Single<&Transform, With<Player>>, time: Res<Time>) {
    nav.since_rebuild += time.delta_secs();
    let target = player.translation.xz();
    let cell = cell_of(target);
    let moved = nav.last_cell != Some(cell);
    if (moved && nav.since_rebuild > 0.15) || nav.since_rebuild > 1.0 {
        nav.small.rebuild(target);
        nav.large.rebuild(target);
        nav.last_cell = Some(cell);
        nav.since_rebuild = 0.0;
    }
}

/// F3 toggles the pathfinding overlay.
#[derive(Resource, Default)]
pub struct NavDebug(bool);

pub fn draw_nav_debug(
    mut debug: ResMut<NavDebug>,
    keys: Res<ButtonInput<KeyCode>>,
    nav: Res<NavGrid>,
    enemies: Query<(&Enemy, &Transform)>,
    mut gizmos: Gizmos,
) {
    if keys.just_pressed(KeyCode::F3) {
        debug.0 = !debug.0;
    }
    if !debug.0 {
        return;
    }
    // Blocked cells for normal-size enemies, as small red squares on the ground.
    for z in 0..SIZE {
        for x in 0..SIZE {
            if nav.small.is_blocked(x, z) {
                let c = center(x, z);
                gizmos.rect(
                    Isometry3d::new(Vec3::new(c.x, 0.03, c.y), Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    Vec2::splat(CELL * 0.8),
                    Color::srgba(1.0, 0.2, 0.1, 0.6),
                );
            }
        }
    }
    // Each enemy's current heading.
    for (enemy, tf) in &enemies {
        let from = tf.translation.with_y(0.3);
        let to = Vec3::new(enemy.waypoint.x, 0.3, enemy.waypoint.y);
        gizmos.line(from, to, Color::srgb(0.3, 1.0, 0.4));
        gizmos.sphere(Isometry3d::from_translation(to), 0.15, Color::srgb(0.3, 1.0, 0.4));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Box3;

    /// A 10m-wide wall across the middle of the arena, along X at z = 0.
    fn wall() -> Obstacles {
        Obstacles(vec![Box3::from_transform(&Transform::from_xyz(0.0, 1.0, 0.0), Vec3::new(10.0, 2.0, 1.0))])
    }

    #[test]
    fn open_ground_goes_straight() {
        let mut f = Field::new(&Obstacles::default(), 0.45);
        f.rebuild(Vec2::new(0.0, -10.0));
        assert_eq!(f.waypoint(Vec2::new(0.0, 10.0), Vec2::new(0.0, -10.0)), Vec2::new(0.0, -10.0));
    }

    #[test]
    fn goes_around_a_wall_instead_of_into_it() {
        let mut f = Field::new(&wall(), 0.45);
        let player = Vec2::new(0.0, -5.0);
        let enemy = Vec2::new(0.0, 5.0);
        f.rebuild(player);
        assert!(!f.clear_line(enemy, player), "the wall is between them");

        // Straight through would be 10m; around the wall's end is longer.
        let walk = f.meters(enemy).unwrap();
        assert!(walk > 11.0, "walking distance should route around, got {walk}");

        // The first waypoint should head toward one end of the wall, not straight at it.
        let w = f.waypoint(enemy, player);
        assert!(w.x.abs() > 2.0, "should swing wide toward a wall end, got {w:?}");

        // Following waypoints actually gets there without touching blocked cells.
        let mut pos = enemy;
        for _ in 0..400 {
            let w = f.waypoint(pos, player);
            pos += (w - pos).normalize_or_zero() * 0.2;
            let (x, z) = cell_of(pos);
            assert!(!f.is_blocked(x, z), "walked into the wall at {pos:?}");
            if pos.distance(player) < 0.5 {
                return;
            }
        }
        panic!("never reached the player, stuck at {pos:?}");
    }

    #[test]
    fn player_on_a_prop_still_gets_a_field() {
        // Player standing on top of a 3m bale stack: every enemy should still have a path to its edge.
        let stack = Obstacles(vec![Box3::from_transform(&Transform::from_xyz(5.0, 0.9, 5.0), Vec3::new(3.0, 1.8, 3.0))]);
        let mut f = Field::new(&stack, 0.45);
        f.rebuild(Vec2::new(5.0, 5.0));
        assert!(f.meters(Vec2::new(-20.0, -20.0)).is_some());
        // Distances count from the player up on the stack, so its edge is ~2m away already.
        let beside = f.meters(Vec2::new(5.0, 7.5)).unwrap();
        let far = f.meters(Vec2::new(5.0, 15.0)).unwrap();
        assert!(beside < 3.5, "edge of the stack should be close, got {beside}");
        assert!(far > beside + 6.0);
    }

    #[test]
    fn line_check_catches_a_corner_graze() {
        // A line that only clips the corner of a blocked cell must count as blocked.
        let f = Field::new(&Obstacles(vec![Box3::from_transform(&Transform::from_xyz(0.25, 0.5, 0.25), Vec3::new(0.4, 1.0, 0.4))]), 0.0);
        assert!(!f.clear_line(Vec2::new(-2.0, 0.6), Vec2::new(2.0, -0.1)));
        assert!(f.clear_line(Vec2::new(-2.0, 2.0), Vec2::new(2.0, 2.0)));
    }

    #[test]
    fn no_corner_cutting() {
        // A single blocked cell: diagonal moves past it must not slip through its corner.
        let f = Field::new(&Obstacles(vec![Box3::from_transform(&Transform::from_xyz(0.25, 0.5, 0.25), Vec3::new(0.4, 1.0, 0.4))]), 0.0);
        let (bx, bz) = cell_of(Vec2::new(0.25, 0.25));
        assert!(f.is_blocked(bx, bz));
        let from = (bx - 1, bz);
        assert!(f.neighbors(from.0, from.1).all(|(x, z, _)| (x, z) != (bx, bz + 1) && (x, z) != (bx, bz - 1)));
    }
}


#[cfg(test)]
mod festival_map {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Build the real festival layout without opening a window.
    fn real_obstacles() -> Obstacles {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.run_system_once(crate::festival::setup_festival).unwrap();
        world.remove_resource::<Obstacles>().unwrap()
    }

    /// Spawn points every 4m along all four corn walls, like the wave spawner uses.
    fn spawn_points() -> Vec<Vec2> {
        let edge = ARENA_HALF - 1.0;
        let mut pts = Vec::new();
        let mut a = -ARENA_HALF + 2.0;
        while a < ARENA_HALF - 2.0 {
            pts.extend([Vec2::new(a, edge), Vec2::new(a, -edge), Vec2::new(edge, a), Vec2::new(-edge, a)]);
            a += 4.0;
        }
        pts
    }

    #[test]
    fn every_spawn_point_reaches_the_player_on_the_real_map() {
        let obstacles = real_obstacles();
        assert!(obstacles.0.len() > 20, "festival props should be loaded");
        // Player spots: the start, behind the booths, inside the graveyard, beside a bale stack.
        let players = [Vec2::new(0.0, 10.0), Vec2::new(-2.0, -26.5), Vec2::new(21.0, -23.5), Vec2::new(-6.0, 0.5)];
        for (pad, radius) in [(0.45, 0.45), (1.0, 1.0)] {
            let mut f = Field::new(&obstacles, pad);
            for &player in &players {
                f.rebuild(player);
                for &start in &spawn_points() {
                    let mut pos = start;
                    let mut reached = false;
                    for _ in 0..1500 {
                        let w = f.waypoint(pos, player);
                        let next = pos + (w - pos).normalize_or_zero() * 0.1;
                        // Same push-out the game applies after steering.
                        let (resolved, _) = obstacles.resolve(Vec3::new(next.x, 0.0, next.y), radius * 0.9, 2.0);
                        pos = resolved.xz();
                        if pos.distance(player) < 2.5 {
                            reached = true;
                            break;
                        }
                    }
                    assert!(reached, "pad {pad}: from {start:?} never reached player at {player:?}, stuck at {pos:?}");
                }
            }
        }
    }

    /// `cargo test export_path_map -- --ignored` draws a top-down picture of the real map
    /// (props, padded no-go zones, and walked paths) to target/pathmap-*.ppm.
    #[test]
    #[ignore]
    fn export_path_map() {
        let obstacles = real_obstacles();
        let raw = Field::new(&obstacles, 0.0);
        let mut f = Field::new(&obstacles, 0.45);
        let scenes = [
            ("booths", Vec2::new(-2.0, -26.0)),
            ("stack", Vec2::new(-6.0, 0.6)),
            ("graveyard", Vec2::new(21.0, -23.6)),
        ];
        let px = 6;
        let w = SIZE * px;
        for (name, player) in scenes {
            f.rebuild(player);
            let mut img = vec![[28u8, 40, 22]; w * w];
            for z in 0..SIZE {
                for x in 0..SIZE {
                    let col = if raw.is_blocked(x, z) {
                        [200, 160, 70]
                    } else if f.is_blocked(x, z) {
                        [120, 45, 35]
                    } else {
                        continue;
                    };
                    for dy in 0..px {
                        for dx in 0..px {
                            img[(z * px + dy) * w + x * px + dx] = col;
                        }
                    }
                }
            }
            let mut plot = |p: Vec2, col: [u8; 3], r: isize| {
                let cx = ((p.x + ARENA_HALF) / CELL * px as f32) as isize;
                let cz = ((p.y + ARENA_HALF) / CELL * px as f32) as isize;
                for dz in -r..=r {
                    for dx in -r..=r {
                        let (x, z) = (cx + dx, cz + dz);
                        if x >= 0 && z >= 0 && (x as usize) < w && (z as usize) < w {
                            img[z as usize * w + x as usize] = col;
                        }
                    }
                }
            };
            for start in spawn_points().into_iter().step_by(3) {
                let mut pos = start;
                for _ in 0..1500 {
                    plot(pos, [110, 255, 120], 1);
                    let wp = f.waypoint(pos, player);
                    let next = pos + (wp - pos).normalize_or_zero() * 0.1;
                    let (r, _) = obstacles.resolve(Vec3::new(next.x, 0.0, next.y), 0.4, 2.0);
                    pos = r.xz();
                    if pos.distance(player) < 1.5 {
                        break;
                    }
                }
                plot(start, [255, 255, 255], 3);
            }
            plot(player, [255, 220, 0], 6);
            let mut out = format!("P6 {w} {w} 255\n").into_bytes();
            out.extend(img.iter().flatten());
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/pathmap-{name}.ppm"));
            std::fs::write(path, out).unwrap();
        }
    }

    #[test]
    fn rebuild_is_fast_enough() {
        let obstacles = real_obstacles();
        let mut f = Field::new(&obstacles, 0.45);
        let t = std::time::Instant::now();
        for i in 0..20 {
            f.rebuild(Vec2::new(i as f32 - 10.0, 5.0));
        }
        let each = t.elapsed().as_secs_f32() / 20.0 * 1000.0;
        println!("rebuild: {each:.2} ms (test build)");
        assert!(each < 50.0, "rebuild took {each} ms");
    }
}

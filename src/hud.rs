//! On-screen UI: crosshair with hit markers, wave/score, health bar, weapon list,
//! damage flash, damage direction markers, and the big center text (wave banners, game over).
//! The title and pause screens are the menu in `settings.rs`.

use bevy::prelude::*;

use crate::enemies::{self, Enemy, Waves};
use crate::player::{HurtFlash, Player, Shake};
use crate::save::{Best, Practice};
use crate::loadout::Look;
use crate::settings::Settings;
use crate::weapons::{Arsenal, WEAPONS};
use crate::{GameState, Score};

#[derive(Component)]
pub struct Crosshair;
#[derive(Component)]
pub struct TopText;
#[derive(Component)]
pub struct HealthText;
#[derive(Component)]
pub struct HealthFill;
#[derive(Component)]
pub struct WeaponSlot(usize);
#[derive(Component)]
pub struct CenterText;
#[derive(Component)]
pub struct HurtOverlay;
#[derive(Component)]
pub struct AmmoHint;
#[derive(Component)]
pub struct BossBar;
#[derive(Component)]
pub struct BossFill;
#[derive(Component)]
pub struct BossName;

/// Floating health bar over a hurt (non-boss) enemy.
#[derive(Component)]
pub struct EnemyBar {
    enemy: Entity,
    fill: Entity,
    width: f32,
}

/// A damage number floating up from where an enemy got hit.
#[derive(Component)]
pub struct DamageNumber {
    at: Vec3,
    life: f32,
    crit: bool,
}

#[derive(Component)]
pub struct GrenadeText;

const BAR_SHOW_SECS: f32 = 5.0;
const NUMBER_LIFE: f32 = 0.9;

/// Black ring with crosshairs, shown when fully aimed through a scope.
#[derive(Component)]
pub struct ScopeOverlay;

/// A red bar around the crosshair pointing toward whatever just hit you.
#[derive(Component)]
pub struct DamageIndicator {
    from: Vec3,
    life: f32,
}

const INDICATOR_LIFE: f32 = 1.4;
/// Distance from screen center to the marker, in pixels.
const INDICATOR_RADIUS: f32 = 120.0;

const ORANGE: Color = Color::srgb(1.0, 0.6, 0.15);
const DIM: Color = Color::srgba(1.0, 0.85, 0.6, 0.45);

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

pub fn setup_hud(mut commands: Commands) {
    // Red damage flash, full screen, under everything else.
    commands.spawn((
        HurtOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::NONE),
        Pickable::IGNORE,
    ));

    // Scope view: a huge black border around a round hole, plus thin crosshairs.
    // The border is wide enough to cover the whole screen outside the circle.
    const LENS: f32 = 620.0;
    const RIM: f32 = 1400.0;
    let total = LENS + 2.0 * RIM;
    commands
        .spawn((
            ScopeOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(total),
                    height: px(total),
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect { left: px(-total / 2.0), top: px(-total / 2.0), ..default() },
                    border: UiRect::all(px(RIM)),
                    border_radius: BorderRadius::all(percent(50)),
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                Pickable::IGNORE,
            ));
            p.spawn((Node { position_type: PositionType::Absolute, width: px(LENS), height: px(2), ..default() }, BackgroundColor(Color::BLACK)));
            p.spawn((Node { position_type: PositionType::Absolute, width: px(2), height: px(LENS), ..default() }, BackgroundColor(Color::BLACK)));
            p.spawn((
                Node { position_type: PositionType::Absolute, width: px(5), height: px(5), ..default() },
                BackgroundColor(Color::srgb(1.0, 0.15, 0.1)),
            ));
        });

    // Crosshair
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((Crosshair, Node { width: px(6), height: px(6), ..default() }, BackgroundColor(Color::WHITE)));
        });

    // Reload / low-ammo hint just under the crosshair
    commands.spawn((
        AmmoHint,
        Text::new(""),
        font(20.0),
        TextColor(ORANGE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, top: percent(56), width: percent(100), ..default() },
    ));

    // Top left: wave and score
    commands.spawn((
        TopText,
        Text::new(""),
        font(24.0),
        TextColor(ORANGE),
        Node { position_type: PositionType::Absolute, top: px(16), left: px(18), ..default() },
    ));

    // Bottom left: health
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            left: px(18),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|p| {
            p.spawn((HealthText, Text::new(""), font(22.0), TextColor(ORANGE)));
            p.spawn((
                Node { width: px(260), height: px(16), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    HealthFill,
                    Node { width: percent(100), height: percent(100), ..default() },
                    BackgroundColor(Color::srgb(0.85, 0.35, 0.08)),
                ));
            });
        });

    // Bottom right: weapon list
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            right: px(18),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(4),
            ..default()
        })
        .with_children(|p| {
            for i in 0..WEAPONS.len() {
                p.spawn((WeaponSlot(i), Text::new(""), font(20.0), TextColor(DIM)));
            }
            p.spawn((GrenadeText, Text::new(""), font(20.0), TextColor(Color::srgb(1.0, 0.45, 0.55))));
        });

    // Boss health bar, top center, hidden until the King shows up.
    commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: px(56),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((BossName, Text::new(""), font(24.0), TextColor(Color::srgb(0.6, 1.0, 0.5))));
            p.spawn((Node { width: px(520), height: px(14), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))))
                .with_children(|bar| {
                    bar.spawn((
                        BossFill,
                        Node { width: percent(100), height: percent(100), ..default() },
                        BackgroundColor(Color::srgb(0.35, 0.85, 0.25)),
                    ));
                });
        });

    // Center text
    commands.spawn((
        CenterText,
        Text::new(""),
        font(34.0),
        TextColor(ORANGE),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, top: percent(24), width: percent(100), ..default() },
    ));
}

pub fn update_top(mut text: Single<&mut Text, With<TopText>>, waves: Res<Waves>, score: Res<Score>) {
    text.0 = format!(
        "Wave {}    Score {}    Kills {}    Candy {}",
        waves.wave.max(1),
        score.points,
        score.kills,
        score.candy
    );
}

/// One bar for whatever boss is alive. After the Golem splits, it tracks both halves together.
pub fn update_boss_bar(
    mut bar: Single<&mut Node, With<BossBar>>,
    mut fill: Single<&mut Node, (With<BossFill>, Without<BossBar>)>,
    mut name: Single<&mut Text, With<BossName>>,
    enemies: Query<&Enemy>,
) {
    let bosses: Vec<&Enemy> = enemies.iter().filter(|e| enemies::is_boss(e.kind)).collect();
    let Some(first) = bosses.first() else {
        bar.display = Display::None;
        return;
    };
    bar.display = Display::Flex;
    let (hp, max) = bosses.iter().fold((0.0, 0.0), |(h, m), e| (h + e.health.max(0.0), m + e.max_health));
    fill.width = percent(100.0 * hp / max);
    let status = bosses.iter().map(|e| e.status).find(|s| !s.is_empty()).unwrap_or("");
    name.0 = if status.is_empty() {
        enemies::boss_name(first.kind).to_string()
    } else {
        format!("{}  -  {}", enemies::boss_name(first.kind), status)
    };
}

pub fn update_health(
    mut text: Single<&mut Text, With<HealthText>>,
    mut fill: Single<&mut Node, With<HealthFill>>,
    player: Single<&Player>,
) {
    let hp = player.health.max(0);
    text.0 = format!("Health {hp} / {}", player.max_health);
    fill.width = percent(100.0 * hp as f32 / player.max_health as f32);
}

pub fn update_weapons(
    mut slots: Query<(&WeaponSlot, &mut Text, &mut TextColor)>,
    mut grenades: Single<&mut Text, (With<GrenadeText>, Without<WeaponSlot>)>,
    arsenal: Res<Arsenal>,
) {
    grenades.0 = format!("G  Candy Grenades  x{}", arsenal.grenades);
    for (slot, mut text, mut color) in &mut slots {
        let w = &WEAPONS[slot.0];
        if !arsenal.loadout.owned_weapon[slot.0] {
            text.0 = format!("{}  {}   [locked - shop]", slot.0 + 1, w.name);
            color.0 = Color::srgba(0.6, 0.6, 0.6, 0.3);
            continue;
        }
        let reserve = if Arsenal::infinite(slot.0) { "--".to_string() } else { arsenal.reserve[slot.0].to_string() };
        let ammo = format!("{} / {}", arsenal.mag[slot.0], reserve);
        let marker = if slot.0 == arsenal.current { "> " } else { "" };
        text.0 = format!("{marker}{}  {}   [{}]", slot.0 + 1, w.name, ammo);
        color.0 = if slot.0 == arsenal.current {
            ORANGE
        } else if arsenal.has_ammo(slot.0) {
            DIM
        } else {
            Color::srgba(0.6, 0.3, 0.3, 0.4)
        };
    }
}

pub fn update_crosshair(
    mut cross: Single<(&mut BackgroundColor, &mut Node), With<Crosshair>>,
    mut scope: Single<&mut Node, (With<ScopeOverlay>, Without<Crosshair>)>,
    arsenal: Res<Arsenal>,
    state: Res<State<GameState>>,
) {
    let (ref mut bg, ref mut node) = *cross;
    let sight = arsenal.loadout.equipped_look(arsenal.current, 0);
    let scoped_in = sight == Some(Look::Scope) && arsenal.aim > 0.85 && *state.get() == GameState::Playing;
    scope.display = if scoped_in { Display::Flex } else { Display::None };
    let (t, head) = arsenal.hit_marker;
    if scoped_in {
        // The scope has its own crosshairs.
        bg.0 = Color::NONE;
    } else if t > 0.0 {
        bg.0 = if head { Color::srgb(1.0, 0.15, 0.1) } else { ORANGE };
        node.width = px(12);
        node.height = px(12);
    } else if sight == Some(Look::RedDot) && arsenal.aim > 0.6 {
        // Looking through the red dot: the dot is the crosshair.
        bg.0 = Color::srgb(1.0, 0.1, 0.05);
        node.width = px(5);
        node.height = px(5);
    } else {
        bg.0 = Color::WHITE;
        node.width = px(6);
        node.height = px(6);
    }
}

pub fn update_hurt(
    mut overlay: Single<&mut BackgroundColor, With<HurtOverlay>>,
    mut hurt: ResMut<HurtFlash>,
    god: Res<crate::save::GodMode>,
    time: Res<Time>,
) {
    hurt.0 = (hurt.0 - time.delta_secs()).max(0.0);
    if god.0 {
        hurt.0 = 0.0;
    }
    overlay.0 = Color::srgba(0.8, 0.0, 0.0, hurt.0);
}

pub fn update_center(
    mut text: Single<&mut Text, With<CenterText>>,
    state: Res<State<GameState>>,
    waves: Res<Waves>,
    score: Res<Score>,
    best: Res<Best>,
    practice: Res<Practice>,
    settings: Res<crate::settings::Settings>,
    time: Res<Time<Virtual>>,
) {
    text.0 = match state.get() {
        GameState::Title => String::new(),
        GameState::GameOver => {
            let record = if practice.0 {
                "Practice run - not counted toward your best".to_string()
            } else if best.new_record {
                "NEW BEST SCORE!".to_string()
            } else {
                let b = best.get(settings.difficulty);
                format!(
                    "Best on {}:  wave {}  |  score {}  |  kills {}",
                    crate::settings::difficulty_name(settings.difficulty),
                    b.wave,
                    b.score,
                    b.kills
                )
            };
            format!(
                "THE PUMPKINS GOT YOU\n\nReached wave {}  |  Score {}  |  Kills {}\n{record}\n\nPress R to try again",
                waves.wave, score.points, score.kills
            )
        }
        GameState::Playing if time.is_paused() => String::new(),
        GameState::Playing if waves.intermission > 0.0 => {
            if waves.wave == 0 {
                "Get ready...".to_string()
            } else {
                format!("WAVE {} CLEARED", waves.wave)
            }
        }
        GameState::Playing => String::new(),
    };
}

pub fn update_ammo_hint(mut text: Single<&mut Text, With<AmmoHint>>, arsenal: Res<Arsenal>, state: Res<State<GameState>>) {
    let i = arsenal.current;
    text.0 = if *state.get() != GameState::Playing {
        String::new()
    } else if arsenal.reloading.is_some() {
        "RELOADING".to_string()
    } else if arsenal.mag[i] == 0 && !arsenal.has_ammo(i) {
        "OUT OF AMMO".to_string()
    } else if arsenal.mag[i] == 0 {
        "Press R to reload".to_string()
    } else if arsenal.mag[i] * 4 <= arsenal.mag_size(i) && arsenal.mag_size(i) > 1 {
        "Low ammo  (R)".to_string()
    } else {
        String::new()
    };
}

/// Turn new hits into screen shake and a direction marker each.
pub fn process_hits(mut commands: Commands, mut player: Single<&mut Player>, mut shake: ResMut<Shake>) {
    let hits: Vec<(Vec3, i32)> = player.hits.drain(..).collect();
    for (from, dmg) in hits {
        // No shake for hits that did nothing (god mode).
        if dmg > 0 {
            shake.add(0.25 + dmg as f32 / 60.0);
        }
        commands.spawn((
            DamageIndicator { from, life: INDICATOR_LIFE },
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(130),
                height: px(16),
                margin: UiRect { left: px(-65), top: px(-8), ..default() },
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiTransform::default(),
            Pickable::IGNORE,
        ));
    }
}

/// Where a hit came from, as a screen angle: 0 = straight ahead (top of the screen),
/// positive = to your right (clockwise), up to ±PI = directly behind you.
pub fn hit_angle(yaw: f32, player: Vec3, from: Vec3) -> f32 {
    let to = (from - player).xz();
    let forward = Vec2::new(-yaw.sin(), -yaw.cos());
    let right = Vec2::new(yaw.cos(), -yaw.sin());
    to.dot(right).atan2(to.dot(forward))
}

/// Keep each marker pointing at its source as you turn, and fade it out.
pub fn update_indicators(
    mut commands: Commands,
    mut indicators: Query<(Entity, &mut DamageIndicator, &mut UiTransform, &mut BackgroundColor)>,
    player: Single<(&Player, &Transform)>,
    time: Res<Time>,
) {
    let (p, tf) = *player;
    for (e, mut ind, mut ui, mut bg) in &mut indicators {
        ind.life -= time.delta_secs();
        if ind.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let a = hit_angle(p.yaw, tf.translation, ind.from);
        ui.translation = Val2::px(a.sin() * INDICATOR_RADIUS, -a.cos() * INDICATOR_RADIUS);
        ui.rotation = Rot2::radians(a);
        bg.0 = Color::srgba(1.0, 0.1, 0.05, 0.9 * (ind.life / INDICATOR_LIFE).min(1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn hit_angle_points_the_right_way() {
        let me = Vec3::ZERO;
        // Facing -Z (yaw 0): ahead is -Z, right is +X.
        assert!(hit_angle(0.0, me, Vec3::new(0.0, 0.0, -5.0)).abs() < 1e-4);
        assert!((hit_angle(0.0, me, Vec3::new(5.0, 0.0, 0.0)) - FRAC_PI_2).abs() < 1e-4);
        assert!((hit_angle(0.0, me, Vec3::new(-5.0, 0.0, 0.0)) + FRAC_PI_2).abs() < 1e-4);
        assert!((hit_angle(0.0, me, Vec3::new(0.0, 0.0, 5.0)).abs() - PI).abs() < 1e-4);
        // Turn 90° left (yaw +PI/2 faces -X): something at -X is now dead ahead.
        assert!(hit_angle(FRAC_PI_2, me, Vec3::new(-5.0, 0.0, 0.0)).abs() < 1e-4);
    }
}

/// Each frame: turn the damage enemies just took into floating numbers, and give hurt
/// enemies a health bar. Runs after everything that deals damage, before deaths are cleaned up.
pub fn spawn_damage_feedback(
    mut commands: Commands,
    mut enemies: Query<(Entity, &mut Enemy, &Transform)>,
    settings: Res<Settings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut enemy, tf) in &mut enemies {
        enemy.since_hit += dt;
        if enemy.recent <= 0.0 {
            continue;
        }
        let head = tf.translation + Vec3::Y * 2.3 * tf.scale.y;
        if settings.damage_numbers {
            // Shown at 10x so pistol hits read as "10" rather than "1".
            let shown = (enemy.recent * 10.0).round().max(1.0);
            let crit = enemy.recent_crit;
            let jitter = Vec3::new(rand::random::<f32>() - 0.5, rand::random::<f32>() * 0.3, rand::random::<f32>() - 0.5) * 0.5;
            commands.spawn((
                DamageNumber { at: head + jitter, life: NUMBER_LIFE, crit },
                Text::new(if crit { format!("{shown}!") } else { format!("{shown}") }),
                font(if crit { 26.0 } else { 19.0 }),
                TextColor(if crit { Color::srgb(1.0, 0.25, 0.15) } else { Color::srgb(1.0, 0.9, 0.35) }),
                Node { position_type: PositionType::Absolute, display: Display::None, ..default() },
                Pickable::IGNORE,
            ));
        }
        enemy.recent = 0.0;
        enemy.recent_crit = false;

        // Bosses already have the big bar at the top of the screen.
        if enemy.bar.is_none() && !enemies::is_boss(enemy.kind) {
            let width = (46.0 * tf.scale.x).clamp(36.0, 80.0);
            let fill = commands
                .spawn((Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(Color::srgb(0.4, 0.9, 0.3))))
                .id();
            let bar = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(width),
                        height: px(6),
                        border: UiRect::all(px(1)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                    Pickable::IGNORE,
                ))
                .add_child(fill)
                .id();
            commands.entity(bar).insert(EnemyBar { enemy: e, fill, width });
            enemy.bar = Some(bar);
        }
    }
}

/// Where a world point lands on screen, if it's in front of the camera.
fn on_screen(camera: &Camera, cam_tf: &GlobalTransform, at: Vec3) -> Option<Vec2> {
    let ahead = (at - cam_tf.translation()).dot(cam_tf.forward().as_vec3()) > 0.2;
    if !ahead {
        return None;
    }
    camera.world_to_viewport(cam_tf, at).ok()
}

pub fn update_enemy_bars(
    mut commands: Commands,
    mut bars: Query<(Entity, &EnemyBar, &mut Node)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), Without<EnemyBar>>,
    enemies: Query<(&Enemy, &Transform)>,
    camera: Single<(&Camera, &GlobalTransform), With<Player>>,
    state: Res<State<GameState>>,
) {
    let (cam, cam_tf) = *camera;
    for (e, bar, mut node) in &mut bars {
        let Ok((enemy, tf)) = enemies.get(bar.enemy) else {
            commands.entity(e).despawn();
            continue;
        };
        let top = tf.translation + Vec3::Y * 2.55 * tf.scale.y;
        let near = top.distance(cam_tf.translation()) < 45.0;
        let pos = on_screen(cam, cam_tf, top).filter(|_| near && enemy.since_hit < BAR_SHOW_SECS && *state.get() == GameState::Playing);
        let Some(p) = pos else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(p.x - bar.width / 2.0);
        node.top = px(p.y - 6.0);
        if let Ok((mut fill, mut color)) = fills.get_mut(bar.fill) {
            let f = (enemy.health / enemy.max_health).clamp(0.0, 1.0);
            fill.width = percent(100.0 * f);
            // Green when healthy, through yellow, to red when nearly dead.
            color.0 = if f > 0.5 {
                Color::srgb(0.4 + (1.0 - f) * 1.0, 0.9, 0.3)
            } else {
                Color::srgb(1.0, 0.25 + f * 1.3, 0.2)
            };
        }
    }
}

pub fn update_damage_numbers(
    mut commands: Commands,
    mut numbers: Query<(Entity, &mut DamageNumber, &mut Node, &mut TextColor)>,
    camera: Single<(&Camera, &GlobalTransform), With<Player>>,
    time: Res<Time>,
) {
    let (cam, cam_tf) = *camera;
    let dt = time.delta_secs();
    for (e, mut n, mut node, mut color) in &mut numbers {
        n.life -= dt;
        if n.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        n.at.y += dt * 1.3;
        match on_screen(cam, cam_tf, n.at) {
            Some(p) => {
                node.display = Display::Flex;
                node.left = px(p.x - 10.0);
                node.top = px(p.y - 12.0);
            }
            None => node.display = Display::None,
        }
        let a = (n.life / NUMBER_LIFE * 1.6).min(1.0);
        color.0 = if n.crit { Color::srgba(1.0, 0.25, 0.15, a) } else { Color::srgba(1.0, 0.9, 0.35, a) };
    }
}

//! Pumpkin Purge — a Halloween festival FPS. Pumpkin-headed scarecrows pour out
//! of the corn maze in waves; fight them off with a candy-themed arsenal.
//!
//! Controls: WASD move, Shift sprint, Space jump, left mouse fire, hold right mouse to
//! aim down sights, F pitchfork, G candy grenade, R reload, 1-4 or mouse wheel to switch weapons, B shop
//! (between waves), Esc to pause (and open settings), R to restart after dying.

mod bat;
mod boss;
mod collision;
mod effects;
mod enemies;
mod festival;
mod hud;
mod loadout;
mod looks;
mod pathfinding;
mod player;
mod powerups;
mod save;
mod settings;
mod shop;
mod sound;
mod weapons;

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Title,
    Playing,
    GameOver,
}

#[derive(Resource, Default)]
pub struct Score {
    pub points: u32,
    pub kills: u32,
    /// Spendable currency for the upgrade shop. Earned alongside points.
    pub candy: u32,
}

fn main() {
    let settings = settings::Settings::load();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pumpkin Purge".into(),
                mode: settings::window_mode(settings.fullscreen),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(settings)
        .insert_resource(save::Best::load())
        .init_resource::<save::Practice>()
        .insert_resource(save::GodMode(std::env::var("PUMPKIN_GOD").is_ok_and(|v| v == "1")))
        .init_resource::<player::Shake>()
        .init_resource::<settings::MenuQueue>()
        .init_resource::<settings::ResetRequest>()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.03, 0.07)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.45, 0.45, 0.65),
            brightness: 160.0,
            ..default()
        })
        .init_state::<GameState>()
        .init_resource::<Score>()
        .init_resource::<enemies::Waves>()
        .init_resource::<weapons::Arsenal>()
        .init_resource::<player::HurtFlash>()
        .init_resource::<shop::Upgrades>()
        .init_resource::<shop::ShopScreen>()
        .init_resource::<shop::ShopQueue>()
        .init_resource::<shop::ShopMessage>()
        .init_resource::<powerups::PowerUps>()
        .add_systems(
            Startup,
            (
                effects::setup_fx,
                sound::setup_sound,
                festival::setup_festival,
                pathfinding::setup_nav,
                player::spawn_player,
                weapons::spawn_viewmodels,
                hud::setup_hud,
                shop::setup_shop_ui,
                powerups::setup_ui,
                settings::setup_menu,
                practice_start,
            )
                .chain(),
        )
        // Always running: menu, input for pausing, effects, and the HUD.
        .add_systems(
            Update,
            (
                settings::handle_menu.before(weapons::fire),
                settings::update_menu,
                settings::apply_settings,
                hud::process_hits,
                hud::update_indicators,
                weapons::fire.before(player::grab_cursor),
                player::grab_cursor,
                effects::update_effects,
                weapons::update_viewmodels,
                hud::update_top,
                hud::update_health,
                hud::update_weapons,
                hud::update_crosshair.after(weapons::update_viewmodels),
                hud::update_hurt,
                hud::update_center,
                hud::update_ammo_hint,
                hud::update_boss_bar,
                powerups::update_ui,
            ),
        )
        // Gameplay
        .add_systems(
            Update,
            (
                player::look_and_move,
                weapons::switch_weapon,
                weapons::reload,
                weapons::melee,
                powerups::tick,
                powerups::magnet,
                weapons::update_gourds,
                weapons::update_pickups,
                enemies::run_waves,
                enemies::move_and_attack,
                boss::king_attacks,
                boss::update_shockwaves,
                boss::witch_attacks,
                boss::update_wisps,
                boss::golem_attacks,
                enemies::animate_limbs,
                enemies::update_seeds,
                enemies::handle_deaths,
                player::check_death,
            )
                .chain()
                .run_if(in_state(GameState::Playing))
                .run_if(not_paused),
        )
        .add_systems(
            Update,
            pathfinding::update_nav
                .before(enemies::move_and_attack)
                .run_if(in_state(GameState::Playing))
                .run_if(not_paused),
        )
        .add_systems(Update, pathfinding::draw_nav_debug)
        .add_systems(
            Update,
            (
                shop::toggle_shop.after(player::grab_cursor),
                shop::handle_shop,
                shop::rebuild_shop,
                shop::update_shop,
            )
                .chain(),
        )
        .init_resource::<pathfinding::NavDebug>()
        // Bats and grenades
        .add_systems(
            Update,
            (bat::bat_behavior, weapons::throw_grenade, weapons::update_grenades)
                .after(enemies::run_waves)
                .before(enemies::handle_deaths)
                .run_if(in_state(GameState::Playing))
                .run_if(not_paused),
        )
        // Damage numbers and health bars: collect after all damage, before the dead are removed.
        .add_systems(
            Update,
            hud::spawn_damage_feedback
                .after(weapons::fire)
                .after(weapons::melee)
                .after(weapons::update_gourds)
                .after(weapons::update_grenades)
                .after(bat::bat_behavior)
                .before(enemies::handle_deaths)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (hud::update_enemy_bars, hud::update_damage_numbers, effects::update_goo))
        .add_systems(OnEnter(GameState::GameOver), save::record_best)
        .add_systems(Update, restart)
        .run();
}

fn not_paused(time: Res<Time<Virtual>>) -> bool {
    !time.is_paused()
}

/// Start a fresh run: R after dying, or Restart / Quit to Title from the pause menu.
fn restart(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut request: ResMut<settings::ResetRequest>,
    mut practice: ResMut<save::Practice>,
    god: Res<save::GodMode>,
    mut next: ResMut<NextState<GameState>>,
    mut player: Single<(&mut player::Player, &mut Transform)>,
    leftovers: Query<
        Entity,
        Or<(
            With<enemies::Enemy>,
            With<enemies::Seed>,
            With<weapons::Pickup>,
            With<boss::Shockwave>,
            With<boss::Wisp>,
            With<weapons::Grenade>,
            With<effects::Goo>,
        )>,
    >,
) {
    let target = if *state.get() == GameState::GameOver && keys.just_pressed(KeyCode::KeyR) {
        GameState::Playing
    } else if let Some(target) = request.0.take() {
        target
    } else {
        return;
    };
    practice.0 = god.0;
    for e in &leftovers {
        commands.entity(e).despawn();
    }
    commands.insert_resource(Score::default());
    commands.insert_resource(enemies::Waves::default());
    commands.insert_resource(weapons::Arsenal::default());
    commands.insert_resource(shop::Upgrades::default());
    commands.insert_resource(shop::ShopScreen::default());
    commands.insert_resource(powerups::PowerUps::default());
    let (ref mut p, ref mut tf) = *player;
    **p = player::Player::new();
    tf.translation = Vec3::new(0.0, player::EYE_HEIGHT, 10.0);
    next.set(target);
}

/// Practice mode: launch with `PUMPKIN_WAVE=5` (or any wave) to start at the upgrade
/// shop right before that wave, with 2,000 candy to spend and one of each power-up
/// waiting in front of you.
fn practice_start(
    mut commands: Commands,
    mut waves: ResMut<enemies::Waves>,
    mut score: ResMut<Score>,
    mut practice: ResMut<save::Practice>,
    mut arsenal: ResMut<weapons::Arsenal>,
    god: Res<save::GodMode>,
    fx: Res<effects::Fx>,
) {
    if god.0 {
        practice.0 = true;
    }
    let Some(wave) = std::env::var("PUMPKIN_WAVE").ok().and_then(|v| v.parse::<u32>().ok()) else {
        return;
    };
    practice.0 = true;
    if wave > 1 {
        waves.wave = wave - 1;
        waves.intermission = enemies::SHOP_TIME;
        score.candy = 2000;
    }
    // Practice runs start with every weapon so you can test the bosses right away.
    arsenal.loadout.owned_weapon = [true; 4];
    use weapons::PickupKind::*;
    for (x, kind) in [(-3.0, DoubleDamage), (0.0, Speed), (3.0, Magnet)] {
        weapons::spawn_pickup(&mut commands, &fx, kind, Vec3::new(x, 0.0, 5.0));
    }
}

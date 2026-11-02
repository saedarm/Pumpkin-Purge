//! Settings and the menu.
//!
//! The same menu panel shows on the title screen (with Play) and when paused with Esc
//! (with Resume / Restart / Quit to Title). Every setting has `<` and `>` buttons and is
//! saved to disk as soon as it changes.

use bevy::audio::{AudioSinkPlayback, GlobalVolume, Volume};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, PrimaryWindow, WindowMode};

use crate::save::{self, Best};
use crate::sound::Music;
use crate::weapons::Arsenal;
use crate::GameState;

const FILE: &str = "settings.txt";
/// The music's own level before the music-volume setting is applied.
const MUSIC_BASE: f32 = 0.45;

#[derive(Resource, Clone, PartialEq, Debug)]
pub struct Settings {
    /// Multiplier on mouse look speed.
    pub sensitivity: f32,
    pub invert_y: bool,
    /// Field of view in degrees.
    pub fov: f32,
    /// 0-100
    pub master_volume: u32,
    /// 0-100
    pub music_volume: u32,
    /// 0 = off, 1 = low, 2 = full
    pub shake: u32,
    pub fullscreen: bool,
    /// 0 = Easy, 1 = Normal, 2 = Hard. Only changeable from the title screen.
    pub difficulty: u32,
    pub damage_numbers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 1.0,
            invert_y: false,
            fov: 80.0,
            master_volume: 80,
            music_volume: 60,
            shake: 2,
            fullscreen: false,
            difficulty: 1,
            damage_numbers: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let m = save::load(FILE);
        let d = Self::default();
        let mut s = Self {
            sensitivity: save::get(&m, "sensitivity", d.sensitivity),
            invert_y: save::get(&m, "invert_y", d.invert_y),
            fov: save::get(&m, "fov", d.fov),
            master_volume: save::get(&m, "master_volume", d.master_volume),
            music_volume: save::get(&m, "music_volume", d.music_volume),
            shake: save::get(&m, "shake", d.shake),
            fullscreen: save::get(&m, "fullscreen", d.fullscreen),
            difficulty: save::get(&m, "difficulty", d.difficulty),
            damage_numbers: save::get(&m, "damage_numbers", d.damage_numbers),
        };
        // Clamp anything hand-edited out of range.
        for i in 0..ROWS.len() {
            s.adjust(i, 0);
        }
        s
    }

    fn save(&self) {
        save::store(
            FILE,
            &[
                ("sensitivity", format!("{:.1}", self.sensitivity)),
                ("invert_y", self.invert_y.to_string()),
                ("fov", format!("{:.0}", self.fov)),
                ("master_volume", self.master_volume.to_string()),
                ("music_volume", self.music_volume.to_string()),
                ("shake", self.shake.to_string()),
                ("fullscreen", self.fullscreen.to_string()),
                ("difficulty", self.difficulty.to_string()),
                ("damage_numbers", self.damage_numbers.to_string()),
            ],
        );
    }

    /// Enemy health multiplier for the chosen difficulty.
    pub fn enemy_health_mult(&self) -> f32 {
        [0.7, 1.0, 1.4][self.difficulty.min(2) as usize]
    }

    /// Multiplier on all damage you take, for the chosen difficulty.
    pub fn damage_taken_mult(&self) -> f32 {
        [0.6, 1.0, 1.4][self.difficulty.min(2) as usize]
    }

    /// Screen-shake strength: 0, 0.5, or 1.
    pub fn shake_amount(&self) -> f32 {
        self.shake as f32 / 2.0
    }

    /// Step setting `row` by `dir` (-1, 0, or +1), clamped to its range.
    pub fn adjust(&mut self, row: usize, dir: i32) {
        let d = dir as f32;
        match row {
            0 => self.sensitivity = ((self.sensitivity + 0.1 * d).clamp(0.1, 3.0) * 10.0).round() / 10.0,
            1 => {
                if dir != 0 {
                    self.invert_y = !self.invert_y;
                }
            }
            2 => self.fov = (self.fov + 5.0 * d).clamp(60.0, 110.0),
            3 => self.master_volume = (self.master_volume as i32 + 10 * dir).clamp(0, 100) as u32,
            4 => self.music_volume = (self.music_volume as i32 + 10 * dir).clamp(0, 100) as u32,
            5 => self.shake = (self.shake as i32 + dir).clamp(0, 2) as u32,
            6 => {
                if dir != 0 {
                    self.fullscreen = !self.fullscreen;
                }
            }
            7 => self.difficulty = (self.difficulty as i32 + dir).clamp(0, 2) as u32,
            _ => {
                if dir != 0 {
                    self.damage_numbers = !self.damage_numbers;
                }
            }
        }
    }

    fn value(&self, row: usize) -> String {
        let on_off = |b: bool| if b { "On" } else { "Off" }.to_string();
        match row {
            0 => format!("{:.1}x", self.sensitivity),
            1 => on_off(self.invert_y),
            2 => format!("{:.0}", self.fov),
            3 => format!("{}%", self.master_volume),
            4 => format!("{}%", self.music_volume),
            5 => ["Off", "Low", "Full"][self.shake.min(2) as usize].to_string(),
            6 => on_off(self.fullscreen),
            7 => difficulty_name(self.difficulty).to_string(),
            _ => on_off(self.damage_numbers),
        }
    }
}

const ROWS: [&str; 9] = [
    "Mouse sensitivity",
    "Invert mouse Y",
    "Field of view",
    "Master volume",
    "Music volume",
    "Screen shake",
    "Fullscreen",
    "Difficulty",
    "Damage numbers",
];
pub const DIFFICULTY_ROW: usize = 7;

pub fn difficulty_name(d: u32) -> &'static str {
    ["Easy", "Normal", "Hard"][d.min(2) as usize]
}

pub fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
}

/// Push setting changes into the game and save them.
pub fn apply_settings(
    settings: Res<Settings>,
    mut global: ResMut<GlobalVolume>,
    mut music: Query<&mut AudioSink, With<Music>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    // The music sink only exists once playback starts, so also catch it then.
    let changed = settings.is_changed();
    let music_started = music.iter_mut().any(|sink| sink.is_added());
    if !changed && !music_started {
        return;
    }
    let master = settings.master_volume as f32 / 100.0;
    global.volume = Volume::Linear(master);
    for mut sink in &mut music {
        sink.set_volume(Volume::Linear(MUSIC_BASE * master * settings.music_volume as f32 / 100.0));
    }
    if !changed {
        return;
    }
    let mode = window_mode(settings.fullscreen);
    if window.mode != mode {
        window.mode = mode;
    }
    settings.save();
}

// ---------------------------------------------------------------- menu UI

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Play,
    Resume,
    Restart,
    QuitToTitle,
    QuitGame,
    Adjust(usize, i32),
}

/// Button clicks queue up here and get handled in `handle_menu`.
#[derive(Resource, Default)]
pub struct MenuQueue(Vec<Action>);

/// Set by `handle_menu` when the run should reset; `main.rs` does the resetting.
#[derive(Resource, Default)]
pub struct ResetRequest(pub Option<GameState>);

#[derive(Component)]
pub struct MenuPanel;
#[derive(Component)]
pub struct MenuTitle;
#[derive(Component)]
pub struct MenuSubtitle;
#[derive(Component)]
pub struct TitleOnly;
#[derive(Component)]
pub struct PauseOnly;
#[derive(Component)]
pub struct SettingValue(usize);
#[derive(Component)]
pub struct MenuButton;

const ORANGE: Color = Color::srgb(1.0, 0.6, 0.15);
const BUTTON: Color = Color::srgba(0.35, 0.15, 0.05, 0.9);
const BUTTON_HOVER: Color = Color::srgba(0.7, 0.3, 0.05, 1.0);

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

fn button(p: &mut ChildSpawnerCommands, label: &str, action: Action, width: f32) -> Entity {
    p.spawn((
        MenuButton,
        Hovered::default(),
        Node {
            width: px(width),
            height: px(30),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON),
    ))
    .with_children(|b| {
        b.spawn((Text::new(label), font(18.0), TextColor(Color::WHITE), Pickable::IGNORE));
    })
    .observe(move |click: On<PointerClick>, mut queue: ResMut<MenuQueue>| {
        if click.button == PointerButton::Primary {
            queue.0.push(action);
        }
    })
    .id()
}

pub fn setup_menu(mut commands: Commands) {
    commands
        .spawn((
            MenuPanel,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(5),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.01, 0.04, 0.94)),
        ))
        .with_children(|p| {
            p.spawn((MenuTitle, Text::new("PUMPKIN PURGE"), font(48.0), TextColor(ORANGE)));
            p.spawn((
                MenuSubtitle,
                Text::new(""),
                font(19.0),
                TextColor(Color::srgb(1.0, 0.85, 0.6)),
                TextLayout::justify(Justify::Center),
            ));

            // Buttons: Play on the title screen; Resume/Restart/Quit to Title when paused.
            p.spawn(Node { column_gap: px(10), margin: UiRect::vertical(px(8)), ..default() }).with_children(|row| {
                row.spawn((TitleOnly, Node { column_gap: px(10), ..default() })).with_children(|r| {
                    button(r, "PLAY", Action::Play, 160.0);
                });
                row.spawn((PauseOnly, Node { column_gap: px(10), ..default() })).with_children(|r| {
                    button(r, "RESUME", Action::Resume, 140.0);
                    button(r, "RESTART", Action::Restart, 140.0);
                    button(r, "QUIT TO TITLE", Action::QuitToTitle, 170.0);
                });
                button(row, "QUIT GAME", Action::QuitGame, 140.0);
            });

            p.spawn((Text::new("SETTINGS"), font(22.0), TextColor(ORANGE)));
            for (i, label) in ROWS.iter().enumerate() {
                p.spawn(Node { column_gap: px(10), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((Text::new(*label), font(18.0), TextColor(Color::WHITE), Node { width: px(220), ..default() }));
                    button(row, "<", Action::Adjust(i, -1), 36.0);
                    row.spawn((
                        SettingValue(i),
                        Text::new(""),
                        font(19.0),
                        TextColor(ORANGE),
                        TextLayout::justify(Justify::Center),
                        Node { width: px(100), ..default() },
                    ));
                    button(row, ">", Action::Adjust(i, 1), 36.0);
                });
            }

            p.spawn((
                TitleOnly,
                Text::new(
                    "WASD move  |  Shift sprint  |  Space jump  |  Click fire  |  Right-click aim  |  F pitchfork  |  G grenade\n\
                     R reload  |  1-4 or wheel switch weapons  |  B shop between waves  |  Esc pause",
                ),
                font(15.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                TextLayout::justify(Justify::Center),
                Node { margin: UiRect::top(px(14)), ..default() },
            ));
        });
}

pub fn menu_open(state: &GameState, paused: bool) -> bool {
    *state == GameState::Title || (*state == GameState::Playing && paused)
}

pub fn update_menu(
    mut panel: Single<&mut Node, With<MenuPanel>>,
    mut title_only: Query<&mut Node, (With<TitleOnly>, Without<MenuPanel>, Without<PauseOnly>)>,
    mut pause_only: Query<&mut Node, (With<PauseOnly>, Without<MenuPanel>, Without<TitleOnly>)>,
    mut title: Single<&mut Text, (With<MenuTitle>, Without<MenuSubtitle>, Without<SettingValue>)>,
    mut subtitle: Single<&mut Text, (With<MenuSubtitle>, Without<MenuTitle>, Without<SettingValue>)>,
    mut values: Query<(&SettingValue, &mut Text, &mut TextColor), (Without<MenuTitle>, Without<MenuSubtitle>)>,
    mut buttons: Query<(&Hovered, &mut BackgroundColor), With<MenuButton>>,
    settings: Res<Settings>,
    best: Res<Best>,
    shop: Res<crate::shop::ShopScreen>,
    state: Res<State<GameState>>,
    time: Res<Time<Virtual>>,
) {
    let on_title = *state.get() == GameState::Title;
    // The shop also pauses time; don't show the pause menu over it.
    let open = menu_open(state.get(), time.is_paused() && !shop.open);
    panel.display = if open { Display::Flex } else { Display::None };
    if !open {
        return;
    }
    let show = |b: bool| if b { Display::Flex } else { Display::None };
    for mut n in &mut title_only {
        n.display = show(on_title);
    }
    for mut n in &mut pause_only {
        n.display = show(!on_title);
    }
    title.0 = if on_title { "PUMPKIN PURGE" } else { "PAUSED" }.to_string();
    subtitle.0 = if on_title {
        let d = settings.difficulty;
        let b = best.get(d);
        let record = if b.score > 0 {
            format!("Best on {}:  wave {}  |  score {}  |  kills {}", difficulty_name(d), b.wave, b.score, b.kills)
        } else {
            format!("No best score on {} yet", difficulty_name(d))
        };
        format!("The jack-o'-lanterns have taken over the festival.\n\n{record}")
    } else {
        String::new()
    };
    for (v, mut text, mut color) in &mut values {
        text.0 = settings.value(v.0);
        // Difficulty is locked during a run (pick it on the title screen).
        let locked = v.0 == DIFFICULTY_ROW && !on_title;
        color.0 = if locked { Color::srgba(1.0, 0.6, 0.15, 0.35) } else { ORANGE };
    }
    for (hovered, mut bg) in &mut buttons {
        bg.0 = if hovered.get() { BUTTON_HOVER } else { BUTTON };
    }
}

fn lock(cursor: &mut CursorOptions) {
    cursor.grab_mode = CursorGrabMode::Locked;
    cursor.visible = false;
}

pub fn handle_menu(
    mut queue: ResMut<MenuQueue>,
    mut settings: ResMut<Settings>,
    mut reset: ResMut<ResetRequest>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut time: ResMut<Time<Virtual>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut arsenal: ResMut<Arsenal>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in std::mem::take(&mut queue.0) {
        match action {
            Action::Adjust(row, dir) => {
                if row != DIFFICULTY_ROW || *state.get() == GameState::Title {
                    settings.adjust(row, dir);
                }
            }
            Action::Play => {
                lock(&mut cursor);
                next.set(GameState::Playing);
            }
            Action::Resume => {
                lock(&mut cursor);
                time.unpause();
            }
            Action::Restart => {
                lock(&mut cursor);
                time.unpause();
                reset.0 = Some(GameState::Playing);
            }
            Action::QuitToTitle => {
                time.unpause();
                reset.0 = Some(GameState::Title);
            }
            Action::QuitGame => {
                exit.write(AppExit::Success);
            }
        }
        if !matches!(action, Action::Adjust(..)) {
            // Don't let the click that closed the menu also fire a shot.
            arsenal.block_fire(0.25);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_clamp_to_their_ranges() {
        let mut s = Settings::default();
        for _ in 0..100 {
            s.adjust(0, 1);
            s.adjust(2, -1);
            s.adjust(3, 1);
            s.adjust(5, 1);
        }
        assert_eq!((s.sensitivity, s.fov, s.master_volume, s.shake), (3.0, 60.0, 100, 2));
        for _ in 0..100 {
            s.adjust(0, -1);
        }
        assert_eq!(s.sensitivity, 0.1);
        s.adjust(6, 1);
        assert!(s.fullscreen);
        s.adjust(6, 0);
        assert!(s.fullscreen, "a zero step must not toggle");
        s.adjust(7, -5);
        assert_eq!(s.difficulty, 0);
        s.adjust(7, 9);
        assert_eq!(s.difficulty, 2);
        assert!(s.enemy_health_mult() > 1.0 && s.damage_taken_mult() > 1.0);
        s.adjust(8, 1);
        assert!(!s.damage_numbers);
    }

    #[test]
    fn sensitivity_steps_stay_clean() {
        let mut s = Settings::default();
        for _ in 0..7 {
            s.adjust(0, 1);
        }
        assert_eq!(s.value(0), "1.7x");
    }
}

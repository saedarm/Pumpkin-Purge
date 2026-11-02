//! The shop. Between waves, press B to open it (the break timer stops while it's open).
//!
//! - **Upgrades tab:** permanent player upgrades (health, damage, reload, mags, armor, ammo).
//! - **One tab per weapon:** unlock the weapon, then buy and swap attachments. Hovering an
//!   attachment previews exactly which stats it changes before you buy it.
//!
//! Enter starts the next wave early, from the shop or from the field.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::enemies::{self, Waves};
use crate::loadout::{self, SLOTS, WEAPON_COST};
use crate::player::Player;
use crate::sound::{self, Sfx};
use crate::weapons::{Arsenal, WEAPONS};
use crate::{GameState, Score};

pub struct Item {
    pub name: &'static str,
    pub effect: &'static str,
    /// Highest level you can buy. 0 = unlimited (a consumable).
    pub max_level: u32,
    pub base_cost: u32,
}

pub const ITEMS: [Item; 7] = [
    Item { name: "Big Bag of Candy", effect: "+25 max health, full heal", max_level: 5, base_cost: 150 },
    Item { name: "Sharper Kernels", effect: "+15% damage, every weapon", max_level: 5, base_cost: 200 },
    Item { name: "Butter Fingers", effect: "-15% reload time", max_level: 3, base_cost: 150 },
    Item { name: "Deep Pockets", effect: "+25% magazine size", max_level: 3, base_cost: 175 },
    Item { name: "Scarecrow Armor", effect: "-10% damage taken", max_level: 4, base_cost: 200 },
    Item { name: "Ammo Refill", effect: "fill every weapon you own", max_level: 0, base_cost: 100 },
    Item { name: "Candy Grenade", effect: "+1 grenade (carry up to 5)", max_level: 0, base_cost: 80 },
];
const GRENADE_ITEM: usize = 6;

const WEAPON_BLURBS: [&str; 4] = [
    "Reliable sidearm. Never runs out of spare ammo.",
    "Nine pellets per shot. Devastating up close, shoves scarecrows back.",
    "Full auto. Hold the trigger and hose down a crowd.",
    "Lobs an exploding gourd. Splash damage hits everything nearby.",
];

#[derive(Resource, Default)]
pub struct Upgrades {
    pub levels: [u32; 7],
}

impl Upgrades {
    pub fn cost(&self, i: usize) -> u32 {
        // Consumables keep a flat price; upgrades get pricier each level.
        if ITEMS[i].max_level == 0 { ITEMS[i].base_cost } else { ITEMS[i].base_cost * (self.levels[i] + 1) }
    }

    pub fn maxed(&self, i: usize) -> bool {
        ITEMS[i].max_level > 0 && self.levels[i] >= ITEMS[i].max_level
    }
}

/// True during the break between waves (after wave 1).
pub fn shop_open(waves: &Waves) -> bool {
    waves.wave > 0 && waves.intermission > 0.0
}

/// Apply one level of upgrade `i`.
fn apply(i: usize, level: u32, player: &mut Player, arsenal: &mut Arsenal) {
    match i {
        0 => {
            player.max_health += 25;
            player.health = player.max_health;
        }
        1 => arsenal.damage_mult = 1.0 + 0.15 * level as f32,
        2 => arsenal.reload_mult = 0.85f32.powi(level as i32),
        3 => arsenal.mag_mult = 1.0 + 0.25 * level as f32,
        4 => player.armor = 0.1 * level as f32,
        GRENADE_ITEM => arsenal.grenades = (arsenal.grenades + 1).min(crate::weapons::MAX_GRENADES),
        _ => {
            for w in 0..WEAPONS.len() {
                if arsenal.loadout.owned_weapon[w] {
                    arsenal.mag[w] = arsenal.mag_size(w);
                    if !Arsenal::infinite(w) {
                        arsenal.reserve[w] = WEAPONS[w].max_reserve;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- state & actions

/// Whether the shop screen is up, and which tab: 0 = Upgrades, 1-4 = weapons.
#[derive(Resource, Default)]
pub struct ShopScreen {
    pub open: bool,
    tab: usize,
    dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Tab(usize),
    BuyUpgrade(usize),
    BuyWeapon(usize),
    /// Weapon, slot, option (None = take it off).
    Attach(usize, usize, Option<u8>),
    StartWave,
    Close,
}

#[derive(Resource, Default)]
pub struct ShopQueue(Vec<Action>);

/// Why a purchase didn't go through, for the error message line.
#[derive(Debug, PartialEq)]
enum Refusal {
    TooPoor,
    Maxed,
}

/// Buy (or equip, if already owned) an attachment. Returns whether anything changed.
fn attach(arsenal: &mut Arsenal, candy: &mut u32, w: usize, slot: usize, opt: Option<u8>) -> Result<bool, Refusal> {
    let lo = &mut arsenal.loadout;
    if lo.equipped[w][slot] == opt {
        return Ok(false);
    }
    if let Some(o) = opt {
        if !lo.owns(w, slot, o) {
            let cost = loadout::options(w, slot)[o as usize].cost;
            if *candy < cost {
                return Err(Refusal::TooPoor);
            }
            *candy -= cost;
            lo.give(w, slot, o);
        }
    }
    lo.equipped[w][slot] = opt;
    arsenal.clamp_mags();
    Ok(true)
}

fn buy_weapon(arsenal: &mut Arsenal, candy: &mut u32, w: usize) -> Result<bool, Refusal> {
    if arsenal.loadout.owned_weapon[w] {
        return Ok(false);
    }
    if *candy < WEAPON_COST[w] {
        return Err(Refusal::TooPoor);
    }
    *candy -= WEAPON_COST[w];
    arsenal.loadout.owned_weapon[w] = true;
    arsenal.mag[w] = arsenal.mag_size(w);
    Ok(true)
}

fn buy_upgrade(upgrades: &mut Upgrades, candy: &mut u32, i: usize, player: &mut Player, arsenal: &mut Arsenal) -> Result<bool, Refusal> {
    if upgrades.maxed(i) || (i == GRENADE_ITEM && arsenal.grenades >= crate::weapons::MAX_GRENADES) {
        return Err(Refusal::Maxed);
    }
    let cost = upgrades.cost(i);
    if *candy < cost {
        return Err(Refusal::TooPoor);
    }
    *candy -= cost;
    upgrades.levels[i] += 1;
    apply(i, upgrades.levels[i], player, arsenal);
    Ok(true)
}

fn lock(cursor: &mut CursorOptions) {
    cursor.grab_mode = CursorGrabMode::Locked;
    cursor.visible = false;
}

/// B opens/closes the shop during a break; Esc closes it; Enter starts the next wave.
/// Runs after `player::grab_cursor`, which ignores Esc while the shop is open.
pub fn toggle_shop(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut shop: ResMut<ShopScreen>,
    mut queue: ResMut<ShopQueue>,
    mut waves: ResMut<Waves>,
    mut time: ResMut<Time<Virtual>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if *state.get() != GameState::Playing {
        shop.open = false;
        return;
    }
    if shop.open {
        if keys.just_pressed(KeyCode::KeyB) || keys.just_pressed(KeyCode::Escape) {
            queue.0.push(Action::Close);
        } else if keys.just_pressed(KeyCode::Enter) {
            queue.0.push(Action::StartWave);
        }
        return;
    }
    // Paused by the menu: leave it alone.
    if time.is_paused() || !shop_open(&waves) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        shop.open = true;
        shop.dirty = true;
        time.pause();
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if keys.just_pressed(KeyCode::Enter) {
        waves.intermission = 0.01;
    }
}

pub fn handle_shop(
    mut commands: Commands,
    mut queue: ResMut<ShopQueue>,
    mut shop: ResMut<ShopScreen>,
    mut arsenal: ResMut<Arsenal>,
    mut upgrades: ResMut<Upgrades>,
    mut score: ResMut<Score>,
    mut waves: ResMut<Waves>,
    mut player: Single<&mut Player>,
    mut time: ResMut<Time<Virtual>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut message: ResMut<ShopMessage>,
    sfx: Res<Sfx>,
) {
    for action in std::mem::take(&mut queue.0) {
        let result = match action {
            Action::Tab(t) => {
                shop.tab = t;
                shop.dirty = true;
                message.0.clear();
                continue;
            }
            Action::Close | Action::StartWave => {
                if action == Action::StartWave {
                    waves.intermission = 0.01;
                }
                shop.open = false;
                time.unpause();
                lock(&mut cursor);
                arsenal.block_fire(0.25);
                continue;
            }
            Action::BuyUpgrade(i) => buy_upgrade(&mut upgrades, &mut score.candy, i, &mut player, &mut arsenal),
            Action::BuyWeapon(w) => buy_weapon(&mut arsenal, &mut score.candy, w),
            Action::Attach(w, slot, opt) => attach(&mut arsenal, &mut score.candy, w, slot, opt),
        };
        match result {
            Ok(true) => {
                sound::play(&mut commands, &sfx.pickup, 0.8);
                message.0.clear();
            }
            Ok(false) => {}
            Err(why) => {
                sound::play(&mut commands, &sfx.empty, 0.9);
                message.0 = match why {
                    Refusal::TooPoor => "Not enough candy.".to_string(),
                    Refusal::Maxed => "Already maxed out (or carrying as many as you can).".to_string(),
                };
            }
        }
        shop.dirty = true;
    }
}

/// Short feedback line under the shop header ("Not enough candy.").
#[derive(Resource, Default)]
pub struct ShopMessage(String);

// ---------------------------------------------------------------- UI

#[derive(Component)]
pub struct ShopRoot;
#[derive(Component)]
pub struct ShopHeader;
#[derive(Component)]
pub struct ShopTabs;
#[derive(Component)]
pub struct ShopBody;
#[derive(Component)]
pub struct ShopStats;
#[derive(Component)]
pub struct ShopInfo;
#[derive(Component)]
pub struct ShopPrompt;
/// A clickable shop button; `base` is its color when not hovered.
#[derive(Component)]
pub struct ShopButton {
    base: Color,
}
/// Hovering this button previews this attachment on the stats panel.
#[derive(Component)]
pub struct Preview {
    weapon: usize,
    slot: usize,
    opt: Option<u8>,
}

const ORANGE: Color = Color::srgb(1.0, 0.6, 0.15);
const BTN: Color = Color::srgba(0.35, 0.15, 0.05, 0.9);
const BTN_ON: Color = Color::srgba(0.15, 0.45, 0.12, 0.95);
const BTN_TAB: Color = Color::srgba(0.75, 0.33, 0.05, 1.0);
const BTN_HOVER: Color = Color::srgba(0.8, 0.4, 0.1, 1.0);
const TEXT_DIM: Color = Color::srgba(1.0, 0.85, 0.75, 0.55);
const TEXT_POOR: Color = Color::srgb(1.0, 0.5, 0.45);

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

fn button<'a>(
    p: &'a mut ChildSpawnerCommands,
    label: &str,
    action: Action,
    width: Option<f32>,
    base: Color,
    text: Color,
) -> EntityCommands<'a> {
    let mut b = p.spawn((
        ShopButton { base },
        Hovered::default(),
        Node {
            width: width.map(px).unwrap_or(Val::Auto),
            min_height: px(30),
            padding: UiRect::axes(px(10), px(4)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(base),
    ));
    b.with_children(|t| {
        t.spawn((Text::new(label), font(16.0), TextColor(text), Pickable::IGNORE));
    });
    b.observe(move |click: On<PointerClick>, mut queue: ResMut<ShopQueue>| {
        if click.button == PointerButton::Primary {
            queue.0.push(action);
        }
    });
    b
}

pub fn setup_shop_ui(mut commands: Commands) {
    // The prompt shown on the field during a break.
    commands.spawn((
        ShopPrompt,
        Text::new(""),
        font(21.0),
        TextColor(Color::srgb(1.0, 0.85, 0.6)),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, top: percent(33), width: percent(100), ..default() },
        Pickable::IGNORE,
    ));

    commands
        .spawn((
            ShopRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(32), px(22)),
                row_gap: px(12),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.03, 0.015, 0.05)),
        ))
        .with_children(|p| {
            p.spawn(Node { align_items: AlignItems::Center, column_gap: px(24), ..default() }).with_children(|row| {
                row.spawn((Text::new("SHOP"), font(40.0), TextColor(ORANGE)));
                row.spawn((ShopHeader, Text::new(""), font(19.0), TextColor(Color::srgb(1.0, 0.85, 0.6))));
            });
            p.spawn((ShopTabs, Node { column_gap: px(8), flex_wrap: FlexWrap::Wrap, row_gap: px(6), ..default() }));
            p.spawn((ShopBody, Node { flex_grow: 1.0, column_gap: px(30), ..default() }));
            p.spawn(Node { column_gap: px(12), ..default() }).with_children(|row| {
                button(row, "START NEXT WAVE  (Enter)", Action::StartWave, None, BTN, Color::WHITE);
                button(row, "CLOSE  (B)", Action::Close, None, BTN, Color::WHITE);
            });
        });
}

/// Rebuild the tabs and body after anything changes (tab switch, purchase, equip).
pub fn rebuild_shop(
    mut commands: Commands,
    mut shop: ResMut<ShopScreen>,
    tabs: Single<Entity, With<ShopTabs>>,
    body: Single<Entity, With<ShopBody>>,
    arsenal: Res<Arsenal>,
    upgrades: Res<Upgrades>,
    score: Res<Score>,
) {
    if !shop.open || !shop.dirty {
        return;
    }
    shop.dirty = false;
    let candy = score.candy;

    commands.entity(*tabs).despawn_children().with_children(|t| {
        let names = ["UPGRADES", "PISTOL", "SCATTERGUN", "SMG", "LAUNCHER"];
        for (i, name) in names.iter().enumerate() {
            let locked = i > 0 && !arsenal.loadout.owned_weapon[i - 1];
            let label = if locked { format!("{name}  (locked)") } else { name.to_string() };
            let base = if shop.tab == i { BTN_TAB } else { BTN };
            button(t, &label, Action::Tab(i), Some(if locked { 190.0 } else { 150.0 }), base, Color::WHITE);
        }
    });

    commands.entity(*body).despawn_children().with_children(|b| {
        if shop.tab == 0 {
            build_upgrades(b, &upgrades, candy, arsenal.grenades);
        } else {
            build_weapon(b, shop.tab - 1, &arsenal, candy);
        }
    });
}

fn build_upgrades(b: &mut ChildSpawnerCommands, upgrades: &Upgrades, candy: u32, arsenal_grenades: u32) {
    b.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() }).with_children(|col| {
        col.spawn((Text::new("Player upgrades last for the rest of the run."), font(17.0), TextColor(TEXT_DIM)));
        for (i, item) in ITEMS.iter().enumerate() {
            col.spawn(Node { column_gap: px(14), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                row.spawn((Text::new(item.name), font(18.0), TextColor(Color::WHITE), Node { width: px(200), ..default() }));
                row.spawn((Text::new(item.effect), font(17.0), TextColor(TEXT_DIM), Node { width: px(270), ..default() }));
                let level = if i == GRENADE_ITEM {
                    format!("Have {}", arsenal_grenades)
                } else if item.max_level == 0 {
                    String::new()
                } else {
                    format!("Lv {}/{}", upgrades.levels[i], item.max_level)
                };
                row.spawn((Text::new(level), font(17.0), TextColor(ORANGE), Node { width: px(80), ..default() }));
                if upgrades.maxed(i) {
                    row.spawn((Text::new("MAXED"), font(17.0), TextColor(TEXT_DIM)));
                } else {
                    let cost = upgrades.cost(i);
                    let color = if candy >= cost { Color::WHITE } else { TEXT_POOR };
                    button(row, &format!("BUY  {cost}"), Action::BuyUpgrade(i), Some(130.0), BTN, color);
                }
            });
        }
    });
}

fn build_weapon(b: &mut ChildSpawnerCommands, w: usize, arsenal: &Arsenal, candy: u32) {
    let owned = arsenal.loadout.owned_weapon[w];

    // Left column: name, blurb, live stats, and the hover info line.
    b.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(10), width: px(430), ..default() }).with_children(|col| {
        col.spawn((Text::new(WEAPONS[w].name), font(26.0), TextColor(ORANGE)));
        col.spawn((Text::new(WEAPON_BLURBS[w]), font(16.0), TextColor(TEXT_DIM)));
        col.spawn((ShopStats, Text::new(""), font(17.0), TextColor(Color::WHITE)));
        col.spawn((ShopInfo, Text::new(""), font(16.0), TextColor(Color::srgb(0.7, 1.0, 0.6))));
    });

    // Right column: unlock button, or the five attachment slots.
    b.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(10), flex_grow: 1.0, ..default() }).with_children(|col| {
        if !owned {
            let cost = WEAPON_COST[w];
            let color = if candy >= cost { Color::WHITE } else { TEXT_POOR };
            col.spawn((Text::new("Locked. Comes with a full magazine and spare ammo."), font(17.0), TextColor(TEXT_DIM)));
            button(col, &format!("UNLOCK  -  {cost} candy"), Action::BuyWeapon(w), Some(280.0), BTN, color);
            return;
        }
        col.spawn((Text::new("Hover an attachment to preview it. Click to buy (once) or swap."), font(15.0), TextColor(TEXT_DIM)));
        for (s, slot) in SLOTS.iter().enumerate() {
            col.spawn(Node { column_gap: px(8), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: px(4), ..default() })
                .with_children(|row| {
                    row.spawn((Text::new(slot.name()), font(18.0), TextColor(ORANGE), Node { width: px(100), ..default() }));
                    let equipped = arsenal.loadout.equipped[w][s];
                    let none_base = if equipped.is_none() { BTN_ON } else { BTN };
                    button(row, "None", Action::Attach(w, s, None), None, none_base, Color::WHITE)
                        .insert(Preview { weapon: w, slot: s, opt: None });
                    for (o, att) in loadout::options(w, s).iter().enumerate() {
                        let o = o as u8;
                        let on = equipped == Some(o);
                        let have = arsenal.loadout.owns(w, s, o);
                        let label = if on {
                            format!("{}  (on)", att.name)
                        } else if have {
                            att.name.to_string()
                        } else {
                            format!("{}  {}", att.name, att.cost)
                        };
                        let color = if have || candy >= att.cost { Color::WHITE } else { TEXT_POOR };
                        button(row, &label, Action::Attach(w, s, Some(o)), None, if on { BTN_ON } else { BTN }, color)
                            .insert(Preview { weapon: w, slot: s, opt: Some(o) });
                    }
                });
        }
    });
}

/// Show/hide the shop and prompt, refresh the header, hover colors, and the stat preview.
pub fn update_shop(
    shop: Res<ShopScreen>,
    mut root: Single<&mut Node, With<ShopRoot>>,
    mut prompt: Single<&mut Text, (With<ShopPrompt>, Without<ShopHeader>, Without<ShopStats>, Without<ShopInfo>)>,
    mut header: Single<&mut Text, (With<ShopHeader>, Without<ShopStats>, Without<ShopInfo>)>,
    mut stats: Query<&mut Text, (With<ShopStats>, Without<ShopHeader>, Without<ShopInfo>)>,
    mut info: Query<&mut Text, (With<ShopInfo>, Without<ShopHeader>, Without<ShopStats>)>,
    mut buttons: Query<(&ShopButton, &Hovered, &mut BackgroundColor)>,
    previews: Query<(&Preview, &Hovered)>,
    message: Res<ShopMessage>,
    arsenal: Res<Arsenal>,
    waves: Res<Waves>,
    score: Res<Score>,
    state: Res<State<GameState>>,
    time: Res<Time<Virtual>>,
) {
    let playing = *state.get() == GameState::Playing;
    root.display = if shop.open { Display::Flex } else { Display::None };

    let next_boss = if enemies::is_boss_wave(waves.wave + 1) {
        format!("Next up: {}", enemies::boss_name(crate::boss::boss_for_wave(waves.wave + 1)))
    } else {
        String::new()
    };
    prompt.0 = if playing && !shop.open && !time.is_paused() && shop_open(&waves) {
        format!(
            "Press B to open the shop   |   Candy: {}   |   Next wave in {:.0}s (Enter to start now)\n{next_boss}",
            score.candy,
            waves.intermission.ceil()
        )
    } else {
        String::new()
    };
    if !shop.open {
        return;
    }

    let msg = if message.0.is_empty() { String::new() } else { format!("   |   {}", message.0) };
    header.0 = format!(
        "Candy: {}   |   Wave {} cleared   |   Break paused while shopping   {}{msg}",
        score.candy, waves.wave, next_boss
    );

    for (btn, hovered, mut bg) in &mut buttons {
        bg.0 = if hovered.get() { BTN_HOVER } else { btn.base };
    }

    // Stats panel: the current weapon tab's numbers, previewing whatever's hovered.
    let hovered = previews.iter().find(|(_, h)| h.get()).map(|(p, _)| p);
    let Some(weapon) = shop.tab.checked_sub(1) else { return };
    let now = arsenal.stats(weapon);
    let preview = hovered.map(|p| arsenal.stats_preview(p.weapon, Some((p.slot, p.opt))));
    for mut t in &mut stats {
        t.0 = loadout::describe(&now, preview.as_ref());
    }
    for mut t in &mut info {
        t.0 = match hovered {
            Some(Preview { opt: Some(o), slot, weapon }) => {
                let att = &loadout::options(*weapon, *slot)[*o as usize];
                let owned = arsenal.loadout.owns(*weapon, *slot, *o);
                let price = if owned { "Owned - free to swap".to_string() } else { format!("{} candy", att.cost) };
                format!("{}: {}\n{}", att.name, att.desc, price)
            }
            Some(Preview { opt: None, slot, .. }) => format!("Take off the {}.", SLOTS[*slot].name().to_lowercase()),
            None => String::new(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_climb_and_cap() {
        let mut u = Upgrades::default();
        assert_eq!(u.cost(1), 200);
        u.levels[1] = 2;
        assert_eq!(u.cost(1), 600);
        u.levels[1] = 5;
        assert!(u.maxed(1));
        // The refill never maxes out and stays the same price.
        u.levels[5] = 50;
        assert!(!u.maxed(5));
        assert_eq!(u.cost(5), 100);
    }

    #[test]
    fn upgrades_change_the_numbers() {
        let mut p = Player::new();
        let mut a = Arsenal::default();
        apply(0, 1, &mut p, &mut a);
        assert_eq!((p.health, p.max_health), (125, 125));
        apply(3, 2, &mut p, &mut a);
        assert_eq!(a.mag_size(1), 9); // scattergun 6 * 1.5
        apply(4, 2, &mut p, &mut a);
        p.take_damage(10, Vec3::ZERO);
        assert_eq!(p.health, 117); // 20% armor
    }

    #[test]
    fn attachments_are_bought_once_then_swap_free() {
        let mut a = Arsenal::default();
        let mut candy = 300;
        // Pistol red dot: 120.
        assert_eq!(attach(&mut a, &mut candy, 0, 0, Some(0)), Ok(true));
        assert_eq!(candy, 180);
        // Take it off, put it back: no charge.
        assert_eq!(attach(&mut a, &mut candy, 0, 0, None), Ok(true));
        assert_eq!(attach(&mut a, &mut candy, 0, 0, Some(0)), Ok(true));
        assert_eq!(candy, 180);
        // Scope: 220, can't afford.
        assert_eq!(attach(&mut a, &mut candy, 0, 0, Some(1)), Err(Refusal::TooPoor));
        assert_eq!(a.loadout.equipped[0][0], Some(0), "failed purchase leaves the red dot on");
    }

    #[test]
    fn weapons_unlock_once() {
        let mut a = Arsenal::default();
        let mut candy = 250;
        assert_eq!(buy_weapon(&mut a, &mut candy, 2), Err(Refusal::TooPoor));
        assert_eq!(buy_weapon(&mut a, &mut candy, 1), Ok(true));
        assert_eq!(candy, 50);
        assert_eq!(buy_weapon(&mut a, &mut candy, 1), Ok(false));
        assert!(a.loadout.owned_weapon[1]);
    }

    #[test]
    fn grenades_cap_at_five() {
        let mut u = Upgrades::default();
        let mut p = Player::new();
        let mut a = Arsenal::default();
        let mut candy = 10_000;
        while buy_upgrade(&mut u, &mut candy, GRENADE_ITEM, &mut p, &mut a).is_ok() {}
        assert_eq!(a.grenades, crate::weapons::MAX_GRENADES);
        assert_eq!(buy_upgrade(&mut u, &mut candy, GRENADE_ITEM, &mut p, &mut a), Err(Refusal::Maxed));
    }

    #[test]
    fn refill_skips_locked_weapons() {
        let mut p = Player::new();
        let mut a = Arsenal::default();
        a.reserve = [0; 4];
        apply(5, 1, &mut p, &mut a);
        assert_eq!(a.reserve[1], 0, "scattergun is still locked");
    }
}

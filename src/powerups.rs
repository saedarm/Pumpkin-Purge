//! Temporary power-ups. They drop rarely from regular kills (6%) and always from bosses.
//!
//! - **Double Damage** (red orb): every weapon and the pitchfork hit twice as hard.
//! - **Sugar Rush** (blue orb): move 50% faster.
//! - **Candy Magnet** (gold orb): pickups within 18m fly to you.
//!
//! Each lasts 12 seconds; grabbing the same one again resets its timer.

use bevy::prelude::*;

use crate::GameState;
use crate::player::Player;
use crate::weapons::Pickup;

pub const DURATION: f32 = 12.0;
const MAGNET_RANGE: f32 = 18.0;
const MAGNET_SPEED: f32 = 14.0;

#[derive(Resource, Default)]
pub struct PowerUps {
    pub damage: f32,
    pub speed: f32,
    pub magnet: f32,
}

impl PowerUps {
    pub fn damage_mult(&self) -> f32 {
        if self.damage > 0.0 { 2.0 } else { 1.0 }
    }

    pub fn speed_mult(&self) -> f32 {
        if self.speed > 0.0 { 1.5 } else { 1.0 }
    }
}

pub fn tick(mut power: ResMut<PowerUps>, time: Res<Time>) {
    let dt = time.delta_secs();
    power.damage = (power.damage - dt).max(0.0);
    power.speed = (power.speed - dt).max(0.0);
    power.magnet = (power.magnet - dt).max(0.0);
}

/// While the magnet is active, nearby pickups slide toward you.
pub fn magnet(
    mut pickups: Query<&mut Transform, (With<Pickup>, Without<Player>)>,
    player: Single<&Transform, With<Player>>,
    power: Res<PowerUps>,
    time: Res<Time>,
) {
    if power.magnet <= 0.0 {
        return;
    }
    let target = player.translation.xz();
    for mut tf in &mut pickups {
        let to = target - tf.translation.xz();
        let d = to.length();
        if d < MAGNET_RANGE && d > 0.1 {
            let step = to / d * (MAGNET_SPEED * time.delta_secs()).min(d);
            tf.translation.x += step.x;
            tf.translation.z += step.y;
        }
    }
}

// ---------------------------------------------------------------- HUD

#[derive(Component)]
pub struct PowerUpText;

pub fn setup_ui(mut commands: Commands) {
    commands.spawn((
        PowerUpText,
        Text::new(""),
        TextFont { font_size: FontSize::Px(22.0), ..default() },
        TextColor(Color::srgb(1.0, 0.9, 0.4)),
        TextLayout::justify(Justify::Right),
        Node { position_type: PositionType::Absolute, top: px(16), right: px(18), ..default() },
    ));
}

pub fn update_ui(mut text: Single<&mut Text, With<PowerUpText>>, power: Res<PowerUps>, state: Res<State<GameState>>) {
    if *state.get() != GameState::Playing {
        text.0.clear();
        return;
    }
    let mut lines = Vec::new();
    for (name, t) in [("DOUBLE DAMAGE", power.damage), ("SUGAR RUSH", power.speed), ("CANDY MAGNET", power.magnet)] {
        if t > 0.0 {
            lines.push(format!("{name}  {:.0}s", t.ceil()));
        }
    }
    text.0 = lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_only_apply_while_active() {
        let mut p = PowerUps::default();
        assert_eq!((p.damage_mult(), p.speed_mult()), (1.0, 1.0));
        p.damage = 3.0;
        p.speed = 0.5;
        assert_eq!((p.damage_mult(), p.speed_mult()), (2.0, 1.5));
    }
}

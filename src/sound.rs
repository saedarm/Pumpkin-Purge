//! Sound effects and music, synthesized in code at startup and turned into
//! in-memory WAV files. No audio assets to download or license.
//!
//! To use real recordings later, replace a field in `Sfx` with
//! `asset_server.load("sounds/shotgun.ogg")` and the rest of the game won't change.

use std::f32::consts::TAU;

use bevy::audio::Volume;
use bevy::prelude::*;
use rand::Rng;

const RATE: u32 = 22_050;

/// Marks the looping background music, so its volume can follow the settings.
#[derive(Component)]
pub struct Music;

#[derive(Resource)]
pub struct Sfx {
    pub shots: [Handle<AudioSource>; 4],
    pub empty: Handle<AudioSource>,
    pub reload_start: Handle<AudioSource>,
    pub reload_end: Handle<AudioSource>,
    pub hit: Handle<AudioSource>,
    pub headshot: Handle<AudioSource>,
    pub splat: Handle<AudioSource>,
    pub explosion: Handle<AudioSource>,
    pub hurt: Handle<AudioSource>,
    pub pickup: Handle<AudioSource>,
    pub throw: Handle<AudioSource>,
    pub groan: Handle<AudioSource>,
    pub bell: Handle<AudioSource>,
}

/// Plays a sound once with a little random pitch variation so repeats don't sound robotic.
pub fn play(commands: &mut Commands, sound: &Handle<AudioSource>, volume: f32) {
    let speed = rand::rng().random_range(0.93..1.07);
    play_with_speed(commands, sound, volume, speed);
}

/// Plays a sound at a set speed. Below 1.0 is slower and deeper.
pub fn play_with_speed(commands: &mut Commands, sound: &Handle<AudioSource>, volume: f32, speed: f32) {
    commands.spawn((
        AudioPlayer::new(sound.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(speed),
    ));
}

/// Plays a sound quieter the farther it is from the player.
pub fn play_at(commands: &mut Commands, sound: &Handle<AudioSource>, at: Vec3, listener: Vec3, volume: f32) {
    let falloff = (1.0 - at.distance(listener) / 35.0).clamp(0.08, 1.0);
    play(commands, sound, volume * falloff);
}

pub fn setup_sound(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| sources.add(AudioSource { bytes: to_wav(&samples).into() });

    let music = add(music());
    commands.insert_resource(Sfx {
        shots: [add(pistol()), add(shotgun()), add(smg()), add(launcher())],
        empty: add(clicks(&[(0.0, 0.5)], 0.06)),
        reload_start: add(clicks(&[(0.0, 0.7), (0.08, 0.5)], 0.15)),
        reload_end: add(clicks(&[(0.0, 0.6), (0.11, 1.0)], 0.2)),
        hit: add(hit()),
        headshot: add(headshot()),
        splat: add(splat()),
        explosion: add(explosion()),
        hurt: add(hurt()),
        pickup: add(pickup()),
        throw: add(whoosh()),
        groan: add(groan()),
        bell: add(bell()),
    });
    commands.spawn((
        Music,
        AudioPlayer::new(music),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.45)),
    ));
}

// ---------------------------------------------------------------- synthesis helpers

fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    let mut out: Vec<f32> = (0..n).map(|i| f(i as f32 / RATE as f32)).collect();
    // Normalize so every effect peaks at the same level.
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    for s in &mut out {
        *s *= 0.9 / peak;
    }
    out
}

/// Cheap deterministic white noise.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// One-pole low-pass filter. Smaller `a` = darker sound.
struct LowPass(f32);
impl LowPass {
    fn run(&mut self, x: f32, a: f32) -> f32 {
        self.0 += a * (x - self.0);
        self.0
    }
}

/// Oscillator that accumulates phase, so frequency sweeps stay smooth.
struct Osc(f32);
impl Osc {
    fn sine(&mut self, freq: f32) -> f32 {
        self.0 = (self.0 + freq / RATE as f32) % 1.0;
        (self.0 * TAU).sin()
    }
}

fn decay(t: f32, rate: f32) -> f32 {
    (-t * rate).exp()
}

// ---------------------------------------------------------------- weapons

fn pistol() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(1), LowPass(0.0), Osc(0.0));
    render(0.2, |t| {
        let crack = lp.run(n.next(), 0.55) * decay(t, 40.0);
        let body = o.sine(170.0 - 300.0 * t) * decay(t, 22.0);
        crack * 0.8 + body * 0.6
    })
}

fn shotgun() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(2), LowPass(0.0), Osc(0.0));
    render(0.55, |t| {
        let snap = if t < 0.015 { n.next() * 0.6 } else { 0.0 };
        let blast = lp.run(n.next(), 0.22) * decay(t, 8.0);
        let boom = o.sine(75.0 - 40.0 * t) * decay(t, 10.0);
        snap + blast * 1.2 + boom * 0.9
    })
}

fn smg() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(3), LowPass(0.0), Osc(0.0));
    render(0.09, |t| {
        let crack = lp.run(n.next(), 0.6) * decay(t, 60.0);
        crack + o.sine(260.0) * decay(t, 50.0) * 0.5
    })
}

fn launcher() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(4), LowPass(0.0), Osc(0.0));
    render(0.4, |t| {
        let thoomp = o.sine(220.0 * (-t * 7.0).exp() + 50.0) * decay(t, 7.0);
        let air = lp.run(n.next(), 0.15) * decay(t, 18.0);
        thoomp + air * 0.5
    })
}

/// Short mechanical clicks at the given (time, loudness) pairs.
fn clicks(at: &[(f32, f32)], len: f32) -> Vec<f32> {
    let mut n = Noise(5);
    let at = at.to_vec();
    render(len, move |t| {
        at.iter()
            .map(|&(start, amp)| {
                let dt = t - start;
                if (0.0..0.012).contains(&dt) {
                    (n.next() * 0.6 + (dt * TAU * 2800.0).sin() * 0.4) * decay(dt, 300.0) * amp
                } else {
                    0.0
                }
            })
            .sum()
    })
}

// ---------------------------------------------------------------- hits and impacts

fn hit() -> Vec<f32> {
    let mut n = Noise(6);
    render(0.06, |t| ((t * TAU * 1400.0).sin() + n.next() * 0.3) * decay(t, 70.0))
}

fn headshot() -> Vec<f32> {
    render(0.3, |t| {
        ((t * TAU * 2100.0).sin() + (t * TAU * 3150.0).sin() * 0.4) * decay(t, 13.0)
    })
}

fn splat() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(7), LowPass(0.0), Osc(0.0));
    render(0.4, |t| {
        let wobble = 0.5 + 0.5 * (t * TAU * 28.0).sin();
        let goo = lp.run(n.next(), 0.18) * wobble;
        let thud = o.sine(320.0 * (-t * 9.0).exp() + 60.0);
        (goo * 1.2 + thud * 0.7) * decay(t, 9.0)
    })
}

fn explosion() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(8), LowPass(0.0), Osc(0.0));
    render(1.5, |t| {
        let attack = (t / 0.006).min(1.0);
        let a = 0.3 * (-t * 2.5).exp() + 0.04;
        let roar = lp.run(n.next(), a);
        let rumble = o.sine(42.0) * decay(t, 3.5);
        (roar * 1.4 + rumble * 0.8) * decay(t, 3.0) * attack
    })
}

fn hurt() -> Vec<f32> {
    let (mut n, mut lp, mut o) = (Noise(9), LowPass(0.0), Osc(0.0));
    render(0.3, |t| {
        let buzz = o.sine(95.0 - 30.0 * t).signum() * 0.5;
        lp.run(buzz + n.next() * 0.4, 0.12) * decay(t, 9.0)
    })
}

fn pickup() -> Vec<f32> {
    render(0.35, |t| {
        let (f, dt) = if t < 0.1 { (784.0, t) } else { (1175.0, t - 0.1) };
        ((dt * TAU * f).sin() + (dt * TAU * f * 2.0).sin() * 0.25) * decay(dt, 12.0)
    })
}

// ---------------------------------------------------------------- enemies

fn whoosh() -> Vec<f32> {
    let (mut n, mut lp) = (Noise(10), LowPass(0.0));
    render(0.45, |t| {
        let p = t / 0.45;
        let shape = (p * std::f32::consts::PI).sin();
        lp.run(n.next(), 0.03 + 0.25 * shape) * shape
    })
}

/// A low, wobbly moan for nearby scarecrows.
fn groan() -> Vec<f32> {
    let (mut n, mut lp) = (Noise(11), LowPass(0.0));
    let mut osc: Vec<Osc> = (0..6).map(|_| Osc(0.0)).collect();
    render(1.3, |t| {
        let p = t / 1.3;
        let env = (p / 0.15).min(1.0) * ((1.0 - p) / 0.35).min(1.0);
        let f0 = 82.0 - 18.0 * p + (t * TAU * 4.5).sin() * 3.0;
        let voice: f32 = osc.iter_mut().enumerate().map(|(k, o)| o.sine(f0 * (k + 1) as f32) / (k + 1) as f32).sum();
        lp.run(voice + n.next() * 0.15, 0.09) * env
    })
}

/// Low church-bell toll that marks the start of each wave.
fn bell() -> Vec<f32> {
    let partials = [(1.0, 0.6, 1.2), (2.0, 0.35, 1.8), (2.4, 0.3, 2.6), (3.0, 0.2, 3.5), (4.2, 0.12, 5.0)];
    render(3.0, |t| {
        partials.iter().map(|&(m, amp, d)| (t * TAU * 147.0 * m).sin() * amp * decay(t, d)).sum::<f32>()
            * (t / 0.004).min(1.0)
    })
}

// ---------------------------------------------------------------- music

/// A 16-second seamless loop: a low drone under a slightly-off music-box tune in A minor.
fn music() -> Vec<f32> {
    const LEN: f32 = 16.0;
    // A harmonic minor; G# is what makes it sound creepy.
    let a4 = 440.0;
    let semis = |s: f32| a4 * 2f32.powf(s / 12.0);
    let melody: [f32; 32] = [
        0.0, 3.0, 7.0, 3.0, 0.0, 3.0, 8.0, 7.0, //
        5.0, 8.0, 12.0, 8.0, 5.0, 3.0, 2.0, -1.0, //
        0.0, 3.0, 7.0, 12.0, 11.0, 7.0, 8.0, 7.0, //
        5.0, 3.0, 2.0, 3.0, -1.0, 2.0, -1.0, -5.0,
    ];
    let step = LEN / melody.len() as f32;
    let mut n = Noise(12);
    let mut lp = LowPass(0.0);

    let samples = (0..(LEN * RATE as f32) as usize)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            // Drone: whole numbers of cycles per loop so it wraps with no click.
            let tremolo = 0.75 + 0.25 * (t * TAU * 0.25).sin();
            let drone = ((t * TAU * 55.0).sin() + (t * TAU * 110.25).sin() * 0.45) * tremolo * 0.22;
            let wind = lp.run(n.next(), 0.01) * 0.6;

            // Music box: each note rings out until the next one.
            let idx = (t / step) as usize;
            let dt = t - idx as f32 * step;
            let f = semis(melody[idx % melody.len()]);
            // Slight detune on the second partial makes it sound old and warped.
            let note = ((dt * TAU * f).sin() + (dt * TAU * f * 2.01).sin() * 0.3) * decay(dt, 6.0) * 0.16;
            drone + wind + note
        })
        .collect::<Vec<_>>();
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    samples.into_iter().map(|s| s * 0.8 / peak).collect()
}

// ---------------------------------------------------------------- WAV encoding

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let wav = to_wav(&pistol());
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(wav.len(), 44 + data_len);
    }

    /// `cargo test export_wavs -- --ignored` writes every sound to target/sounds/ to listen to.
    #[test]
    #[ignore]
    fn export_wavs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sounds");
        std::fs::create_dir_all(&dir).unwrap();
        let all: [(&str, Vec<f32>); 14] = [
            ("pistol", pistol()), ("shotgun", shotgun()), ("smg", smg()), ("launcher", launcher()),
            ("reload", clicks(&[(0.0, 0.7), (0.08, 0.5)], 0.15)), ("hit", hit()), ("headshot", headshot()),
            ("splat", splat()), ("explosion", explosion()), ("hurt", hurt()), ("pickup", pickup()),
            ("groan", groan()), ("bell", bell()), ("music", music()),
        ];
        for (name, samples) in all {
            std::fs::write(dir.join(format!("{name}.wav")), to_wav(&samples)).unwrap();
        }
    }

    #[test]
    fn music_loop_is_seamless_enough() {
        let m = music();
        // The loop point shouldn't jump by more than a small amount (no audible pop).
        assert!((m[0] - m[m.len() - 1]).abs() < 0.15);
        assert!(m.iter().all(|s| s.is_finite()));
    }
}

//! Saving to disk: settings and best scores, as small `key=value` text files.
//!
//! Files live in:
//! - Windows: `%APPDATA%\PumpkinPurge\`
//! - Linux/macOS: `$XDG_CONFIG_HOME/pumpkin-purge/` or `~/.config/pumpkin-purge/`
//!
//! Set `PUMPKIN_SAVE_DIR` to put them somewhere else. If a file is missing or
//! unreadable the game just uses defaults; save errors are logged, never fatal.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;

use crate::enemies::Waves;
use crate::Score;

pub fn dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = env("PUMPKIN_SAVE_DIR") {
        d
    } else if let Some(d) = env("APPDATA") {
        d.join("PumpkinPurge")
    } else if let Some(d) = env("XDG_CONFIG_HOME") {
        d.join("pumpkin-purge")
    } else if let Some(d) = env("HOME") {
        d.join(".config").join("pumpkin-purge")
    } else {
        PathBuf::from(".")
    }
}

pub fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn load(name: &str) -> HashMap<String, String> {
    std::fs::read_to_string(dir().join(name)).map(|t| parse(&t)).unwrap_or_default()
}

pub fn store(name: &str, pairs: &[(&str, String)]) {
    let text: String = pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    let d = dir();
    if let Err(e) = std::fs::create_dir_all(&d).and_then(|_| std::fs::write(d.join(name), text)) {
        warn!("couldn't save {name} to {}: {e}", d.display());
    }
}

/// Read a value, falling back to `default` if it's missing or doesn't parse.
pub fn get<T: std::str::FromStr>(map: &HashMap<String, String>, key: &str, default: T) -> T {
    map.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ---------------------------------------------------------------- best scores

const BEST_FILE: &str = "best.txt";
/// Key prefix per difficulty in best.txt. Normal has none, so older save files still load.
const PREFIX: [&str; 3] = ["easy.", "", "hard."];

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct Record {
    pub wave: u32,
    pub score: u32,
    pub kills: u32,
}

/// Best runs, kept separately for Easy, Normal, and Hard.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Best {
    pub per: [Record; 3],
    /// Set when the run that just ended beat the saved best score. Not saved.
    pub new_record: bool,
}

impl Best {
    pub fn load() -> Self {
        let m = load(BEST_FILE);
        let rec = |p: &str| Record {
            wave: get(&m, &format!("{p}wave"), 0),
            score: get(&m, &format!("{p}score"), 0),
            kills: get(&m, &format!("{p}kills"), 0),
        };
        Self { per: PREFIX.map(rec), new_record: false }
    }

    pub fn get(&self, difficulty: u32) -> Record {
        self.per[difficulty.min(2) as usize]
    }

    /// Fold a finished run into the bests for that difficulty. Each stat keeps its own
    /// maximum; "new record" means the score went up.
    pub fn record(&mut self, difficulty: u32, wave: u32, score: u32, kills: u32) {
        let r = &mut self.per[difficulty.min(2) as usize];
        self.new_record = score > r.score;
        r.wave = r.wave.max(wave);
        r.score = r.score.max(score);
        r.kills = r.kills.max(kills);
    }

    fn save(&self) {
        let mut pairs = Vec::new();
        for (i, p) in PREFIX.iter().enumerate() {
            let r = self.per[i];
            pairs.push((format!("{p}wave"), r.wave.to_string()));
            pairs.push((format!("{p}score"), r.score.to_string()));
            pairs.push((format!("{p}kills"), r.kills.to_string()));
        }
        let refs: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        store(BEST_FILE, &refs);
    }
}

/// True when the run started from practice mode (`PUMPKIN_WAVE` or `PUMPKIN_GOD`).
/// Practice runs don't count toward the saved bests. Cleared on restart.
#[derive(Resource, Default)]
pub struct Practice(pub bool);

/// `PUMPKIN_GOD=1`: you take no damage (for testing mechanics). Lasts the whole session.
#[derive(Resource, Default)]
pub struct GodMode(pub bool);

pub fn record_best(
    mut best: ResMut<Best>,
    waves: Res<Waves>,
    score: Res<Score>,
    practice: Res<Practice>,
    settings: Res<crate::settings::Settings>,
) {
    if practice.0 {
        best.new_record = false;
        return;
    }
    best.record(settings.difficulty, waves.wave, score.points, score.kills);
    best.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignores_junk_and_whitespace() {
        let m = parse("score = 120\nnot a pair\n\nwave=7\n");
        assert_eq!(get(&m, "score", 0u32), 120);
        assert_eq!(get(&m, "wave", 0u32), 7);
        assert_eq!(get(&m, "kills", 5u32), 5);
        assert_eq!(get(&parse("score=lots"), "score", 9u32), 9);
    }

    #[test]
    fn best_keeps_each_maximum() {
        let mut b = Best::default();
        b.per[1] = Record { wave: 8, score: 900, kills: 60 };
        b.record(1, 5, 1200, 40);
        assert_eq!((b.get(1), b.new_record), (Record { wave: 8, score: 1200, kills: 60 }, true));
        b.record(1, 11, 300, 20);
        assert_eq!((b.get(1), b.new_record), (Record { wave: 11, score: 1200, kills: 60 }, false));
    }

    #[test]
    fn each_difficulty_has_its_own_best() {
        let mut b = Best::default();
        b.record(2, 4, 500, 30);
        assert!(b.new_record);
        assert_eq!(b.get(1), Record::default(), "normal untouched");
        assert_eq!(b.get(2).score, 500);
        b.record(0, 9, 100, 80);
        assert!(b.new_record, "first easy run is a record on easy");
    }
}

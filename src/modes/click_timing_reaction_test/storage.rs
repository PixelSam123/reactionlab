use std::fs;
use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime};

use super::state::compute_mean;
use super::types::{Configurables, RoundOutcome, RunData, RunFileInfo};

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reactionlab")
        .join("click_timing_reaction_test")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn load_config() -> Configurables {
    let path = config_path();
    if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        Configurables::default()
    }
}

pub fn save_config(config: &Configurables) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(config) {
        fs::write(path, json).ok();
    }
}

pub fn save_run(run_data: &RunData) {
    let runs_dir = data_dir();
    fs::create_dir_all(&runs_dir).ok();
    let filename = format!("reactionlab-{}.json", run_data.timestamp);
    if let Ok(json) = serde_json::to_string_pretty(run_data) {
        fs::write(runs_dir.join(filename), json).ok();
    }
}

pub fn list_run_files() -> Vec<RunFileInfo> {
    let dir = data_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && let Some(filename) = path.file_name()
            {
                let filename = filename.to_string_lossy().to_string();
                if filename.starts_with("reactionlab-") {
                    files.push(RunFileInfo {
                        display_name: filename_to_display(&filename),
                        filename,
                    });
                }
            }
        }
    }
    files.sort_by(|a, b| b.filename.cmp(&a.filename));
    files
}

pub fn filename_to_display(filename: &str) -> String {
    let timestamp = filename
        .strip_prefix("reactionlab-")
        .and_then(|value| value.strip_suffix(".json"))
        .unwrap_or("");
    parse_timestamp(timestamp).map_or_else(
        || filename.to_string(),
        |date| {
            date.format("%e %B %Y, %H:%M:%S").to_string()
                + &format!(".{:03}", date.and_utc().timestamp_subsec_millis())
        },
    )
}

pub fn load_run_data(filename: &str) -> Option<RunData> {
    serde_json::from_str(&fs::read_to_string(data_dir().join(filename)).ok()?).ok()
}

pub fn delete_runs_before(date: NaiveDate) {
    let dir = data_dir();
    if !dir.exists() {
        return;
    }
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && let Some(stem) = path.file_stem()
            {
                let timestamp = stem.to_string_lossy();
                if let Some(timestamp) = timestamp.strip_prefix("reactionlab-")
                    && timestamp.len() >= 10
                    && let Ok(file_date) = NaiveDate::parse_from_str(&timestamp[..10], "%Y-%m-%d")
                    && file_date < date
                {
                    fs::remove_file(path).ok();
                }
            }
        }
    }
}

pub fn load_history_summary() -> Vec<(NaiveDateTime, f64)> {
    let dir = data_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && path
                    .file_stem()
                    .is_some_and(|stem| stem.to_string_lossy().starts_with("reactionlab-"))
                && let Ok(content) = fs::read_to_string(path)
                && let Ok(run_data) = serde_json::from_str::<RunData>(&content)
            {
                let errors: Vec<f64> = run_data
                    .attempts
                    .iter()
                    .filter(|round| round.outcome == RoundOutcome::Hit)
                    .filter_map(|round| round.click_offset_ms.map(f64::abs))
                    .collect();
                if let Some(timestamp) = parse_timestamp(&run_data.timestamp)
                    && !errors.is_empty()
                {
                    entries.push((timestamp, compute_mean(&errors)));
                }
            }
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    entries.truncate(10);
    entries.reverse();
    entries
}

fn parse_timestamp(timestamp: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S%.3f")
        .or_else(|_| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S"))
        .ok()
}

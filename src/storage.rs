use std::fs;
use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime};

use crate::state::compute_mean;
use crate::types::{Configurables, RunData, RunFileInfo};

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reactionlab")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn load_config() -> Configurables {
    let path = config_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
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
    let mut files: Vec<RunFileInfo> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Some(fname) = path.file_name()
            {
                let filename = fname.to_string_lossy().to_string();
                if filename.starts_with("reactionlab-") {
                    let display_name = filename_to_display(&filename);
                    files.push(RunFileInfo {
                        filename,
                        display_name,
                    });
                }
            }
        }
    }
    files.sort_by(|a, b| b.filename.cmp(&a.filename));
    files
}

pub fn filename_to_display(filename: &str) -> String {
    let ts_str = filename
        .strip_prefix("reactionlab-")
        .and_then(|s| s.strip_suffix(".json"))
        .unwrap_or("");
    NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d_%H-%M-%S")
        .map(|dt| {
            dt.format("%e %B %Y, %H:%M:%S")
                .to_string()
                .trim_start()
                .to_string()
        })
        .unwrap_or_else(|_| filename.to_string())
}

pub fn load_run_data(filename: &str) -> Option<RunData> {
    let path = data_dir().join(filename);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn delete_runs_before(date: NaiveDate) {
    let dir = data_dir();
    if !dir.exists() {
        return;
    }
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Some(stem) = path.file_stem()
            {
                let name = stem.to_string_lossy().to_string();
                if let Some(ts_str) = name.strip_prefix("reactionlab-")
                    && let Ok(file_date) = NaiveDate::parse_from_str(&ts_str[..10], "%Y-%m-%d")
                    && file_date < date
                {
                    fs::remove_file(&path).ok();
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
    let mut entries: Vec<(NaiveDateTime, f64)> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && path
                    .file_stem()
                    .is_some_and(|s| s.to_string_lossy().starts_with("reactionlab-"))
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(run_data) = serde_json::from_str::<RunData>(&content)
            {
                let times: Vec<f64> = run_data.rounds.iter().map(|r| r.reaction_time_ms).collect();
                let mean = compute_mean(&times);
                if let Ok(dt) =
                    NaiveDateTime::parse_from_str(&run_data.timestamp, "%Y-%m-%d_%H-%M-%S")
                {
                    entries.push((dt, mean));
                }
            }
        }
    }
    entries.sort_by_key(|b| std::cmp::Reverse(b.0));
    entries.truncate(10);
    entries.reverse();
    entries
}

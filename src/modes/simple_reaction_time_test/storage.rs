use std::fs;
use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime};

use super::stats_math::compute_mean;
use super::types::{Configurables, RunData, RunFileInfo};

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reactionlab")
        .join("simple_reaction_time_test")
}

fn config_path() -> PathBuf {
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

pub fn save_config(config: &Configurables) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("directory creation: {error}"))?;
    }
    let json =
        serde_json::to_string_pretty(config).map_err(|error| format!("serialization: {error}"))?;
    fs::write(path, json).map_err(|error| format!("file write: {error}"))?;
    Ok(())
}

pub fn save_run(run_data: &RunData) -> Result<(), String> {
    let runs_dir = data_dir();
    fs::create_dir_all(&runs_dir).map_err(|error| format!("directory creation: {error}"))?;
    let filename = format!("reactionlab-{}.json", run_data.timestamp);
    let json = serde_json::to_string_pretty(run_data)
        .map_err(|error| format!("serialization: {error}"))?;
    fs::write(runs_dir.join(filename), json).map_err(|error| format!("file write: {error}"))?;
    Ok(())
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
            if path.extension().is_some_and(|ext| ext == "json")
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

fn filename_to_display(filename: &str) -> String {
    let timestamp_str = filename
        .strip_prefix("reactionlab-")
        .and_then(|s| s.strip_suffix(".json"))
        .unwrap_or("");
    parse_timestamp(timestamp_str).map_or_else(
        || filename.to_string(),
        |dt| {
            format!(
                "{}.{:03}",
                dt.format("%e %B %Y, %H:%M:%S"),
                dt.and_utc().timestamp_subsec_millis()
            )
        },
    )
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
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(stem) = path.file_stem()
            {
                let name = stem.to_string_lossy().to_string();
                if is_filename_stem_before(&name, date) {
                    fs::remove_file(&path).ok();
                }
            }
        }
    }
}

fn is_filename_stem_before(filename_stem: &str, date: NaiveDate) -> bool {
    const DATE_FMT_LEN: usize = 10;
    filename_stem
        .strip_prefix("reactionlab-")
        .and_then(|timestamp| timestamp.get(..DATE_FMT_LEN))
        .and_then(|date_str| NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok())
        .is_some_and(|file_date| file_date < date)
}

/// Get last 10 runs with their mean reaction times
#[allow(clippy::cast_precision_loss)]
pub fn load_history_summary() -> Vec<(NaiveDateTime, f64)> {
    let dir = data_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries: Vec<(NaiveDateTime, f64)> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && path
                    .file_stem()
                    .is_some_and(|s| s.to_string_lossy().starts_with("reactionlab-"))
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(run_data) = serde_json::from_str::<RunData>(&content)
            {
                let times: Vec<f64> = run_data
                    .rounds
                    .iter()
                    .map(|r| r.reaction_time_ms as f64)
                    .collect();
                let mean = compute_mean(&times);
                if let Some(date_time) = parse_timestamp(&run_data.timestamp) {
                    entries.push((date_time, mean));
                }
            }
        }
    }
    entries.sort_by_key(|b| std::cmp::Reverse(b.0));
    entries.truncate(10);
    entries.reverse();
    entries
}

fn parse_timestamp(timestamp: &str) -> Option<NaiveDateTime> {
    if !timestamp.contains('.') {
        return None;
    }
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S%.3f").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_millisecond_precision_timestamp() {
        assert_eq!(
            filename_to_display("reactionlab-2026-08-18_12-34-56.123.json"),
            "18 August 2026, 12:34:56.123"
        );
    }

    #[test]
    fn marks_filename_stem_before_date() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 19).unwrap();

        assert!(is_filename_stem_before(
            "reactionlab-2026-08-18_12-34-56.123",
            date
        ));
    }

    #[test]
    fn denies_filename_stem_after_date() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 18).unwrap();

        assert!(!is_filename_stem_before(
            "reactionlab-2026-08-19_12-34-56.123",
            date
        ));
    }
}

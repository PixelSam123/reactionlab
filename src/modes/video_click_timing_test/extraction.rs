#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::io::Read;
use std::process::Command;
use std::thread;

use eframe::egui::ColorImage;
use ffmpeg_sidecar::command::FfmpegCommand;

/// Maximum width (px) a decoded frame is scaled down to, to bound texture memory.
pub const MAX_FRAME_WIDTH: u32 = 1920;

/// Basic video stream metadata obtained via ffprobe.
#[derive(Debug, Clone, Copy)]
pub struct VideoInfo {
    pub fps: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A contiguous list of decoded RGB video frames plus timing metadata.
#[derive(Clone)]
pub struct VideoSegment {
    pub frames: Vec<ColorImage>,
    pub fps: f64,
    pub duration_ms: f64,
}

/// Detect the video frame rate using ffprobe. Returns `None` when ffprobe is
/// unavailable or reports nothing usable.
pub fn probe_fps(path: &str) -> Option<f64> {
    probe_video(path)?.fps
}

/// Probe fps and dimensions with a single ffprobe call.
pub fn probe_video(path: &str) -> Option<VideoInfo> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate,width,height",
            "-of",
            "default=nw=1",
        ])
        .arg(path)
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let mut info = VideoInfo {
        fps: None,
        width: None,
        height: None,
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "r_frame_rate" => info.fps = parse_frame_rate(value),
            "width" => info.width = value.parse().ok(),
            "height" => info.height = value.parse().ok(),
            _ => {}
        }
    }
    Some(info)
}

fn parse_frame_rate(value: &str) -> Option<f64> {
    if let Some((num, den)) = value.split_once('/') {
        let num: f64 = num.trim().parse().ok()?;
        let den: f64 = den.trim().parse().ok()?;
        (den > 0.0).then(|| num / den)
    } else {
        value.parse().ok()
    }
}

/// Output dimensions after applying `scale=min({MAX_FRAME_WIDTH},iw)` while
/// preserving aspect ratio.
fn scaled_dimensions(src_width: u32, src_height: u32) -> (u32, u32) {
    if src_width == 0 || src_height == 0 {
        return (src_width.max(1), src_height.max(1));
    }
    let width = src_width.clamp(1, MAX_FRAME_WIDTH);
    let height = (f64::from(src_height) * f64::from(width) / f64::from(src_width))
        .round()
        .max(2.0) as u32;
    (width, height)
}

/// Decode the slice `[start_ms, end_ms]` of `path` into a list of RGB frames.
///
/// Frames are forced to `fps` (falling back to 30 when unknown) and scaled down
/// to at most [`MAX_FRAME_WIDTH`] pixels wide so playback stays light. Raw
/// frames are read directly from ffmpeg's piped stdout in fixed-size chunks,
/// so playback does not depend on any log parsing. Frame `i` corresponds to
/// `start_ms + i / fps * 1000` milliseconds.
pub fn extract_segment(
    path: &str,
    start_ms: f64,
    end_ms: f64,
    fps: f64,
) -> Result<VideoSegment, String> {
    let fps = if fps > 0.0 { fps } else { 30.0 };
    let start_s = (start_ms / 1000.0).max(0.0);
    let end_s = (end_ms / 1000.0).max(start_s);
    // A zero-length range denotes a single starting frame, so never request
    // less than one frame interval from ffmpeg.
    let duration_s = ((end_s - start_s).max(1.0 / fps)).max(0.001);

    let info = probe_video(path).ok_or_else(|| {
        "Failed to probe video dimensions. Is ffprobe installed and the video path valid?"
            .to_string()
    })?;
    let Some(src_width) = info.width else {
        return Err("Could not determine the video width via ffprobe.".to_string());
    };
    let Some(src_height) = info.height else {
        return Err("Could not determine the video height via ffprobe.".to_string());
    };
    let (width, height) = scaled_dimensions(src_width, src_height);
    let frame_bytes = width as usize * height as usize * 3;

    let mut child = FfmpegCommand::new()
        .hide_banner()
        .arg("-ss")
        .arg(format!("{start_s:.3}"))
        .input(path)
        .arg("-t")
        .arg(format!("{duration_s:.3}"))
        .arg("-vf")
        .arg(format!("scale={width}:{height},fps={fps}"))
        // `rawvideo()` already targets stdout (`-f rawvideo -pix_fmt rgb24 -`);
        // adding another explicit output here would make ffmpeg abort.
        .rawvideo()
        .spawn()
        .map_err(|error| format!("Failed to start ffmpeg: {error}"))?;

    // Drain stderr on another thread so a full pipe cannot deadlock ffmpeg,
    // keeping the tail around to explain failures.
    let stderr_tail = child.take_stderr().map(|mut stderr| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            stderr.read_to_end(&mut buffer).ok();
            String::from_utf8_lossy(&buffer).into_owned()
        })
    });

    let Some(mut stdout) = child.take_stdout() else {
        let _ = child.kill();
        return Err("ffmpeg did not expose a piped stdout stream.".to_string());
    };

    let mut frames = Vec::new();
    let mut buffer = vec![0u8; frame_bytes];
    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => frames.push(ColorImage::from_rgb(
                [width as usize, height as usize],
                &buffer,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => {
                let _ = child.kill();
                return Err(format!("Failed to read ffmpeg output: {error}"));
            }
        }
    }

    let _ = child.wait();
    let captured = match stderr_tail {
        Some(handle) => handle.join().unwrap_or_default(),
        None => String::new(),
    };

    if frames.is_empty() {
        return Err(error_with_tail(
            "ffmpeg produced no frames. Is ffmpeg installed and the video path valid?",
            &captured,
        ));
    }

    let duration_ms = frames.len() as f64 / fps * 1000.0;
    Ok(VideoSegment {
        frames,
        fps,
        duration_ms,
    })
}

fn error_with_tail(context: &str, logs: &str) -> String {
    let lines: Vec<&str> = logs.lines().collect();
    let start = lines.len().saturating_sub(12);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        format!("{context} No ffmpeg error output was captured.")
    } else {
        format!("{context}\nffmpeg output:\n{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_down_preserving_aspect_ratio() {
        assert_eq!(scaled_dimensions(2560, 1440), (1920, 1080));
        assert_eq!(scaled_dimensions(3840, 2160), (1920, 1080));
        assert_eq!(scaled_dimensions(800, 600), (800, 600));
        assert_eq!(scaled_dimensions(641, 481), (641, 481));
        assert_eq!(scaled_dimensions(0, 100), (1, 100));
    }

    #[test]
    fn parses_frame_rates() {
        assert_eq!(parse_frame_rate("60/1"), Some(60.0));
        assert_eq!(parse_frame_rate("30000/1001"), Some(29.970_029_970_029_97));
        assert_eq!(parse_frame_rate("29.97"), Some(29.97));
        assert_eq!(parse_frame_rate("0/0"), None);
    }

    /// End-to-end check against a synthetic clip, skipped automatically when
    /// ffmpeg is not installed on the machine running the tests.
    #[test]
    fn extracts_frames_from_generated_video() {
        if Command::new("ffprobe").arg("-version").output().is_err() {
            return;
        }
        let target = std::env::temp_dir().join(format!(
            "reactionlab_extract_test_{}.mp4",
            std::process::id()
        ));
        let target_path = target.to_string_lossy().into_owned();

        let created = FfmpegCommand::new()
            .format("lavfi")
            .input("testsrc=size=320x240:rate=30:duration=1")
            .overwrite()
            .output(&target_path)
            .spawn();
        let mut child = match created {
            Ok(child) => child,
            Err(_) => return,
        };
        match child.iter() {
            Ok(iter) => iter.for_each(|_| {}),
            Err(_) => {
                let _ = std::fs::remove_file(&target);
                return;
            }
        }

        let segment = extract_segment(&target_path, 0.0, 500.0, 30.0)
            .expect("extraction of a generated clip should succeed");
        assert!(
            segment.frames.len() >= 10,
            "expected ~15 frames, got {}",
            segment.frames.len()
        );
        assert_eq!(segment.fps, 30.0);
        let first = &segment.frames[0];
        assert_eq!(first.width(), 320);
        assert_eq!(first.height(), 240);

        // A zero-length range denotes a single starting frame.
        let degenerate = extract_segment(&target_path, 500.0, 500.0, 30.0)
            .expect("a zero-length range should still yield one frame");
        assert!(
            !degenerate.frames.is_empty(),
            "degenerate range must round up to one frame"
        );

        let _ = std::fs::remove_file(&target);
    }
}

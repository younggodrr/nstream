use std::path::{Path, PathBuf};

/// Renders a replacement dialogue track at its original time offsets and muxes it
/// with the source video.  FFmpeg is deliberately invoked as an argument array so
/// user-controlled paths never pass through a shell.
pub async fn render_dubbed_video(
    source: &Path,
    segments: &[(PathBuf, u64)],
    source_audio_mute_ranges: &[(u64, u64)],
    output: &Path,
) -> Result<(), String> {
    if segments.is_empty() {
        return Err("cannot render a dub without synthesized audio".into());
    }
    let parent = output.parent().ok_or("output path has no parent")?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;

    let mut command = tokio::process::Command::new("ffmpeg");
    command
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(source);
    for (audio, _) in segments {
        command.arg("-i").arg(audio);
    }
    let mut filters = Vec::with_capacity(segments.len() + source_audio_mute_ranges.len() + 2);
    let mut labels = Vec::with_capacity(segments.len());
    for (index, (_, start_ms)) in segments.iter().enumerate() {
        let label = format!("a{index}");
        filters.push(format!("[{}:a]adelay={0}|{0}[{label}]", start_ms));
        labels.push(format!("[{label}]"));
    }
    if source_audio_mute_ranges.is_empty() {
        filters.push(format!(
            "{}amix=inputs={}:normalize=0:dropout_transition=0[dub]",
            labels.join(""),
            segments.len()
        ));
    } else {
        let mut original = "0:a".to_string();
        for (index, (start_ms, end_ms)) in source_audio_mute_ranges.iter().enumerate() {
            let next = format!("original{index}");
            filters.push(format!(
                "[{original}]volume=0:enable='between(t,{:.3},{:.3})'[{next}]",
                *start_ms as f64 / 1000.0,
                *end_ms as f64 / 1000.0
            ));
            original = next;
        }
        labels.insert(0, format!("[{original}]"));
        filters.push(format!(
            "{}amix=inputs={}:normalize=0:dropout_transition=0[dub]",
            labels.join(""),
            labels.len()
        ));
    }
    command
        .arg("-filter_complex")
        .arg(filters.join(";"))
        .args([
            "-map", "0:v:0", "-map", "[dub]", "-c:v", "copy", "-c:a", "aac",
        ])
        .args(["-movflags", "+faststart"])
        .arg(output);
    let result = command
        .output()
        .await
        .map_err(|e| format!("ffmpeg unavailable: {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg dub render failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ))
    }
}

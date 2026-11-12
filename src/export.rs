//! Offline exports through the same renderer as playback. No hardware is opened.
use crate::{engine::Renderer, language::Score};
use anyhow::{Result, ensure};
use std::path::Path;
#[derive(Debug)]
pub struct Report {
    pub frames: u64,
    pub peak: f32,
    pub clipped: u64,
}
pub fn region(root: &Path, score: Score, path: &Path, start: f64, end: f64) -> Result<Report> {
    ensure!(
        start.is_finite()
            && end.is_finite()
            && start >= 0.
            && end > start
            && end <= score.duration(),
        "export region requires 0 <= start < end <= arrangement duration"
    );
    let rate = score.rate as f64;
    let limits = crate::phonetics::app_config().storage;
    crate::storage::preflight(
        path,
        ((end - start) * rate).ceil() as u64 * 2 * (score.bits as u64 / 8) + 1048576,
        limits.minimum_free_bytes,
    )?;
    let mut renderer = Renderer::prepare(root, score)?;
    renderer.seek((start * rate).round() as u64)?;
    let first = renderer.position;
    renderer.peak = 0.;
    renderer.clipped = 0;
    renderer.length = (end * rate).round() as u64;
    ensure!(
        renderer.length > first,
        "export region is shorter than one frame"
    );
    renderer.render(path)?;
    Ok(Report {
        frames: renderer.position - first,
        peak: renderer.peak,
        clipped: renderer.clipped,
    })
}
/// Post-fader, post-track-effects isolated stems, without master effects.
/// Solo/mute are ignored so every requested channel is exported independently.
pub fn stems(root: &Path, score: &Score, directory: &Path) -> Result<Vec<Report>> {
    ensure!(!score.tracks.is_empty(), "no tracks to export");
    let bytes = (score.duration() * score.rate as f64).ceil() as u64
        * 2
        * (score.bits as u64 / 8)
        * score.tracks.len() as u64;
    crate::storage::preflight(
        directory,
        bytes.saturating_add(1048576),
        crate::phonetics::app_config().storage.minimum_free_bytes,
    )?;
    std::fs::create_dir(directory)?; // Never merge into or replace an existing destination.
    let mut reports = Vec::new();
    for (i, track) in score.tracks.iter().enumerate() {
        let mut stem = score.clone();
        stem.tracks = vec![track.clone()];
        stem.tracks[0].mute = false;
        stem.tracks[0].solo = false;
        stem.master.clear();
        stem.mixer.clear();
        stem.tracks[0].routing = Default::default();
        let name: String = track
            .name
            .chars()
            .take(64)
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let path = directory.join(format!("{:03}-{name}.flac", i + 1));
        let end = stem.duration();
        reports.push(region(root, stem, &path, 0., end)?);
    }
    Ok(reports)
}

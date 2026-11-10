//! Capture alignment settings and a bounded gate shared with the native input callback.
use crate::audio::Meters;
use serde::Deserialize;
use std::sync::{Arc, atomic::Ordering};
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub latency_ms: f64,
    pub count_in_bars: u32,
    pub pre_roll_bars: u32,
    pub metronome: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            latency_ms: 0.,
            count_in_bars: 1,
            pre_roll_bars: 0,
            metronome: false,
        }
    }
}
#[derive(Clone)]
pub struct Gate {
    pub transport: Arc<Meters>,
    pub start: u64,
    pub end: u64,
    pub looping: bool,
    pub spans: Arc<Spans>,
}
pub struct Spans {
    pub frames: Vec<std::sync::atomic::AtomicU64>,
    pub count: std::sync::atomic::AtomicU64,
}
impl Default for Spans {
    fn default() -> Self {
        Self {
            frames: (0..1024)
                .map(|_| std::sync::atomic::AtomicU64::new(0))
                .collect(),
            count: std::sync::atomic::AtomicU64::new(0),
        }
    }
}
impl Gate {
    pub fn save_spans(&self, path: &std::path::Path, frames: u64, rate: u32) -> anyhow::Result<()> {
        let count = self.spans.count.load(Ordering::Acquire) as usize;
        let mut spans = vec![];
        for i in 0..=count {
            let start = self.spans.frames[i].load(Ordering::Relaxed);
            let end = if i == count {
                frames
            } else {
                self.spans.frames[i + 1].load(Ordering::Relaxed)
            };
            if end > start {
                spans.push((start as f64 / rate as f64, end as f64 / rate as f64));
            }
        }
        crate::project::atomic(
            &path.with_extension("loops.toml"),
            toml::to_string(&LoopSpans { spans })?.as_bytes(),
        )
    }
    pub fn status(&self) -> i8 {
        if self.transport.stopped.load(Ordering::Relaxed) {
            return 1;
        }
        if self.transport.count_in.load(Ordering::Relaxed) > 0
            || self.transport.paused.load(Ordering::Relaxed)
        {
            return -1;
        }
        let frame = self.transport.frames.load(Ordering::Relaxed);
        if frame >= self.end && !self.looping {
            1
        } else if frame < self.start {
            -1
        } else {
            0
        }
    }
}

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{fs, path::Path};
#[derive(Default, Serialize, Deserialize)]
struct LoopSpans {
    spans: Vec<(f64, f64)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Take {
    pub id: u32,
    pub file: String,
    pub at: f64,
    pub latency_ms: f64,
    #[serde(default)]
    pub source: Option<(f64, f64)>,
}
#[derive(Default, Serialize, Deserialize)]
struct Takes {
    takes: Vec<Take>,
}
pub fn takes(root: &Path) -> Result<Vec<Take>> {
    let path = root.join("recordings/takes.toml");
    if !path.exists() {
        return Ok(vec![]);
    }
    ensure!(
        path.metadata()?.len() <= 1048576,
        "take index exceeds 1 MiB"
    );
    let result: Takes = toml::from_str(&fs::read_to_string(path)?)?;
    ensure!(result.takes.len() <= 4096, "take index limit 4096");
    Ok(result.takes)
}
pub fn register(root: &Path, file: &Path, at: f64, latency_ms: f64) -> Result<Take> {
    ensure!(
        at.is_finite() && at >= 0. && latency_ms.is_finite() && latency_ms.abs() <= 2000.,
        "invalid take alignment"
    );
    let file = crate::codec::script_path(
        file.strip_prefix(root)
            .context("take lies outside project")?,
    );
    crate::codec::safe_path(root, &file)?;
    let mut entries = takes(root)?;
    let id = entries
        .iter()
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .context("take numbering exhausted")?;
    ensure!(entries.len() < 4096, "take limit 4096");
    let take = Take {
        id,
        file,
        at: (at - latency_ms / 1000.).max(0.),
        latency_ms,
        source: None,
    };
    let spans_file = root.join(&take.file).with_extension("loops.toml");
    if spans_file.exists() {
        ensure!(
            spans_file.metadata()?.len() <= 1048576,
            "loop take index limit"
        );
        let spans: LoopSpans = toml::from_str(&fs::read_to_string(spans_file)?)?;
        ensure!(
            spans.spans.len() <= 1024 && entries.len() + spans.spans.len() <= 4096,
            "loop take limit exceeded"
        );
        for (i, span) in spans.spans.into_iter().enumerate() {
            let mut lap = take.clone();
            lap.id += i as u32;
            lap.source = Some(span);
            entries.push(lap);
        }
    } else {
        entries.push(take.clone());
    }
    let take = entries
        .iter()
        .find(|t| t.id == id)
        .context("no nonempty loop takes captured")?
        .clone();
    crate::project::atomic(
        &root.join("recordings/takes.toml"),
        toml::to_string_pretty(&Takes { takes: entries })?.as_bytes(),
    )?;
    Ok(take)
}
pub fn clip(take: &Take, name: &str, at: f64, trim: Option<(f64, f64)>) -> Result<String> {
    ensure!(at.is_finite() && at >= 0., "invalid take placement");
    let mut source = format!("\ntrack {name:?} {{ at {at}s; import {:?};", take.file);
    if let Some((a, b)) = trim.or(take.source) {
        ensure!(
            a.is_finite() && b.is_finite() && a >= 0. && b > a,
            "invalid take trim"
        );
        source.push_str(&format!(" trim {a}s {b}s;"));
    }
    source.push_str(" }\n");
    Ok(source)
}

pub fn preference(key: &str, value: &str) -> Result<Settings> {
    let path = crate::locale::settings_path()?;
    let mut document: toml::Value = if path.exists() {
        ensure!(path.metadata()?.len() <= 65536, "config exceeds 64 KiB");
        toml::from_str(&fs::read_to_string(&path)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    let (key, value) = match key {
        "latency" => ("latency_ms", toml::Value::Float(value.parse()?)),
        "count-in" => ("count_in_bars", toml::Value::Integer(value.parse()?)),
        "pre-roll" => ("pre_roll_bars", toml::Value::Integer(value.parse()?)),
        "metronome" => {
            ensure!(["on", "off"].contains(&value), "metronome on|off");
            ("metronome", toml::Value::Boolean(value == "on"))
        }
        _ => anyhow::bail!("unknown recording setting"),
    };
    let root = document.as_table_mut().context("invalid config root")?;
    root.entry("recording")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .context("recording settings must be a table")?
        .insert(key.into(), value);
    let text = toml::to_string_pretty(&document)?;
    let settings = crate::config::Config::parse(&text)?.recording;
    fs::create_dir_all(path.parent().context("configuration needs a directory")?)?;
    crate::project::atomic(&path, text.as_bytes())?;
    Ok(settings)
}

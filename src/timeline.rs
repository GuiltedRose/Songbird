//! Quarter-note musical positions and analytic tempo integration. PPQ is display resolution.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const PPQ: u32 = 960;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tempo {
    pub beat: f64,
    pub bpm: f64,
    pub end: Option<f64>,
    pub end_bpm: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meter {
    pub bar: u32,
    pub signature: [u32; 2],
}
#[derive(Clone, Debug)]
struct Segment {
    beat: f64,
    seconds: f64,
    bpm: f64,
    slope: f64,
}
#[derive(Clone, Debug)]
pub struct Clock {
    segments: Vec<Segment>,
    meters: Vec<(u32, f64, [u32; 2])>,
}
fn duration(bpm: f64, slope: f64, beats: f64) -> f64 {
    if slope.abs() < 1e-12 {
        beats * 60. / bpm
    } else {
        60. / slope * (slope * beats / bpm).ln_1p()
    }
}
impl Clock {
    pub fn new(bpm: f64, signature: [u32; 2], tempos: &[Tempo], meters: &[Meter]) -> Result<Self> {
        ensure!(
            bpm.is_finite() && (20. ..=400.).contains(&bpm),
            "tempo range 20..400"
        );
        ensure!(
            tempos.len() <= 256 && meters.len() <= 256,
            "tempo/meter map limit 256"
        );
        let mut changes = tempos.to_vec();
        changes.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        let mut segments = vec![Segment {
            beat: 0.,
            seconds: 0.,
            bpm,
            slope: 0.,
        }];
        let mut last_end = 0.;
        let mut previous = None;
        for t in changes {
            ensure!(
                t.beat.is_finite()
                    && t.beat >= 0.
                    && t.beat <= 1e6
                    && t.bpm.is_finite()
                    && (20. ..=400.).contains(&t.bpm),
                "invalid tempo position/BPM"
            );
            ensure!(
                previous != Some(t.beat) && t.beat >= last_end,
                "conflicting or overlapping tempo changes at {}b",
                t.beat
            );
            let last = segments.last().unwrap();
            let seconds = last.seconds + duration(last.bpm, last.slope, t.beat - last.beat);
            if let Some(end) = t.end {
                let end_bpm = t.end_bpm.context("tempo ramp needs ending BPM")?;
                ensure!(
                    end.is_finite()
                        && end > t.beat
                        && end <= 1e6
                        && end_bpm.is_finite()
                        && (20. ..=400.).contains(&end_bpm),
                    "invalid tempo ramp"
                );
                let slope = (end_bpm - t.bpm) / (end - t.beat);
                segments.push(Segment {
                    beat: t.beat,
                    seconds,
                    bpm: t.bpm,
                    slope,
                });
                segments.push(Segment {
                    beat: end,
                    seconds: seconds + duration(t.bpm, slope, end - t.beat),
                    bpm: end_bpm,
                    slope: 0.,
                });
                last_end = end;
            } else {
                ensure!(t.end_bpm.is_none(), "tempo ending BPM requires a ramp");
                segments.push(Segment {
                    beat: t.beat,
                    seconds,
                    bpm: t.bpm,
                    slope: 0.,
                });
                last_end = t.beat;
            }
            previous = Some(t.beat);
        }
        let valid = |s: [u32; 2]| (1..=32).contains(&s[0]) && [1, 2, 4, 8, 16, 32].contains(&s[1]);
        ensure!(valid(signature), "invalid initial meter");
        let mut map = vec![(1, 0., signature)];
        let mut changes = meters.to_vec();
        changes.sort_by_key(|m| m.bar);
        for m in changes {
            let &(bar, beat, sig) = map.last().unwrap();
            ensure!(
                m.bar > bar && m.bar <= 4096 && valid(m.signature),
                "meter changes require unique bar boundaries after bar 1"
            );
            map.push((
                m.bar,
                beat + (m.bar - bar) as f64 * sig[0] as f64 * 4. / sig[1] as f64,
                m.signature,
            ));
        }
        Ok(Self {
            segments,
            meters: map,
        })
    }
    pub fn seconds(&self, beat: f64) -> f64 {
        let i = self
            .segments
            .partition_point(|s| s.beat <= beat)
            .saturating_sub(1);
        let s = &self.segments[i];
        s.seconds + duration(s.bpm, s.slope, beat - s.beat)
    }
    pub fn beat(&self, seconds: f64) -> f64 {
        let i = self
            .segments
            .partition_point(|s| s.seconds <= seconds)
            .saturating_sub(1);
        let s = &self.segments[i];
        let elapsed = seconds - s.seconds;
        s.beat
            + if s.slope.abs() < 1e-12 {
                elapsed * s.bpm / 60.
            } else {
                s.bpm * (elapsed * s.slope / 60.).exp_m1() / s.slope
            }
    }
    pub fn bpm(&self, beat: f64) -> f64 {
        let s = &self.segments[self
            .segments
            .partition_point(|s| s.beat <= beat)
            .saturating_sub(1)];
        s.bpm + s.slope * (beat - s.beat)
    }
    pub fn bar_beat(&self, bar: u32) -> f64 {
        let &(start, beat, sig) = &self.meters[self
            .meters
            .partition_point(|m| m.0 <= bar)
            .saturating_sub(1)];
        beat + bar.saturating_sub(start) as f64 * sig[0] as f64 * 4. / sig[1] as f64
    }
    /// Bar, notated beat (denominator unit), tick within that beat, active meter.
    pub fn position(&self, beat: f64) -> (u32, u32, u32, [u32; 2]) {
        let &(bar, start, sig) = &self.meters[self
            .meters
            .partition_point(|m| m.1 <= beat)
            .saturating_sub(1)];
        let unit = 4. / sig[1] as f64;
        let width = sig[0] as f64 * unit;
        let offset = ((beat - start).max(0.) / width).floor();
        let within = ((beat - start).max(0.) - offset * width).max(0.);
        let index = (within / unit).floor();
        (
            bar + offset as u32,
            index as u32 + 1,
            ((within - index * unit) * PPQ as f64).floor() as u32,
            sig,
        )
    }
    pub fn next_bar_frame(&self, frame: u64, rate: u32) -> u64 {
        let beat = self.beat(frame as f64 / rate as f64);
        let (bar, _, _, _) = self.position(beat);
        (self.seconds(self.bar_beat(bar + 1)) * rate as f64).round() as u64
    }
    pub fn parse(&self, value: &str, markers: &BTreeMap<String, f64>) -> Result<f64> {
        let seconds = if let Some(name) = value.strip_prefix('@') {
            self.seconds(
                *markers
                    .get(name)
                    .with_context(|| format!("unknown marker {name}"))?,
            )
        } else if let Some(value) = value.strip_suffix('b') {
            self.seconds(value.parse()?)
        } else if let Some(value) = value.strip_prefix("bar:") {
            let parts = value.split(':').collect::<Vec<_>>();
            ensure!(parts.len() <= 3, "position uses bar:BAR:BEAT[:TICK]");
            let bar: u32 = parts[0].parse()?;
            ensure!((1..=4096).contains(&bar), "bar range 1..4096");
            let beat: f64 = parts.get(1).unwrap_or(&"1").parse()?;
            let signature = self.position(self.bar_beat(bar)).3;
            ensure!(
                beat >= 1. && beat < signature[0] as f64 + 1.,
                "beat outside bar"
            );
            let tick: f64 = parts.get(2).unwrap_or(&"0").parse()?;
            ensure!(
                tick.is_finite() && tick >= 0. && tick < PPQ as f64 * 4. / signature[1] as f64,
                "tick lies outside notated beat"
            );
            self.seconds(
                self.bar_beat(bar) + (beat - 1.) * 4. / signature[1] as f64 + tick / PPQ as f64,
            )
        } else {
            crate::language::seconds(value, 60.)?
        };
        ensure!(
            seconds.is_finite() && (0. ..=86400.).contains(&seconds),
            "invalid timeline position"
        );
        Ok(seconds)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Transport {
    pub cursor: f64,
    pub range: Option<(f64, f64)>,
    pub looping: bool,
}
impl Transport {
    pub fn load(root: &std::path::Path) -> Result<Self> {
        let path = root.join("transport.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            path.metadata()?.len() <= 4096,
            "transport preferences exceed 4 KiB"
        );
        let state: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        state.validate()?;
        Ok(state)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.cursor.is_finite() && (0. ..=86400.).contains(&self.cursor),
            "invalid saved cursor"
        );
        if let Some((a, b)) = self.range {
            ensure!(
                a.is_finite() && b.is_finite() && a >= 0. && b > a && b <= 86400.,
                "invalid saved playback range"
            );
        }
        ensure!(
            !self.looping || self.range.is_some(),
            "saved transport loop needs a range"
        );
        Ok(())
    }
    pub fn save(&self, root: &std::path::Path) -> Result<()> {
        self.validate()?;
        crate::project::atomic(
            &root.join("transport.toml"),
            toml::to_string(self)?.as_bytes(),
        )
    }
}

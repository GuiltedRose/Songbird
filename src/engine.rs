use crate::{
    codec::{Asset, Encoder, safe_path},
    dsp::{self, Processor},
    language::{self, Clip, Event, Score, Track},
    voice::{self, LoadedBank, VocalUnit},
};
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, path::Path};
pub struct PreparedClip {
    pub spec: Clip,
    pub audio: Asset,
    pub effects: Vec<Processor>,
}
pub struct PreparedTrack {
    pub spec: Track,
    pub events: Vec<Event>,
    pub cycle: f64,
    pub clips: Vec<PreparedClip>,
    pub bank: Option<LoadedBank>,
    pub units: Vec<VocalUnit>,
    unit_ends: Vec<f64>,
    pub effects: Vec<Processor>,
    pub drums: BTreeMap<String, Asset>,
    phases: Vec<Vec<f64>>,
}
pub struct Renderer {
    pub score: Score,
    pub tracks: Vec<PreparedTrack>,
    pub position: u64,
    pub length: u64,
    master: Vec<Processor>,
    pub peak: f32,
    pub clipped: u64,
}
impl Renderer {
    pub fn prepare(root: &Path, mut score: Score) -> Result<Self> {
        crate::studio::apply_presets(root, &mut score)?;
        let used = score
            .tracks
            .iter()
            .filter_map(|t| t.voice.clone())
            .collect::<std::collections::BTreeSet<_>>();
        ensure!(
            score.tracks.iter().filter(|t| t.voice.is_some()).count() <= 16,
            "singer track limit 16"
        );
        let defaults = crate::phonetics::default_selection(root, None)?;
        for (id, voice) in &mut score.voices {
            if !used.contains(id) {
                continue;
            }
            if voice.bank == "default" || voice.bank.starts_with("default:") {
                let selected = crate::phonetics::Selection {
                    language: voice
                        .bank
                        .strip_prefix("default:")
                        .map(str::to_owned)
                        .or(voice.language.clone())
                        .or(score.language.clone())
                        .or(defaults.language.clone()),
                    inventory: voice
                        .inventory
                        .clone()
                        .or(score.inventory.clone())
                        .or(defaults.inventory.clone()),
                    provider: voice
                        .provider
                        .clone()
                        .or(score.provider.clone())
                        .or(defaults.provider.clone()),
                };
                voice.bank = crate::procedural::default_bank(root, &selected)?;
            }
        }

        let effect_count = score.master.len()
            + score
                .tracks
                .iter()
                .map(|t| t.effects.len() + t.clips.iter().map(|c| c.effects.len()).sum::<usize>())
                .sum::<usize>();
        ensure!(effect_count <= 256, "project effect limit 256");
        ensure!(
            score
                .tracks
                .iter()
                .map(|t| t.clips.len() + t.drums.len())
                .sum::<usize>()
                <= 512,
            "project clip/sample asset limit 512"
        );
        ensure!(
            score.tracks.iter().filter(|t| t.voice.is_some()).count() <= 16,
            "singer track limit 16"
        );
        let mut tracks = vec![];
        let cache = root.join("cache");
        let mut total_events = 0;
        let mut total_effects = score.master.len();
        for spec in &score.tracks {
            ensure!(
                [
                    "sine",
                    "saw",
                    "square",
                    "triangle",
                    "noise",
                    "analog",
                    "soft-lead",
                    "drumkit",
                    "sample"
                ]
                .contains(&spec.instrument.as_str()),
                "unknown instrument {}",
                spec.instrument
            );
            let mut events = language::events(&spec.sequence, spec.step)?;
            let cycle = events.last().map_or(0., |e| e.beat + e.duration);
            if spec.reverse {
                for e in &mut events {
                    e.beat = cycle - e.beat - e.duration;
                }
                events.sort_by(|a, b| a.beat.total_cmp(&b.beat));
            }
            total_events += events.len();
            ensure!(total_events <= 131072, "project event limit 131072");
            let mut clips = vec![];
            for c in &spec.clips {
                let audio = Asset::decode(&safe_path(root, &c.path)?, &cache)?;
                ensure!(
                    c.start < audio.duration()
                        && c.end
                            .is_none_or(|e| e <= audio.duration() + 1. / audio.rate as f64),
                    "clip trim exceeds {}",
                    c.path
                );
                total_effects += c.effects.len();
                clips.push(PreparedClip {
                    spec: c.clone(),
                    audio,
                    effects: dsp::effects(&c.effects, score.rate),
                });
            }
            ensure!(
                spec.instrument != "sample" || !clips.is_empty(),
                "sample instrument requires a sample file"
            );
            let (bank, units) = if let Some(name) = &spec.voice {
                let voice = &score.voices[name];
                let mut track = spec.clone();
                if track.expression == "normal"
                    && !voice.style.is_empty()
                    && !track.vocal_overrides.contains("expression")
                {
                    track.expression = voice.style.clone();
                }
                let mut bank = LoadedBank::load(root, voice, &cache)?;
                let context = crate::phonetics::for_track(root, &bank.meta, voice, &track, &score)?;
                ensure!(
                    context.inventory.inventory == bank.profile.inventory.inventory,
                    "incompatible phoneme inventories: track {} and bank {}",
                    context.inventory.inventory,
                    bank.profile.inventory.inventory
                );
                bank.profile = context;
                let mut units = voice::align(&bank, voice, &track, &events, score.tempo)?;
                units.sort_by(|a, b| a.start.total_cmp(&b.start));
                (Some(bank), units)
            } else {
                (None, vec![])
            };
            let mut drums = BTreeMap::new();
            for (name, path) in &spec.drums {
                drums.insert(
                    name.clone(),
                    Asset::decode(&safe_path(root, path)?, &cache)?,
                );
            }
            total_effects += spec.effects.len();
            ensure!(total_effects <= 256, "project effect limit 256");
            let phases = events.iter().map(|e| vec![0.; e.notes.len()]).collect();
            let mut furthest = 0f64;
            let unit_ends = units
                .iter()
                .map(|u| {
                    furthest = furthest.max(u.end);
                    furthest
                })
                .collect();
            tracks.push(PreparedTrack {
                phases,
                unit_ends,
                spec: spec.clone(),
                events,
                cycle,
                clips,
                bank,
                units,
                effects: dsp::effects(&spec.effects, score.rate),
                drums,
            });
        }
        let length = (score.duration() * score.rate as f64).round() as u64;
        let master = dsp::effects(&score.master, score.rate);
        Ok(Self {
            score,
            tracks,
            position: 0,
            length,
            master,
            peak: 0.,
            clipped: 0,
        })
    }
    pub fn block(&mut self, out: &mut [[f32; 2]]) -> Result<usize> {
        let count = out.len().min((self.length - self.position) as usize);
        let solo = self.tracks.iter().any(|t| t.spec.solo);
        let rate = self.score.rate;
        let bpm = self.score.tempo;
        for frame in &mut out[..count] {
            let time = self.position as f64 / rate as f64;
            let mut mix = [0.; 2];
            for t in &mut self.tracks {
                if t.spec.mute || (solo && !t.spec.solo) {
                    continue;
                }
                let local = time - t.spec.at;
                let mut x = [0.; 2];
                let mut gain = t.spec.gain;
                let mut pan = t.spec.pan;
                let mut bend = 0.;
                for a in &t.spec.automation {
                    let value = dsp::automation(
                        local,
                        &a.points,
                        match a.parameter.as_str() {
                            "gain" => gain,
                            "pan" => pan,
                            _ => 0.,
                        },
                    );
                    match a.parameter.as_str() {
                        "gain" => gain = value,
                        "pan" => pan = value,
                        "bend" => bend = value,
                        _ => {}
                    }
                }
                let mut beat = local * bpm / 60.;
                if t.spec.looping && t.cycle > 0. && beat >= 0. {
                    beat %= t.cycle;
                }
                if local >= 0. {
                    if let Some(bank) = &mut t.bank {
                        let voice_time = beat * 60. / bpm;
                        let end = t.units.partition_point(|u| u.start <= voice_time);
                        let start = t.unit_ends[..end].partition_point(|last| *last < voice_time);
                        for unit in &mut t.units[start..end] {
                            if voice_time >= unit.start && voice_time < unit.end {
                                let v = voice::sample(bank, unit, voice_time, &t.spec, bend, rate)?;
                                x[0] += v[0];
                                x[1] += v[1];
                            }
                        }
                    } else {
                        let end = t.events.partition_point(|e| e.beat <= beat);
                        for (event_index, e) in t.events[..end].iter().enumerate().rev() {
                            if e.beat + 64. < beat {
                                break;
                            }
                            let elapsed = (beat - e.beat) * 60. / bpm;
                            let duration = e.duration * 60. / bpm;
                            if elapsed < 0. || elapsed >= duration {
                                continue;
                            }
                            let mut value = 0.;
                            if let Some(drum) = &e.drum {
                                if let Some(a) = t.drums.get_mut(drum) {
                                    let v = a.sample(
                                        elapsed * a.rate as f64,
                                        a.rate as f64 / rate as f64,
                                    )?;
                                    value = (v[0] + v[1]) * 0.5;
                                } else {
                                    value =
                                        dsp::drum(drum, elapsed, (elapsed * rate as f64) as u64);
                                }
                            } else if t.spec.instrument == "sample" {
                                let c = &mut t.clips[0];
                                for &n in &e.notes {
                                    let speed = 2f64.powf(
                                        (n + t.spec.transpose - t.spec.sample_root) as f64 / 12.,
                                    );
                                    let pos = c.spec.start + elapsed * speed;
                                    if pos < c.spec.end.unwrap_or(c.audio.duration()) {
                                        let v = c.audio.sample(
                                            pos * c.audio.rate as f64,
                                            speed * c.audio.rate as f64 / rate as f64,
                                        )?;
                                        value += (v[0] + v[1]) * 0.5 * c.spec.gain;
                                    }
                                }
                            } else {
                                for (note_index, &n) in e.notes.iter().enumerate() {
                                    let pitch_env = t.spec.pitch_envelope[0] as f64
                                        * (-elapsed / t.spec.pitch_envelope[1] as f64).exp();
                                    let freq = dsp::hz(
                                        n as f64
                                            + t.spec.transpose as f64
                                            + bend as f64
                                            + pitch_env,
                                    );
                                    if freq < rate as f64 / 2. {
                                        value += dsp::oscillator(
                                            &t.spec.instrument,
                                            {
                                                let phase = &mut t.phases[event_index][note_index];
                                                if elapsed < 1.1 / rate as f64 {
                                                    *phase = 0.;
                                                }
                                                let current = *phase;
                                                *phase = (*phase + freq / rate as f64).fract();
                                                current
                                            },
                                            freq,
                                            rate,
                                            (elapsed * rate as f64) as u64,
                                        ) * dsp::envelope(elapsed, duration, &t.spec)
                                            * 0.22;
                                    }
                                }
                            }
                            x[0] += value;
                            x[1] += value;
                        }
                    }
                }
                if t.spec.instrument == "sample" {
                    dsp::apply(&mut x, &mut t.clips[0].effects);
                }
                if t.spec.instrument != "sample" {
                    for c in &mut t.clips {
                        let elapsed = local - c.spec.at;
                        let duration = (c.spec.end.unwrap_or(c.audio.duration()) - c.spec.start)
                            * c.spec.stretch;
                        let mut v = [0.; 2];
                        if elapsed >= 0. && elapsed < duration {
                            v = clip_sample(c, elapsed, rate)?;
                            let fade = (elapsed / c.spec.fade_in.max(1e-9)).min(1.)
                                * ((duration - elapsed) / c.spec.fade_out.max(1e-9)).min(1.);
                            let normal = if c.spec.normalize {
                                0.98 / c.audio.peak.max(1e-9)
                            } else {
                                1.
                            };
                            for x in &mut v {
                                *x *= c.spec.gain * normal * fade as f32;
                            }
                            v = dsp::pan(v, c.spec.pan);
                        }
                        dsp::apply(&mut v, &mut c.effects);
                        x[0] += v[0];
                        x[1] += v[1];
                    }
                }
                dsp::apply(&mut x, &mut t.effects);
                x = dsp::pan(x, pan);
                mix[0] += x[0] * gain;
                mix[1] += x[1] * gain;
            }
            dsp::apply(&mut mix, &mut self.master);
            for x in &mut mix {
                ensure!(x.is_finite(), "DSP produced nonfinite output");
                self.peak = self.peak.max(x.abs());
                if x.abs() > 1. {
                    self.clipped += 1;
                }
            }
            *frame = mix;
            self.position += 1;
        }
        Ok(count)
    }
    pub fn render(&mut self, path: &Path) -> Result<()> {
        let mut e = Encoder::create(path, self.score.rate, 2, self.score.bits)?;
        let mut block = [[0.; 2]; 1024];
        loop {
            let n = self.block(&mut block)?;
            if n == 0 {
                break;
            }
            e.write(&block[..n])?;
        }
        e.finish()
    }
    pub fn bar_frames(&self) -> u64 {
        (self.score.bar_beats() * 60. / self.score.tempo * self.score.rate as f64).round() as u64
    }
    pub fn seek(&mut self, target: u64) -> Result<()> {
        ensure!(
            target >= self.position && target <= self.length,
            "invalid seek"
        );
        let mut scratch = [[0.; 2]; 1024];
        while self.position < target {
            let n = (target - self.position).min(1024) as usize;
            self.block(&mut scratch[..n])?;
        }
        Ok(())
    }
}
fn clip_sample(c: &mut PreparedClip, t: f64, rate: u32) -> Result<[f32; 2]> {
    let spec = &c.spec;
    let end = spec.end.unwrap_or(c.audio.duration());
    let source_rate = c.audio.rate as f64;
    let source_time = |time: f64| {
        let time = time.clamp(0., end - spec.start);
        if spec.reverse {
            end - time - 1. / source_rate
        } else {
            spec.start + time
        }
    };
    let pitch = 2f64.powf(spec.pitch as f64 / 12.);
    let speed = c.audio.rate as f64 / rate as f64 * pitch;
    if (spec.stretch - 1.).abs() < 1e-9 && spec.pitch == 0. {
        return c.audio.sample(source_time(t) * c.audio.rate as f64, speed);
    }
    // Two overlapping Hann-windowed grains separate duration from pitch.
    let hop = 0.025;
    let k = (t / hop).floor() as i64;
    let mut out = [0.; 2];
    let mut weights = 0.;
    for grain in k - 1..=k + 1 {
        let center = grain as f64 * hop;
        let d = t - center;
        if d.abs() >= hop {
            continue;
        }
        let at = center / spec.stretch + d * pitch;
        let w = (0.5 + 0.5 * (std::f64::consts::PI * d / hop).cos()) as f32;
        let v = c
            .audio
            .sample(source_time(at) * c.audio.rate as f64, speed)?;
        out[0] += v[0] * w;
        out[1] += v[1] * w;
        weights += w;
    }
    if weights > 0. {
        out[0] /= weights;
        out[1] /= weights;
    }
    Ok(out)
}

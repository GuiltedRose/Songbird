use crate::{
    codec::{Asset, Encoder, safe_path},
    dsp::{self, Processor},
    language::{self, Clip, Event, Score, Track},
    voice::{self, LoadedBank, VocalUnit},
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path};
pub struct PreparedClip {
    pub spec: Clip,
    pub audio: Asset,
    pub effects: Vec<Processor>,
}
pub struct PreparedTrack {
    pub spec: Track,
    pub events: Vec<Event>,
    schedule: Vec<Event>,
    max_event_duration: f64,
    pub cycle: f64,
    pub clips: Vec<PreparedClip>,
    pub bank: Option<LoadedBank>,
    pub units: Vec<VocalUnit>,
    unit_ends: Vec<f64>,
    pub effects: Vec<Processor>,
    pub drums: BTreeMap<String, Asset>,
    phases: Vec<Vec<f64>>,
}
#[derive(Clone)]
struct TrackState {
    phases: Vec<Vec<f64>>,
    units: Vec<VocalUnit>,
    effects: Vec<Processor>,
    clips: Vec<Vec<Processor>>,
}
#[derive(Clone)]
struct Checkpoint {
    position: u64,
    tracks: Vec<TrackState>,
    master: Vec<Processor>,
    mixer: Option<crate::mixer::Mixer>,
}
pub struct Renderer {
    initial: Option<Box<Checkpoint>>,
    loop_state: Option<Box<Checkpoint>>,
    pub loop_end: Option<u64>,
    pub loop_start: Option<u64>,
    pub clock: crate::timeline::Clock,
    pub count_in_frames: u64,
    count_in_total: u64,
    count_in_end: f64,
    pub metronome: bool,
    pub score: Score,
    pub tracks: Vec<PreparedTrack>,
    pub position: u64,
    pub length: u64,
    master: Vec<Processor>,
    pub peak: f32,
    pub clipped: u64,
    pub channel_peaks: Vec<f32>,
    pub channel_clipped: Vec<u64>,
    pub channel_reduction: Vec<f32>,
    pub mixer: Option<crate::mixer::Mixer>,
    last_frame: [f32; 2],
    transition: Option<([f32; 2], u32)>,
}
impl Renderer {
    pub fn prepare(root: &Path, mut score: Score) -> Result<Self> {
        let clock = score.clock()?;
        crate::studio::apply_presets(root, &mut score)?;
        ensure!(score.tracks.len() <= 128, "expanded track limit 128");
        ensure!(
            score
                .tracks
                .iter()
                .all(|t| t.bypassed.iter().all(|&i| i > 0 && i <= t.effects.len())),
            "unknown bypassed insert after preset expansion"
        );
        let effect_count = score
            .mixer
            .iter()
            .map(|n| n.controls.effects.len())
            .sum::<usize>()
            + score.master.len()
            + score
                .tracks
                .iter()
                .map(|t| t.effects.len() + t.clips.iter().map(|c| c.effects.len()).sum::<usize>())
                .sum::<usize>();
        ensure!(effect_count <= 256, "project effect limit 256");
        let state_estimate: usize = score
            .master
            .iter()
            .chain(score.tracks.iter().flat_map(|t| &t.effects))
            .chain(
                score
                    .tracks
                    .iter()
                    .flat_map(|t| t.clips.iter().flat_map(|c| &c.effects)),
            )
            .chain(score.mixer.iter().flat_map(|n| &n.controls.effects))
            .map(|effect| dsp::estimated_state_bytes(effect, score.rate))
            .sum();
        ensure!(
            state_estimate <= 64 * 1024 * 1024,
            "effect checkpoint state exceeds 64 MiB"
        );
        let mixer = crate::mixer::Mixer::compile(&score)?;
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
            let mini_cycles = if spec.looping {
                ((clock.beat(score.duration()) - clock.beat(spec.at)).max(0.) / spec.cycle_beats)
                    .max(1.)
            } else {
                spec.cycles as f64
            };
            if let Some(label) = &spec.drum_only {
                ensure!(
                    spec.instrument == "drumkit"
                        && (dsp::DRUM_NAMES.contains(&crate::strudel::drum_alias(label))
                            || spec.drums.contains_key(label)),
                    "drum-only requires a drumkit and a known drum/sample label: {label}"
                );
            }
            let mut events = if spec.mini {
                crate::mini::Pattern::compile(&spec.sequence)
                    .and_then(|p| p.events(0., mini_cycles, spec.cycle_beats, spec.seed))
                    .map_err(|e| {
                        anyhow::anyhow!("track {:?}, mini {:?}: {e:#}", spec.name, spec.sequence)
                    })?
            } else {
                language::events(&spec.sequence, spec.step)?
            };
            if spec.mini {
                crate::strudel::controls(&mut events, spec, mini_cycles)?;
            }
            for event in &mut events {
                if let Some(name) = &mut event.drum
                    && !spec.drums.contains_key(name)
                {
                    *name = crate::strudel::drum_alias(name).into();
                }
            }
            let cycle = if spec.mini {
                mini_cycles * spec.cycle_beats
            } else {
                events.last().map_or(0., |e| e.beat + e.duration)
            };
            if spec.mini {
                for e in &events {
                    if let Some(label) = &e.drum {
                        ensure!(
                            dsp::DRUM_NAMES.contains(&label.as_str())
                                || spec.drums.contains_key(label),
                            "track {:?}: mini sample {label:?} has no drum mapping",
                            spec.name
                        );
                    }
                }
                if spec.voice.is_some() {
                    ensure!(
                        events.iter().all(|e| e.drum.is_none()),
                        "track {:?}: singer mini pattern must contain pitches and rests",
                        spec.name
                    );
                    ensure!(
                        events
                            .windows(2)
                            .all(|w| w[0].beat + w[0].duration <= w[1].beat + 1e-9),
                        "track {:?}: overlapping mini singer notes; use separate singer tracks or sequential notes",
                        spec.name
                    );
                }
            }
            if spec.reverse {
                for e in &mut events {
                    e.beat = cycle - e.beat - e.duration;
                }
                events.sort_by(|a, b| a.beat.total_cmp(&b.beat));
            }
            if spec.looping && !spec.mini && !score.tempo_map.is_empty() && cycle > 0. {
                let original = events.clone();
                let end = clock.beat(score.duration()) - clock.beat(spec.at);
                let repetitions = (end.max(0.) / cycle).ceil() as usize;
                ensure!(
                    original.len().saturating_mul(repetitions) <= 65536,
                    "tempo-map loop event limit 65536"
                );
                for repeat in 1..repetitions {
                    events.extend(original.iter().cloned().map(|mut e| {
                        e.beat += repeat as f64 * cycle;
                        e
                    }));
                }
            }
            let mut schedule: Vec<Event> = if score.tempo_map.is_empty() {
                events.clone()
            } else {
                let base = clock.beat(spec.at);
                events
                    .iter()
                    .cloned()
                    .map(|mut event| {
                        let start = clock.seconds(base + event.beat) - spec.at;
                        let end = clock.seconds(base + event.beat + event.duration) - spec.at;
                        event.beat = start * score.tempo / 60.;
                        event.duration = (end - start) * score.tempo / 60.;
                        event
                    })
                    .collect()
            };
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
                let mut units = voice::align(&bank, voice, &track, &schedule, score.tempo)?;
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
            // Compatibility sound events play a bounded source tail independently of
            // pattern speed. Sampler end is a fraction of the actual local sample.
            for event in &mut schedule {
                if let Some(playback) = &event.playback {
                    let name = event
                        .drum
                        .as_deref()
                        .context("sound event needs percussion")?;
                    let whole = drums
                        .get(name)
                        .map_or_else(|| dsp::drum_duration(name), |asset| asset.duration());
                    ensure!(
                        whole.is_finite() && whole > 0.,
                        "sound-expression source must contain finite nonempty audio"
                    );
                    let mut seconds = whole * playback.end;
                    if let Some(decay) = playback.decay.filter(|_| playback.sustain == 0.) {
                        seconds = seconds.min(decay as f64 + 0.0005);
                    }
                    ensure!(
                        seconds.is_finite() && seconds <= 30.,
                        "sound-expression playback tails are limited to 30 seconds; shorten end/decay"
                    );
                    event.duration = seconds * score.tempo / 60.;
                }
            }
            if spec.mini_sound || !spec.mini_controls.is_empty() {
                let longest = schedule
                    .iter()
                    .map(|event| event.duration)
                    .fold(0f64, f64::max);
                let mut first = 0;
                for (index, event) in schedule.iter().enumerate() {
                    while first < index && schedule[first].beat + longest < event.beat {
                        first += 1;
                    }
                    ensure!(
                        index - first < 64,
                        "sound expression is too dense for its playback tails; shorten end/decay or reduce speed (64 candidates per tail window)"
                    );
                }
            }
            tracks.push(PreparedTrack {
                phases,
                unit_ends,
                spec: spec.clone(),
                max_event_duration: schedule.iter().map(|e| e.duration).fold(0., f64::max),
                schedule,
                events,
                cycle,
                clips,
                bank,
                units,
                effects: dsp::effects_bypassed(&spec.effects, &spec.bypassed, score.rate),
                drums,
            });
        }
        let length = (score.duration() * score.rate as f64).round() as u64;
        let master = dsp::effects(&score.master, score.rate);
        let mut renderer = Self {
            initial: None,
            loop_state: None,
            loop_end: None,
            loop_start: None,
            clock,
            count_in_frames: 0,
            count_in_total: 0,
            count_in_end: 0.,
            metronome: false,
            score: score.clone(),
            tracks,
            position: 0,
            length,
            master,
            peak: 0.,
            clipped: 0,
            channel_peaks: vec![0.; score.tracks.len()],
            channel_clipped: vec![0; score.tracks.len()],
            channel_reduction: vec![0.; score.tracks.len()],
            mixer,
            last_frame: [0.; 2],
            transition: None,
        };
        let state_bytes = renderer
            .mixer
            .as_ref()
            .map_or(0, crate::mixer::Mixer::state_bytes)
            + renderer
                .master
                .iter()
                .map(Processor::state_bytes)
                .sum::<usize>()
            + renderer
                .tracks
                .iter()
                .map(|t| {
                    t.effects.iter().map(Processor::state_bytes).sum::<usize>()
                        + t.clips
                            .iter()
                            .flat_map(|c| &c.effects)
                            .map(Processor::state_bytes)
                            .sum::<usize>()
                })
                .sum::<usize>();
        ensure!(
            state_bytes <= 64 * 1024 * 1024,
            "effect checkpoint state exceeds 64 MiB; reduce delay/reverb tracks"
        );
        renderer.initial = Some(Box::new(renderer.checkpoint()));
        Ok(renderer)
    }
    pub fn block(&mut self, out: &mut [[f32; 2]]) -> Result<usize> {
        if self.count_in_frames > 0 {
            let n = out.len().min(self.count_in_frames as usize);
            let beginning = self.count_in_end - self.count_in_total as f64 / self.score.rate as f64;
            for frame in &mut out[..n] {
                let time = beginning
                    + (self.count_in_total - self.count_in_frames) as f64 / self.score.rate as f64;
                let click = self.click(time);
                *frame = [click; 2];
                self.count_in_frames -= 1;
            }
            return Ok(n);
        }
        if self.loop_end.is_some_and(|end| self.position >= end)
            && let Some(state) = self.loop_state.take()
        {
            let last = self.last_frame;
            self.restore(&state);
            self.fade_from(last);
            self.loop_state = Some(state);
        }
        let end = self.loop_end.unwrap_or(self.length).min(self.length);
        let count = out.len().min(end.saturating_sub(self.position) as usize);
        self.channel_peaks.fill(0.);
        let solo = self.tracks.iter().any(|t| t.spec.solo);
        let rate = self.score.rate;
        let bpm = self.score.tempo;
        for frame in &mut out[..count] {
            let time = self.position as f64 / rate as f64;
            let mut mix = [0.; 2];
            if let Some(mixer) = &mut self.mixer {
                mixer.begin_frame();
            }
            for (channel, t) in self.tracks.iter_mut().enumerate() {
                if t.spec.mute
                    || self
                        .mixer
                        .as_ref()
                        .map_or(solo && !t.spec.solo, |m| !m.track_audible(channel))
                {
                    continue;
                }
                let local = time - t.spec.at;
                let mut x = [0.; 2];
                let mut gain = t.spec.gain;
                let mut pan = t.spec.pan;
                let mut bend = 0.;
                for a in &t.spec.automation {
                    let evaluate = if a.step {
                        dsp::automation_step
                    } else {
                        dsp::automation
                    };
                    let value = evaluate(
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
                        "filter" => {
                            for effect in &mut t.effects {
                                effect.automate_filter(value);
                            }
                        }
                        "formant" => t.spec.formant = value,
                        "dynamics" => t.spec.dynamics = value,
                        "vibrato" => t.spec.vibrato = value,
                        "breathiness" => t.spec.breathiness = value,
                        "vocal-pitch" => t.spec.vocal_pitch = value,
                        target if target.starts_with("effect.") => {
                            if let Some((i, j)) = target[7..].split_once('.')
                                && let (Ok(i), Ok(j)) = (i.parse::<usize>(), j.parse::<usize>())
                                && let Some(effect) = t.effects.get_mut(i.saturating_sub(1))
                            {
                                effect.automate_argument(j.saturating_sub(1), value);
                            }
                        }
                        _ => {}
                    }
                }
                let mut beat = local * bpm / 60.;
                if t.spec.looping
                    && !t.spec.mini
                    && self.score.tempo_map.is_empty()
                    && t.cycle > 0.
                    && beat >= 0.
                {
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
                        let end = t.schedule.partition_point(|e| e.beat <= beat);
                        for (event_index, e) in t.schedule[..end].iter().enumerate().rev() {
                            if e.beat + t.max_event_duration < beat {
                                break;
                            }
                            let elapsed = (beat - e.beat) * 60. / bpm;
                            let duration = e.duration * 60. / bpm;
                            if elapsed < 0. || elapsed >= duration {
                                continue;
                            }
                            let mut value = 0.;
                            if let Some(drum) = &e.drum {
                                if t.spec.drum_only.as_ref().is_some_and(|selected| {
                                    let selected = if t.spec.drums.contains_key(selected) {
                                        selected.as_str()
                                    } else {
                                        crate::strudel::drum_alias(selected)
                                    };
                                    selected != drum
                                }) {
                                    continue;
                                }
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
                            if let Some(playback) = &e.playback
                                && let Some(decay) = playback.decay
                            {
                                let phase = ((elapsed - 0.0005).max(0.) as f32 / decay.max(1e-9))
                                    .clamp(0., 1.);
                                value *= 1. - (1. - playback.sustain) * phase;
                            }
                            if e.playback.is_some() {
                                value *= (elapsed / 0.0005).clamp(0., 1.) as f32
                                    * ((duration - elapsed) / 0.002).clamp(0., 1.) as f32;
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
                let reduction = t
                    .effects
                    .iter()
                    .chain(t.clips.iter().flat_map(|c| &c.effects))
                    .map(|e| e.reduction_db)
                    .fold(0f32, f32::max);
                self.channel_reduction[channel] = self.channel_reduction[channel].max(reduction);
                x = dsp::pan(x, pan);
                self.channel_peaks[channel] =
                    self.channel_peaks[channel].max((x[0] * gain).abs().max((x[1] * gain).abs()));
                let post = [x[0] * gain, x[1] * gain];
                if post[0].abs().max(post[1].abs()) > 1. {
                    self.channel_clipped[channel] += 1;
                }
                if let Some(mixer) = &mut self.mixer {
                    mixer.track(channel, x, post, &mut mix);
                } else {
                    mix[0] += post[0];
                    mix[1] += post[1];
                }
            }
            if let Some(mixer) = &mut self.mixer {
                mixer.process(time, &mut mix)?;
            }
            dsp::apply(&mut mix, &mut self.master);
            if self.metronome {
                let click = self.click(time);
                mix[0] += click;
                mix[1] += click;
            }
            for x in &mut mix {
                ensure!(x.is_finite(), "DSP produced nonfinite output");
                self.peak = self.peak.max(x.abs());
                if x.abs() > 1. {
                    self.clipped += 1;
                }
            }
            if let Some((from, remaining)) = &mut self.transition {
                let blend = 1. - *remaining as f32 / 128.;
                for ch in 0..2 {
                    mix[ch] = from[ch] * (1. - blend) + mix[ch] * blend;
                }
                *remaining -= 1;
                if *remaining == 0 {
                    self.transition = None;
                }
            }
            self.last_frame = mix;
            *frame = mix;
            self.position += 1;
        }
        Ok(count)
    }
    pub fn fade_from(&mut self, sample: [f32; 2]) {
        self.transition = Some((sample, 128));
    }
    pub fn render(&mut self, path: &Path) -> Result<()> {
        ensure!(
            self.loop_end.is_none() && self.count_in_frames == 0,
            "offline export needs an arrangement renderer without transport loops or count-in"
        );
        let estimate = self
            .length
            .saturating_sub(self.position)
            .saturating_mul(2 * self.score.bits as u64 / 8)
            .saturating_add(1048576);
        crate::storage::preflight(
            path,
            estimate,
            crate::phonetics::app_config().storage.minimum_free_bytes,
        )?;
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
        ensure!(target <= self.length, "seek exceeds arrangement");
        if target < self.position {
            let state = self.initial.take().unwrap();
            self.restore(&state);
            self.initial = Some(state);
        }
        let loop_end = self.loop_end.take();
        self.transition = None;
        let result = (|| -> Result<()> {
            let mut scratch = [[0.; 2]; 1024];
            while self.position < target {
                let n = (target - self.position).min(1024) as usize;
                ensure!(self.block(&mut scratch[..n])? > 0, "seek made no progress");
            }
            Ok(())
        })();
        self.loop_end = loop_end;
        result
    }
    fn click(&self, seconds: f64) -> f32 {
        let beat = self.clock.beat(seconds);
        let (bar, _, _, signature) = self.clock.position(beat);
        let origin = if beat < 0. {
            0.
        } else {
            self.clock.bar_beat(bar)
        };
        let unit = 4. / signature[1] as f64;
        let index = ((beat - origin) / unit).floor();
        let onset = self.clock.seconds(origin + index * unit);
        let elapsed = seconds - onset;
        if !(0. ..0.035).contains(&elapsed) {
            return 0.;
        }
        let frequency = if (index as i64).rem_euclid(signature[0] as i64) == 0 {
            1800.
        } else {
            1200.
        };
        ((std::f64::consts::TAU * frequency * elapsed).sin() * (-elapsed / 0.008).exp() * 0.15)
            as f32
    }
    pub fn count_in(&mut self, bars: u32) {
        let end = self.position as f64 / self.score.rate as f64;
        let beat = self.clock.beat(end);
        let signature = self.clock.position(beat).3;
        let start = beat - bars as f64 * signature[0] as f64 * 4. / signature[1] as f64;
        self.count_in_total =
            ((end - self.clock.seconds(start)) * self.score.rate as f64).round() as u64;
        self.count_in_frames = self.count_in_total;
        self.count_in_end = end;
    }
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            position: self.position,
            master: self.master.clone(),
            mixer: self.mixer.clone(),
            tracks: self
                .tracks
                .iter()
                .map(|t| TrackState {
                    phases: t.phases.clone(),
                    units: t.units.clone(),
                    effects: t.effects.clone(),
                    clips: t.clips.iter().map(|c| c.effects.clone()).collect(),
                })
                .collect(),
        }
    }
    fn restore(&mut self, state: &Checkpoint) {
        self.position = state.position;
        self.master.clone_from(&state.master);
        if let (Some(mixer), Some(saved)) = (&mut self.mixer, &state.mixer) {
            mixer.restore_state(saved);
        }
        for (t, s) in self.tracks.iter_mut().zip(&state.tracks) {
            t.phases.clone_from(&s.phases);
            t.units.clone_from(&s.units);
            t.effects.clone_from(&s.effects);
            for (clip, effects) in t.clips.iter_mut().zip(&s.clips) {
                clip.effects.clone_from(effects);
            }
        }
    }
    pub fn transport(&mut self, start: u64, end: u64, looping: bool) -> Result<()> {
        ensure!(
            start < end && end <= self.length,
            "transport range requires start < end <= arrangement end"
        );
        ensure!(
            !looping || end - start >= 128,
            "transport loops must span at least 128 sample frames"
        );
        self.loop_end = None;
        self.loop_start = None;
        self.loop_state = None;
        self.seek(start)?;
        if looping {
            self.loop_state = Some(Box::new(self.checkpoint()));
            self.loop_end = Some(end);
            self.loop_start = Some(start);
        } else {
            self.length = end;
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

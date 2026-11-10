use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
    #[serde(default)]
    pub tempo_map: Vec<crate::timeline::Tempo>,
    #[serde(default)]
    pub meter_map: Vec<crate::timeline::Meter>,
    #[serde(default)]
    pub markers: BTreeMap<String, f64>,
    pub name: String,
    pub tempo: f64,
    pub signature: [u32; 2],
    pub bars: u32,
    pub rate: u32,
    pub bits: u16,
    pub tracks: Vec<Track>,
    pub voices: BTreeMap<String, Voice>,
    pub master: Vec<Effect>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub inventory: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}
impl Default for Score {
    fn default() -> Self {
        Self {
            tempo_map: vec![],
            meter_map: vec![],
            markers: BTreeMap::new(),
            name: "Untitled".into(),
            tempo: 120.,
            signature: [4, 4],
            bars: 4,
            rate: 48000,
            bits: 24,
            tracks: vec![],
            voices: BTreeMap::new(),
            master: vec![],
            language: None,
            inventory: None,
            provider: None,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Voice {
    pub bank: String,
    pub register: String,
    pub style: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub inventory: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub instrument: String,
    pub sequence: String,
    pub step: f64,
    #[serde(default)]
    pub mini: bool,
    #[serde(default = "default_cycle")]
    pub cycle_beats: f64,
    #[serde(default = "default_cycles")]
    pub cycles: u32,
    #[serde(default)]
    pub seed: u64,
    pub gain: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub looping: bool,
    pub at: f64,
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub selected_clip: Option<usize>,
    #[serde(default)]
    pub snap_grid: Option<f64>,
    #[serde(skip)]
    pub timeline_edits: Vec<(usize, String, Option<String>)>,
    pub effects: Vec<Effect>,
    pub voice: Option<String>,
    pub lyrics: String,
    pub vibrato: f32,
    pub breathiness: f32,
    pub expression: String,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub drums: BTreeMap<String, String>,
    pub sample_root: i32,
    pub pitch_envelope: [f32; 2],
    pub automation: Vec<Automation>,
    pub transpose: i32,
    pub reverse: bool,
    #[serde(default)]
    pub vocal_preset: Option<String>,
    #[serde(default)]
    pub vocal_overrides: BTreeSet<String>,
    #[serde(default = "one")]
    pub formant: f32,
    #[serde(default = "one")]
    pub dynamics: f32,
    #[serde(default)]
    pub vocal_pitch: f32,
    #[serde(default)]
    pub harmonies: Vec<i32>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub inventory: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}
fn default_cycle() -> f64 {
    4.
}
fn default_cycles() -> u32 {
    1
}
fn one() -> f32 {
    1.
}
impl Track {
    pub fn new(name: String) -> Self {
        Self {
            name,
            instrument: "sine".into(),
            sequence: String::new(),
            step: 0.5,
            mini: false,
            cycle_beats: 4.,
            cycles: 1,
            seed: 0,
            gain: 1.,
            pan: 0.,
            mute: false,
            solo: false,
            looping: false,
            at: 0.,
            clips: vec![],
            selected_clip: None,
            snap_grid: None,
            timeline_edits: vec![],
            effects: vec![],
            voice: None,
            lyrics: String::new(),
            vibrato: 0.,
            breathiness: 0.,
            expression: "normal".into(),
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.1,
            drums: BTreeMap::new(),
            sample_root: 60,
            pitch_envelope: [0., 0.1],
            automation: vec![],
            transpose: 0,
            reverse: false,
            vocal_preset: None,
            vocal_overrides: BTreeSet::new(),
            formant: 1.,
            dynamics: 1.,
            vocal_pitch: 0.,
            harmonies: vec![],
            language: None,
            inventory: None,
            provider: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    #[serde(default)]
    pub id: String,
    pub path: String,
    pub at: f64,
    #[serde(default)]
    pub at_beat: Option<f64>,
    pub start: f64,
    pub end: Option<f64>,
    pub gain: f32,
    pub pan: f32,
    pub fade_in: f64,
    pub fade_out: f64,
    pub reverse: bool,
    pub stretch: f64,
    pub pitch: f32,
    pub normalize: bool,
    pub group: String,
    pub effects: Vec<Effect>,
}
impl Clip {
    pub fn new(path: String) -> Self {
        Self {
            id: String::new(),
            path,
            at: 0.,
            at_beat: None,
            start: 0.,
            end: None,
            gain: 1.,
            pan: 0.,
            fade_in: 0.,
            fade_out: 0.,
            reverse: false,
            stretch: 1.,
            pitch: 0.,
            normalize: false,
            group: String::new(),
            effects: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub kind: String,
    pub args: Vec<f32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Automation {
    #[serde(default)]
    pub step: bool,
    pub parameter: String,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub musical: Vec<Option<f64>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub beat: f64,
    pub duration: f64,
    pub notes: Vec<i32>,
    pub drum: Option<String>,
}

pub fn note(s: &str) -> Result<i32> {
    let bytes = s.as_bytes();
    ensure!(bytes.len() >= 2, "note needs an octave: {s}");
    let base = match bytes[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => bail!("invalid note {s}"),
    };
    let (acc, i) = match bytes[1] {
        b'#' => (1, 2),
        b'b' => (-1, 2),
        _ => (0, 1),
    };
    let octave: i32 = s[i..]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid octave: {s}"))?;
    let n = octave
        .checked_add(1)
        .and_then(|o| o.checked_mul(12))
        .and_then(|o| o.checked_add(base + acc))
        .ok_or_else(|| anyhow::anyhow!("note outside MIDI range: {s}"))?;
    ensure!((0..=127).contains(&n), "note outside MIDI range: {s}");
    Ok(n)
}
fn musical_at(value: &str, markers: &BTreeMap<String, f64>) -> Result<Option<f64>> {
    if let Some(name) = value.strip_prefix('@') {
        Ok(Some(*markers.get(name).with_context(|| {
            format!("declare marker {name:?} before using it")
        })?))
    } else {
        musical(value)
    }
}
fn musical(value: &str) -> Result<Option<f64>> {
    if let Some(value) = value.strip_suffix('b') {
        let value: f64 = value.parse()?;
        ensure!(
            value.is_finite() && (0. ..=1e6).contains(&value),
            "invalid beat position"
        );
        Ok(Some(value))
    } else {
        Ok(None)
    }
}
pub fn seconds(s: &str, tempo: f64) -> Result<f64> {
    let v = if let Some(v) = s.strip_suffix("ms") {
        v.parse::<f64>()? / 1000.
    } else if let Some(v) = s.strip_suffix('b') {
        v.parse::<f64>()? * 60. / tempo
    } else if s.contains(':') {
        let mut v = 0.;
        for x in s.split(':') {
            v = v * 60. + x.parse::<f64>()?;
        }
        v
    } else {
        s.trim_end_matches('s').parse()?
    };
    ensure!(
        v.is_finite() && (0.0..=86400.).contains(&v),
        "invalid time {s}"
    );
    Ok(v)
}
pub fn gain(s: &str) -> Result<f32> {
    let v = if let Some(db) = s.strip_suffix("db") {
        10f32.powf(db.parse::<f32>()? / 20.)
    } else {
        s.parse()?
    };
    ensure!(v.is_finite() && (0.0..=16.).contains(&v), "invalid gain");
    Ok(v)
}

// Quotes preserve whitespace; braces and semicolons delimit statements. No evaluation or shell expansion.
pub fn lex(src: &str) -> Result<Vec<Vec<String>>> {
    ensure!(src.len() <= 1_048_576, "composition exceeds 1 MiB");
    let mut lines = vec![];
    let mut row = vec![];
    let mut token = String::new();
    let mut quote = false;
    let mut escape = false;
    let mut comment = false;
    for c in src.chars().chain(std::iter::once('\n')) {
        if comment {
            if c != '\n' {
                continue;
            }
            comment = false;
        }
        if quote {
            if escape {
                token.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    x => x,
                });
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quote = false;
                row.push(std::mem::take(&mut token));
            } else {
                token.push(c);
            }
            continue;
        }
        match c {
            '"' => {
                ensure!(token.is_empty(), "quote must start a token");
                quote = true;
            }
            '#' if token.is_empty() => comment = true,
            '\n' | ';' | '{' | '}' => {
                if !token.is_empty() {
                    row.push(std::mem::take(&mut token));
                }
                if c == '{' {
                    row.push("{".into());
                }
                if !row.is_empty() {
                    lines.push(std::mem::take(&mut row));
                }
                if c == '}' {
                    lines.push(vec!["}".into()]);
                }
            }
            c if c.is_whitespace() => {
                if !token.is_empty() {
                    row.push(std::mem::take(&mut token));
                }
            }
            _ => token.push(c),
        }
    }
    ensure!(!quote, "unterminated quoted string");
    Ok(lines)
}
fn arg(v: &[String], i: usize) -> Result<&str> {
    v.get(i)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing argument"))
}
fn number(v: &[String], i: usize) -> Result<f32> {
    let n: f32 = arg(v, i)?.parse()?;
    ensure!(n.is_finite(), "nonfinite number");
    Ok(n)
}
pub(crate) fn effect(v: &[String]) -> Result<Effect> {
    ensure!(!v.is_empty(), "effect name required");
    let count = match v[0].as_str() {
        "eq" => 3,
        "compress" => 2,
        "limit" | "gate" | "reverb" | "denoise" | "filter" => 1,
        "delay" => 3,
        _ => bail!("unknown effect {}", v[0]),
    };
    ensure!(
        v.len() == count + 1,
        "{} expects {count} numeric arguments",
        v[0]
    );
    let args = (1..v.len())
        .map(|i| number(v, i))
        .collect::<Result<Vec<_>>>()?;
    match v[0].as_str() {
        "delay" => ensure!(
            args[0] > 0. && args[0] <= 4. && args[1].abs() < 1. && (0.0..=1.).contains(&args[2]),
            "delay: seconds (0,4], feedback (-1,1), wet [0,1]"
        ),
        "eq" => ensure!(
            args.iter().all(|v| (-24.0..=24.).contains(v)),
            "EQ range -24..24 dB"
        ),
        "compress" => ensure!(
            args[0] <= 0. && args[0] >= -90. && args[1] >= 1. && args[1] <= 100.,
            "compress: threshold dB [-90,0], ratio [1,100]"
        ),
        "filter" => ensure!(
            (20.0..=20000.).contains(&args[0]),
            "filter range 20..20000 Hz"
        ),
        "reverb" => ensure!((0.0..=1.).contains(&args[0]), "reverb range 0..1"),
        _ => ensure!(
            (-100.0..=0.).contains(&args[0]),
            "threshold range -100..0 dB"
        ),
    };
    Ok(Effect {
        kind: v[0].clone(),
        args,
    })
}
pub fn parse(src: &str) -> Result<Score> {
    let rows = lex(src)?;
    let mut score = Score::default();
    let mut vars = BTreeMap::<String, String>::new();
    let mut instruments = BTreeMap::<String, Track>::new();
    let mut section: Option<(String, f64)> = None;
    let mut section_beat = None;
    let mut section_repeat = (1usize, 0f64, 0usize);
    let mut track_positions = vec![];
    let mut i = 0;
    while i < rows.len() {
        let v = &rows[i];
        let result = (|| -> Result<()> {
            match v[0].as_str() {
                "language" => score.language = Some(crate::locale::normalize(arg(v, 1)?)?),
                "inventory" => score.inventory = Some(crate::locale::normalize(arg(v, 1)?)?),
                "provider" => score.provider = Some(arg(v, 1)?.into()),
                "project" => score.name = arg(v, 1)?.into(),
                "marker" => {
                    ensure!(arg(v, 2)? == "at", "marker NAME at BEATb");
                    let beat =
                        musical(arg(v, 3)?)?.context("marker needs quarter-note beats (Nb)")?;
                    ensure!(
                        score.markers.len() < 1024
                            && score.markers.insert(arg(v, 1)?.into(), beat).is_none(),
                        "duplicate/too many markers"
                    );
                }
                "tempo" if v.get(1).is_some_and(|x| x == "at" || x == "ramp") => {
                    if arg(v, 1)? == "at" {
                        score.tempo_map.push(crate::timeline::Tempo {
                            beat: musical_at(arg(v, 2)?, &score.markers)?
                                .context("tempo position needs Nb or @marker")?,
                            bpm: arg(v, 3)?.parse()?,
                            end: None,
                            end_bpm: None,
                        });
                    } else {
                        score.tempo_map.push(crate::timeline::Tempo {
                            beat: musical_at(arg(v, 2)?, &score.markers)?
                                .context("ramp needs Nb or @marker")?,
                            end: Some(
                                musical_at(arg(v, 3)?, &score.markers)?
                                    .context("ramp needs Nb or @marker")?,
                            ),
                            bpm: arg(v, 4)?.parse()?,
                            end_bpm: Some(arg(v, 5)?.parse()?),
                        });
                    }
                }
                "signature" if v.get(1).is_some_and(|x| x == "at") => {
                    ensure!(arg(v, 2)? == "bar", "signature at bar NUMBER N/D");
                    let (a, b) = arg(v, 4)?.split_once('/').context("signature needs N/D")?;
                    score.meter_map.push(crate::timeline::Meter {
                        bar: arg(v, 3)?.parse()?,
                        signature: [a.parse()?, b.parse()?],
                    });
                }
                "tempo" => {
                    score.tempo = number(v, 1)? as f64;
                    ensure!((20.0..=400.).contains(&score.tempo), "tempo range 20..400");
                }
                "signature" => {
                    let (a, b) = arg(v, 1)?
                        .split_once('/')
                        .ok_or_else(|| anyhow::anyhow!("signature requires n/d"))?;
                    score.signature = [a.parse()?, b.parse()?];
                    ensure!(
                        (1..=32).contains(&score.signature[0])
                            && [1, 2, 4, 8, 16, 32].contains(&score.signature[1]),
                        "invalid signature"
                    );
                }
                "bars" => {
                    score.bars = arg(v, 1)?.parse()?;
                    ensure!((1..=4096).contains(&score.bars), "bars range 1..4096");
                }
                "rate" => {
                    score.rate = arg(v, 1)?.parse()?;
                    ensure!(
                        [16000, 22050, 24000, 44100, 48000, 88200, 96000].contains(&score.rate),
                        "unsupported sample rate"
                    );
                }
                "bits" => {
                    score.bits = arg(v, 1)?.parse()?;
                    ensure!([16, 24].contains(&score.bits), "bits must be 16 or 24");
                }
                "let" => {
                    vars.insert(arg(v, 1)?.into(), arg(v, 2)?.into());
                }
                "voice" => {
                    ensure!(
                        v.last().map(String::as_str) == Some("{"),
                        "voice requires block"
                    );
                    let name = arg(v, 1)?.to_owned();
                    let mut voice = Voice::default();
                    i += 1;
                    while i < rows.len() && rows[i][0] != "}" {
                        let r = &rows[i];
                        match r[0].as_str() {
                            "bank" => voice.bank = arg(r, 1)?.into(),
                            "register" => voice.register = arg(r, 1)?.into(),
                            "expression" => voice.style = arg(r, 1)?.into(),
                            "language" => {
                                voice.language = Some(crate::locale::normalize(arg(r, 1)?)?)
                            }
                            "inventory" => {
                                voice.inventory = Some(crate::locale::normalize(arg(r, 1)?)?)
                            }
                            "provider" => voice.provider = Some(arg(r, 1)?.into()),
                            _ => bail!("unknown voice directive {}", r[0]),
                        }
                        i += 1;
                    }
                    ensure!(i < rows.len(), "unclosed voice block");
                    ensure!(!voice.bank.is_empty(), "voice needs bank");
                    ensure!(
                        score.voices.insert(name, voice).is_none(),
                        "duplicate voice"
                    );
                }
                "section" => {
                    ensure!(section.is_none(), "sections cannot nest");
                    ensure!(
                        arg(v, 2)? == "at" && v.last().map(String::as_str) == Some("{"),
                        "section NAME at TIME {{ ... }}"
                    );
                    section_repeat = if v.get(4).is_some_and(|x| x == "repeat") {
                        ensure!(
                            arg(v, 6)? == "every",
                            "section NAME at TIME repeat COUNT every BEATSb"
                        );
                        let count: usize = arg(v, 5)?.parse()?;
                        let step =
                            musical(arg(v, 7)?)?.context("section repeat spacing needs Nb")?;
                        ensure!(
                            (1..=128).contains(&count) && step > 0.,
                            "section repeat range 1..128, positive spacing"
                        );
                        (count, step, score.tracks.len())
                    } else {
                        (1, 0., score.tracks.len())
                    };
                    section_beat = musical_at(arg(v, 3)?, &score.markers)?;
                    section = Some((
                        arg(v, 1)?.into(),
                        if let Some(beat) = section_beat {
                            beat * 60. / score.tempo
                        } else {
                            seconds(arg(v, 3)?, score.tempo)?
                        },
                    ));
                }
                "}" => {
                    let (name, _) = section.take().context("unexpected closing brace")?;
                    let (count, spacing, first) = section_repeat;
                    let originals = score.tracks[first..].to_vec();
                    let positions = track_positions[first..].to_vec();
                    ensure!(
                        score.tracks.len() + originals.len() * (count - 1) <= 128,
                        "section repetition exceeds track limit 128"
                    );
                    for repetition in 1..count {
                        for (track, position) in originals.iter().zip(&positions) {
                            let mut track = track.clone();
                            track.name = format!(
                                "{name}#{}/{}",
                                repetition + 1,
                                track
                                    .name
                                    .strip_prefix(&format!("{name}/"))
                                    .unwrap_or(&track.name)
                            );
                            score.tracks.push(track);
                            let (local, origin, seconds, shift) = *position;
                            track_positions.push((
                                local,
                                origin,
                                seconds,
                                shift + spacing * repetition as f64,
                            ));
                        }
                    }
                }
                "instrument" => {
                    ensure!(
                        v.last().map(String::as_str) == Some("{"),
                        "instrument requires block"
                    );
                    let name = arg(v, 1)?.to_owned();
                    let mut instrument = Track::new(name.clone());
                    i += 1;
                    while i < rows.len() && rows[i][0] != "}" {
                        parse_track(&rows[i], &mut instrument, &vars, &instruments, score.tempo)?;
                        i += 1;
                    }
                    ensure!(i < rows.len(), "unclosed instrument block");
                    ensure!(
                        instrument.sequence.is_empty() && instrument.voice.is_none(),
                        "instrument definitions contain sound settings, not notes or vocals"
                    );
                    ensure!(
                        instruments.insert(name, instrument).is_none(),
                        "duplicate instrument definition"
                    );
                }
                "track" => {
                    ensure!(score.tracks.len() < 128, "track limit 128");
                    ensure!(
                        v.last().map(String::as_str) == Some("{"),
                        "track requires block"
                    );
                    let mut t = Track::new(arg(v, 1)?.into());
                    let mut local_beat = None;
                    i += 1;
                    while i < rows.len() && rows[i][0] != "}" {
                        if rows[i][0] == "at" {
                            local_beat = musical(arg(&rows[i], 1)?)?;
                        }
                        parse_track(&rows[i], &mut t, &vars, &instruments, score.tempo)
                            .map_err(|e| anyhow::anyhow!("statement {}: {e}", i + 1))?;
                        i += 1;
                    }
                    ensure!(i < rows.len(), "unclosed track block");
                    if let Some((name, offset)) = &section {
                        t.name = format!("{name}/{}", t.name);
                        t.at += offset;
                    }
                    ensure!(
                        !score.tracks.iter().any(|x| x.name == t.name),
                        "duplicate track {}",
                        t.name
                    );
                    track_positions.push((
                        local_beat,
                        if section.is_some() {
                            section_beat
                        } else {
                            None
                        },
                        section.as_ref().map_or(0., |x| x.1),
                        0.,
                    ));
                    score.tracks.push(t);
                }
                "master" => score.master.push(effect(&v[1..])?),
                _ => bail!("unknown directive {}", v[0]),
            }
            Ok(())
        })();
        result.map_err(|e| anyhow::anyhow!("statement {}: {e}", i + 1))?;
        i += 1;
    }
    ensure!(section.is_none(), "unclosed section");
    let clock = score.clock()?;
    for (track, (local_beat, origin_beat, origin_seconds, repeat_shift)) in
        score.tracks.iter_mut().zip(track_positions)
    {
        let mapped = !score.tempo_map.is_empty() || !score.meter_map.is_empty();
        let origin = origin_beat.map_or(origin_seconds, |b| clock.seconds(b));
        let origin = clock.seconds(clock.beat(origin) + repeat_shift);
        let old_local = track.at - origin_seconds;
        if mapped || repeat_shift > 0. {
            track.at = local_beat.map_or(origin + old_local, |b| {
                clock.seconds(clock.beat(origin) + b)
            });
        }
        if let Some(grid) = track.snap_grid {
            track.at = clock.seconds((clock.beat(track.at) / grid).round() * grid);
        }
        let base = clock.beat(track.at);
        for clip in &mut track.clips {
            if let Some(beat) = clip.at_beat.filter(|_| mapped) {
                clip.at =
                    clock.seconds(base + beat) - track.at + (clip.at - beat * 60. / score.tempo);
            }
        }
        if let Some(grid) = track.snap_grid {
            for clip in &mut track.clips {
                let relative = clock.beat(track.at + clip.at) - base;
                clip.at = clock.seconds(base + (relative / grid).round() * grid) - track.at;
            }
        }
        for (index, a, b) in std::mem::take(&mut track.timeline_edits) {
            let position = |value: &str| -> Result<f64> {
                if value.starts_with('@') || value.starts_with("bar:") {
                    Ok(clock.parse(value, &score.markers)? - track.at)
                } else if let Some(beat) = musical(value)? {
                    Ok(clock.seconds(base + beat) - track.at)
                } else {
                    seconds(value, score.tempo)
                }
            };
            let start = position(&a)?;
            let end = b.as_deref().map(position).transpose()?;
            let clip = track
                .clips
                .get_mut(index)
                .context("timeline edit references a deleted clip")?;
            ensure!(start >= clip.at, "timeline edit lies before clip start");
            let source = clip.start + (start - clip.at) / clip.stretch;
            if let Some(end) = end {
                ensure!(end > start, "trim-at end must follow start");
                let source_end = clip.start + (end - clip.at) / clip.stretch;
                ensure!(
                    clip.end.is_none_or(|limit| source_end <= limit + 1e-9),
                    "trim-at exceeds clip end"
                );
                clip.start = source;
                clip.end = Some(source_end);
                clip.at = start;
                clip.at_beat = None;
            } else {
                ensure!(
                    source > clip.start && clip.end.is_none_or(|limit| source < limit),
                    "split-at must lie inside clip"
                );
                let mut right = clip.clone();
                right.id = format!("{}-right", clip.id);
                right.start = source;
                right.at = start;
                right.at_beat = None;
                right.fade_in = 0.;
                clip.end = Some(source);
                clip.fade_out = 0.;
                track.clips.push(right);
            }
        }
        for automation in &mut track.automation {
            for (point, beat) in automation.points.iter_mut().zip(&automation.musical) {
                if let Some(beat) = beat.filter(|_| mapped) {
                    point[0] = clock.seconds(base + beat) - track.at;
                }
            }
            ensure!(
                automation.points.windows(2).all(|w| w[0][0] < w[1][0]),
                "automation points conflict after tempo conversion"
            );
        }
    }
    ensure!(score.duration() <= 3600., "arrangement exceeds one hour");
    for t in &score.tracks {
        ensure!(
            !t.automation.iter().any(|a| a.parameter == "filter")
                || t.effects.iter().any(|e| e.kind == "filter"),
            "track {:?}: filter automation needs a track filter effect",
            t.name
        );
        for automation in &t.automation {
            if let Some(target) = automation.parameter.strip_prefix("effect.") {
                let (effect_index, parameter) = target
                    .split_once('.')
                    .context("effect automation uses effect.INDEX.ARG (one-based)")?;
                let effect_index = effect_index
                    .parse::<usize>()?
                    .checked_sub(1)
                    .context("effect index starts at 1")?;
                let parameter = parameter
                    .parse::<usize>()?
                    .checked_sub(1)
                    .context("argument index starts at 1")?;
                let effect_spec = t
                    .effects
                    .get(effect_index)
                    .context("unknown automated effect index")?;
                ensure!(
                    parameter < effect_spec.args.len()
                        && !(effect_spec.kind == "delay" && parameter == 0),
                    "unknown or allocation-changing effect argument (delay time cannot be automated)"
                );
                for point in &automation.points {
                    let mut args = effect_spec.args.clone();
                    args[parameter] = point[1] as f32;
                    let mut tokens = vec![effect_spec.kind.clone()];
                    tokens.extend(args.iter().map(ToString::to_string));
                    effect(&tokens)?;
                }
            }
        }
        if t.mini {
            crate::mini::Pattern::compile(&t.sequence)
                .map_err(|e| anyhow::anyhow!("track {:?}: {e:#}", t.name))?;
        } else {
            events(&t.sequence, t.step)?;
        }
        if let Some(v) = &t.voice {
            ensure!(score.voices.contains_key(v), "unknown voice {v}");
        }
    }
    Ok(score)
}
impl Score {
    pub fn bar_beats(&self) -> f64 {
        self.signature[0] as f64 * 4. / self.signature[1] as f64
    }
    pub fn clock(&self) -> Result<crate::timeline::Clock> {
        crate::timeline::Clock::new(self.tempo, self.signature, &self.tempo_map, &self.meter_map)
    }
    pub fn duration(&self) -> f64 {
        if self.tempo_map.is_empty() && self.meter_map.is_empty() {
            self.bars as f64 * self.bar_beats() * 60. / self.tempo
        } else {
            self.clock()
                .map(|c| c.seconds(c.bar_beat(self.bars + 1)))
                .unwrap_or(f64::NAN)
        }
    }
}
fn selected_clip(t: &mut Track) -> Result<&mut Clip> {
    let index = t.selected_clip.unwrap_or(t.clips.len().saturating_sub(1));
    t.clips.get_mut(index).context("import/select a clip first")
}
fn parse_track(
    v: &[String],
    t: &mut Track,
    vars: &BTreeMap<String, String>,
    instruments: &BTreeMap<String, Track>,
    tempo: f64,
) -> Result<()> {
    if [
        "gain",
        "pan",
        "vibrato",
        "breathiness",
        "expression",
        "formant",
        "dynamics",
        "vocal-pitch",
        "harmony",
    ]
    .contains(&v[0].as_str())
    {
        t.vocal_overrides.insert(v[0].clone());
    }
    match v[0].as_str() {
        "notation" => {
            t.mini = match arg(v, 1)? {
                "mini" => true,
                "legacy" => false,
                _ => bail!("notation mini|legacy"),
            };
        }
        "cycle" => {
            let n: f64 = arg(v, 1)?.parse()?;
            ensure!(
                n.is_finite() && (0.0625..=256.).contains(&n),
                "cycle range 1/16..256 beats"
            );
            t.cycle_beats = n;
        }
        "cycles" => {
            t.cycles = arg(v, 1)?.parse()?;
            ensure!((1..=65536).contains(&t.cycles), "cycles range 1..65536");
        }
        "seed" => t.seed = arg(v, 1)?.parse()?,
        "language" => t.language = Some(crate::locale::normalize(arg(v, 1)?)?),
        "inventory" => t.inventory = Some(crate::locale::normalize(arg(v, 1)?)?),
        "provider" => t.provider = Some(arg(v, 1)?.into()),
        "vocal-preset" => t.vocal_preset = Some(arg(v, 1)?.into()),
        "formant" => {
            let n = number(v, 1)?;
            ensure!((0.5..=2.).contains(&n), "formant ratio .5..2");
            t.formant = n;
        }
        "dynamics" => {
            let n = number(v, 1)?;
            ensure!((0.0..=2.).contains(&n), "dynamics range 0..2");
            t.dynamics = n;
        }
        "vocal-pitch" => {
            let n = number(v, 1)?;
            ensure!(n.abs() <= 24., "vocal-pitch range -24..24 semitones");
            t.vocal_pitch = n;
        }
        "harmony" => {
            let h = arg(v, 1)?
                .split_whitespace()
                .map(str::parse)
                .collect::<std::result::Result<Vec<i32>, _>>()?;
            ensure!(
                !h.is_empty() && h.len() <= 8 && h.iter().all(|n| (-24..=24).contains(n)),
                "harmony needs 1..8 semitone offsets in -24..24"
            );
            t.harmonies = h;
        }
        "synth" => t.instrument = arg(v, 1)?.into(),
        "instrument" => {
            let name = arg(v, 1)?;
            if let Some(base) = instruments.get(name) {
                let track_name = t.name.clone();
                *t = base.clone();
                t.name = track_name;
            } else {
                t.instrument = name.into();
            }
        }
        "drum" => {
            let name = arg(v, 1)?;
            ensure!(
                !name.is_empty()
                    && name.len() <= 128
                    && name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-'),
                "drum needs a local sample identifier"
            );
            t.drums.insert(name.into(), arg(v, 2)?.into());
        }
        "sample-root" => t.sample_root = note(arg(v, 1)?)?,
        "pitch-envelope" => {
            let amount = number(v, 1)?;
            let decay = number(v, 2)?;
            ensure!(
                amount.abs() <= 48. && decay > 0. && decay <= 10.,
                "pitch-envelope semitones [-48,48], decay seconds (0,10]"
            );
            t.pitch_envelope = [amount, decay];
        }
        "notes" | "pattern" => {
            let s = arg(v, 1)?;
            t.sequence = if let Some(k) = s.strip_prefix('$') {
                vars.get(k)
                    .ok_or_else(|| anyhow::anyhow!("unknown definition {k}"))?
                    .clone()
            } else {
                s.into()
            };
        }
        "step" => {
            t.step = number(v, 1)? as f64;
            ensure!(t.step > 0. && t.step <= 64., "step range (0,64]");
        }
        "gain" => t.gain = gain(arg(v, 1)?)?,
        "pan" => {
            t.pan = number(v, 1)?;
            ensure!(t.pan.abs() <= 1., "pan range -1..1");
        }
        "mute" => t.mute = true,
        "solo" => t.solo = true,
        "loop" => t.looping = true,
        "at" => t.at = seconds(arg(v, 1)?, tempo)?,
        "transpose" => {
            t.transpose = arg(v, 1)?.parse()?;
            ensure!((-48..=48).contains(&t.transpose), "transpose range -48..48");
        }
        "retrograde" => t.reverse = true,
        "repeat" => {
            let n: usize = arg(v, 1)?.parse()?;
            ensure!(
                (1..=128).contains(&n) && t.sequence.len() * n < 65536,
                "repeat exceeds limits"
            );
            t.sequence = std::iter::repeat_n(t.sequence.clone(), n)
                .collect::<Vec<_>>()
                .join(" ");
        }
        "import" | "sample" => {
            ensure!(t.clips.len() < 1024, "clip limit");
            let mut clip = Clip::new(arg(v, 1)?.into());
            clip.id = format!("clip-{}", t.clips.len() + 1);
            t.clips.push(clip);
            t.selected_clip = Some(t.clips.len() - 1);
            if v[0] == "sample" {
                t.instrument = "sample".into();
            }
        }
        "split-at" | "trim-at" => {
            let index = t.selected_clip.unwrap_or(t.clips.len().saturating_sub(1));
            ensure!(index < t.clips.len(), "select a clip first");
            t.timeline_edits.push((
                index,
                arg(v, 1)?.into(),
                if v[0] == "trim-at" {
                    Some(arg(v, 2)?.into())
                } else {
                    None
                },
            ));
        }
        "group-duplicate" => {
            let name = arg(v, 1)?;
            let shift = seconds(arg(v, 2)?, tempo)?;
            let copies: Vec<_> = t
                .clips
                .iter()
                .filter(|c| c.group == name)
                .cloned()
                .collect();
            ensure!(
                !copies.is_empty() && copies.len() + t.clips.len() <= 1024,
                "unknown group or clip limit exceeded"
            );
            for mut clip in copies {
                clip.id = format!("{}-copy-{}", clip.id, t.clips.len() + 1);
                clip.at += shift;
                t.clips.push(clip);
            }
        }
        "snap" => {
            t.snap_grid = if arg(v, 1)? == "off" {
                None
            } else {
                let grid = musical(arg(v, 1)?)?.context("snap GRIDb|off")?;
                ensure!(grid > 0. && grid <= 64., "snap range (0,64] beats");
                Some(grid)
            };
        }
        "select-clip" => {
            let id = arg(v, 1)?;
            t.selected_clip = Some(
                t.clips
                    .iter()
                    .position(|c| c.id == id)
                    .or_else(|| {
                        id.parse::<usize>()
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .filter(|i| *i < t.clips.len())
                    })
                    .context("unknown clip ID/index")?,
            );
        }
        "clip-id" => {
            let id = arg(v, 1)?;
            ensure!(
                !id.is_empty() && !t.clips.iter().any(|c| c.id == id),
                "duplicate/empty clip ID"
            );
            selected_clip(t)?.id = id.into();
        }
        "relink" => selected_clip(t)?.path = arg(v, 1)?.into(),
        "group-move" => {
            let name = arg(v, 1)?;
            let offset = seconds(arg(v, 2)?, tempo)?;
            ensure!(
                t.clips.iter().any(|c| c.group == name),
                "unknown clip group"
            );
            for clip in t.clips.iter_mut().filter(|c| c.group == name) {
                clip.at += offset;
            }
        }
        "trim" | "crop" => {
            let c = selected_clip(t)?;
            c.start = seconds(arg(v, 1)?, tempo)?;
            c.end = Some(seconds(arg(v, 2)?, tempo)?);
            ensure!(c.end.unwrap() > c.start, "trim end must follow start");
        }
        "place" | "move" => {
            let c = selected_clip(t)?;
            let value = arg(v, if arg(v, 1)? == "at" { 2 } else { 1 })?;
            c.at = seconds(value, tempo)?;
            c.at_beat = musical(value)?;
        }
        "clip-gain" => selected_clip(t)?.gain = gain(arg(v, 1)?)?,
        "clip-pan" => {
            let p = number(v, 1)?;
            ensure!(p.abs() <= 1., "pan range -1..1");
            selected_clip(t)?.pan = p;
        }
        "fade" => {
            let c = selected_clip(t)?;
            let time = seconds(arg(v, 2)?, tempo)?;
            match arg(v, 1)? {
                "in" => c.fade_in = time,
                "out" => c.fade_out = time,
                _ => bail!("fade in|out TIME"),
            }
        }
        "reverse" | "normalize" => {
            let c = selected_clip(t)?;
            if v[0] == "reverse" {
                c.reverse = !c.reverse
            } else {
                c.normalize = true
            }
        }
        "stretch" | "pitch" => {
            let value = number(v, 1)?;
            let c = selected_clip(t)?;
            if v[0] == "stretch" {
                ensure!((0.25..=4.).contains(&value), "stretch range .25..4");
                c.stretch = value as f64
            } else {
                ensure!(value.abs() <= 24., "pitch range -24..24");
                c.pitch = value;
            }
        }
        "group" => selected_clip(t)?.group = arg(v, 1)?.into(),
        "duplicate" => {
            ensure!(t.clips.len() < 1024, "clip limit");
            let mut c = selected_clip(t)?.clone();
            c.id = format!("clip-{}", t.clips.len() + 1);
            c.at = seconds(arg(v, 1)?, tempo)?;
            c.at_beat = musical(arg(v, 1)?)?;
            t.clips.push(c);
            t.selected_clip = Some(t.clips.len() - 1);
        }
        "delete" => {
            let i = t.selected_clip.unwrap_or(t.clips.len().saturating_sub(1));
            ensure!(i < t.clips.len(), "no selected clip");
            t.clips.remove(i);
            t.selected_clip = None;
        }
        "split" => {
            let time = seconds(arg(v, 1)?, tempo)?;
            let c = selected_clip(t)?;
            ensure!(
                time > c.start && c.end.is_some_and(|end| time < end),
                "split needs explicit trim containing time"
            );
            let mut right = c.clone();
            right.id = format!("{}-split", c.id);
            right.start = time;
            right.at += (time - c.start) * c.stretch;
            c.end = Some(time);
            t.clips.push(right);
        }
        "crossfade" => {
            ensure!(t.clips.len() >= 2, "crossfade needs two clips");
            let d = seconds(arg(v, 1)?, tempo)?;
            let n = t.clips.len();
            let left = &mut t.clips[n - 2];
            let end = left
                .end
                .ok_or_else(|| anyhow::anyhow!("crossfade requires explicit trim"))?;
            ensure!(
                d <= (end - left.start) * left.stretch,
                "crossfade exceeds clip"
            );
            left.fade_out = d;
            let at = left.at + (end - left.start) * left.stretch - d;
            t.clips[n - 1].at = at;
            t.clips[n - 1].fade_in = d;
        }
        "use" => {
            ensure!(arg(v, 1)? == "voice", "use voice NAME");
            t.voice = Some(arg(v, 2)?.into());
        }
        "lyrics" => t.lyrics = arg(v, 1)?.into(),
        "expression" => t.expression = arg(v, 1)?.into(),
        "vibrato" | "breathiness" => {
            let n = number(v, 1)?;
            ensure!((0.0..=1.).contains(&n), "expression range 0..1");
            if v[0] == "vibrato" {
                t.vibrato = n
            } else {
                t.breathiness = n
            }
        }
        "adsr" => {
            t.attack = number(v, 1)?;
            t.decay = number(v, 2)?;
            t.sustain = number(v, 3)?;
            t.release = number(v, 4)?;
            ensure!(
                [t.attack, t.decay, t.release]
                    .iter()
                    .all(|v| *v >= 0. && *v <= 10.)
                    && (0.0..=1.).contains(&t.sustain),
                "invalid ADSR"
            );
        }
        "automate" => {
            let p = arg(v, 1)?;
            ensure!(
                [
                    "gain",
                    "pan",
                    "bend",
                    "filter",
                    "formant",
                    "dynamics",
                    "vibrato",
                    "breathiness",
                    "vocal-pitch"
                ]
                .contains(&p)
                    || p.starts_with("effect."),
                "automate gain|pan|bend|filter"
            );
            let mut points = vec![];
            let mut musical_points = vec![];
            for point in arg(v, 2)?.split_whitespace() {
                let (a, b) = point
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("automation uses TIME=VALUE"))?;
                let time = seconds(a, tempo)?;
                let val: f64 = b.parse()?;
                ensure!(
                    val.is_finite()
                        && match p {
                            "gain" => (0.0..=4.).contains(&val),
                            "pan" => (-1.0..=1.).contains(&val),
                            "filter" => (20.0..=20000.).contains(&val),
                            "formant" => (0.5..=2.).contains(&val),
                            "dynamics" => (0. ..=2.).contains(&val),
                            "vibrato" | "breathiness" => (0. ..=1.).contains(&val),
                            value if value.starts_with("effect.") => true,
                            _ => (-24.0..=24.).contains(&val),
                        },
                    "invalid automation value"
                );

                points.push([time, val]);
                musical_points.push(musical(a)?);
            }
            ensure!(!points.is_empty(), "empty automation");
            let interpolation = v.get(3).map(String::as_str).unwrap_or("linear");
            ensure!(
                ["linear", "step"].contains(&interpolation),
                "automation interpolation linear|step"
            );
            t.automation.push(Automation {
                step: interpolation == "step",
                parameter: p.into(),
                points,
                musical: musical_points,
            });
        }
        "clip-effect" => selected_clip(t)?.effects.push(effect(&v[1..])?),
        _ => t.effects.push(effect(v)?),
    }
    Ok(())
}

pub fn events(src: &str, step: f64) -> Result<Vec<Event>> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut budget = 0;
    fn seq(
        c: &[char],
        i: &mut usize,
        step: f64,
        depth: usize,
        budget: &mut usize,
    ) -> Result<Vec<Event>> {
        ensure!(depth < 16, "pattern nesting limit");
        let mut out = vec![];
        let mut beat = 0.;
        while *i < c.len() {
            while *i < c.len() && c[*i].is_whitespace() {
                *i += 1;
            }
            if *i == c.len() {
                break;
            }
            if c[*i] == ')' {
                ensure!(depth > 0, "unexpected )");
                *i += 1;
                return Ok(out);
            }
            let mut part = if c[*i] == '(' {
                *i += 1;
                seq(c, i, step, depth + 1, budget)?
            } else {
                let start = *i;
                let mut chord = false;
                while *i < c.len() {
                    match c[*i] {
                        '[' => chord = true,
                        ']' => chord = false,
                        x if !chord && (x.is_whitespace() || x == '*' || x == ')') => break,
                        _ => {}
                    }
                    *i += 1;
                }
                ensure!(!chord, "unclosed chord");
                let word: String = c[start..*i].iter().collect();
                let (token, duration) = if let Some((a, b)) = word.rsplit_once(':') {
                    let d: f64 = b.parse()?;
                    ensure!(d.is_finite() && d > 0. && d <= 64., "invalid note duration");
                    (a, d)
                } else {
                    (word.as_str(), step)
                };
                let (notes, drum) = if token == "-" {
                    (vec![], None)
                } else if ["kick", "snare", "hat", "clap"].contains(&token) {
                    (vec![], Some(token.into()))
                } else if token.starts_with('[') && token.ends_with(']') {
                    let notes = token[1..token.len() - 1]
                        .split(',')
                        .map(note)
                        .collect::<Result<Vec<_>>>()?;
                    ensure!(!notes.is_empty() && notes.len() <= 32, "chord limit 32");
                    (notes, None)
                } else {
                    (vec![note(token)?], None)
                };
                vec![Event {
                    beat: 0.,
                    duration,
                    notes,
                    drum,
                }]
            };
            let mut repeats = 1usize;
            if *i < c.len() && c[*i] == '*' {
                *i += 1;
                let start = *i;
                while *i < c.len() && c[*i].is_ascii_digit() {
                    *i += 1;
                }
                repeats = c[start..*i].iter().collect::<String>().parse()?;
                ensure!((1..=128).contains(&repeats), "repetition range 1..128");
            }
            let len = part.last().map_or(0., |e| e.beat + e.duration);
            for _ in 0..repeats {
                for e in &mut part {
                    *budget += 1;
                    ensure!(*budget <= 65536, "event limit 65536");
                    let mut e = e.clone();
                    e.beat += beat;
                    out.push(e);
                }
                beat += len;
            }
        }
        ensure!(depth == 0, "unclosed sequence");
        Ok(out)
    }
    seq(&chars, &mut i, step, 0, &mut budget)
}

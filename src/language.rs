use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
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
    pub gain: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub looping: bool,
    pub at: f64,
    pub clips: Vec<Clip>,
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
            gain: 1.,
            pan: 0.,
            mute: false,
            solo: false,
            looping: false,
            at: 0.,
            clips: vec![],
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
    pub path: String,
    pub at: f64,
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
            path,
            at: 0.,
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
    pub parameter: String,
    pub points: Vec<[f64; 2]>,
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
    let mut i = 0;
    while i < rows.len() {
        let v = &rows[i];
        let result = (|| -> Result<()> {
            match v[0].as_str() {
                "language" => score.language = Some(crate::locale::normalize(arg(v, 1)?)?),
                "inventory" => score.inventory = Some(crate::locale::normalize(arg(v, 1)?)?),
                "provider" => score.provider = Some(arg(v, 1)?.into()),
                "project" => score.name = arg(v, 1)?.into(),
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
                    section = Some((arg(v, 1)?.into(), seconds(arg(v, 3)?, score.tempo)?));
                }
                "}" => {
                    ensure!(section.take().is_some(), "unexpected closing brace");
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
                    i += 1;
                    while i < rows.len() && rows[i][0] != "}" {
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
    ensure!(score.duration() <= 3600., "arrangement exceeds one hour");
    for t in &score.tracks {
        events(&t.sequence, t.step)?;
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
    pub fn duration(&self) -> f64 {
        self.bars as f64 * self.bar_beats() * 60. / self.tempo
    }
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
                ["kick", "snare", "hat", "clap"].contains(&name),
                "drum name kick|snare|hat|clap"
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
            t.clips.push(Clip::new(arg(v, 1)?.into()));
            if v[0] == "sample" {
                t.instrument = "sample".into();
            }
        }
        "trim" | "crop" => {
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("import a clip first"))?;
            c.start = seconds(arg(v, 1)?, tempo)?;
            c.end = Some(seconds(arg(v, 2)?, tempo)?);
            ensure!(c.end.unwrap() > c.start, "trim end must follow start");
        }
        "place" | "move" => {
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("import a clip first"))?;
            c.at = seconds(arg(v, if arg(v, 1)? == "at" { 2 } else { 1 })?, tempo)?;
        }
        "clip-gain" => {
            t.clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?
                .gain = gain(arg(v, 1)?)?
        }
        "clip-pan" => {
            let p = number(v, 1)?;
            ensure!(p.abs() <= 1., "pan range -1..1");
            t.clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?
                .pan = p;
        }
        "fade" => {
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?;
            let time = seconds(arg(v, 2)?, tempo)?;
            match arg(v, 1)? {
                "in" => c.fade_in = time,
                "out" => c.fade_out = time,
                _ => bail!("fade in|out TIME"),
            }
        }
        "reverse" | "normalize" => {
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?;
            if v[0] == "reverse" {
                c.reverse = !c.reverse
            } else {
                c.normalize = true
            }
        }
        "stretch" | "pitch" => {
            let value = number(v, 1)?;
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?;
            if v[0] == "stretch" {
                ensure!((0.25..=4.).contains(&value), "stretch range .25..4");
                c.stretch = value as f64
            } else {
                ensure!(value.abs() <= 24., "pitch range -24..24");
                c.pitch = value;
            }
        }
        "group" => {
            t.clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?
                .group = arg(v, 1)?.into()
        }
        "duplicate" => {
            ensure!(t.clips.len() < 1024, "clip limit");
            let mut c = t
                .clips
                .last()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?
                .clone();
            c.at = seconds(arg(v, 1)?, tempo)?;
            t.clips.push(c);
        }
        "delete" => {
            ensure!(t.clips.pop().is_some(), "no clip");
        }
        "split" => {
            let time = seconds(arg(v, 1)?, tempo)?;
            let c = t
                .clips
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("no clip"))?;
            ensure!(
                time > c.start && c.end.is_some_and(|end| time < end),
                "split needs explicit trim containing time"
            );
            let mut right = c.clone();
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
                ["gain", "pan", "bend"].contains(&p),
                "automate gain|pan|bend"
            );
            let mut points = vec![];
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
                            _ => (-24.0..=24.).contains(&val),
                        },
                    "invalid automation value"
                );
                ensure!(
                    points.last().is_none_or(|p: &[f64; 2]| time > p[0]),
                    "automation times must increase"
                );
                points.push([time, val]);
            }
            ensure!(!points.is_empty(), "empty automation");
            t.automation.push(Automation {
                parameter: p.into(),
                points,
            });
        }
        "clip-effect" => t
            .clips
            .last_mut()
            .ok_or_else(|| anyhow::anyhow!("no clip"))?
            .effects
            .push(effect(&v[1..])?),
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

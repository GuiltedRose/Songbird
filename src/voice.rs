use crate::{
    codec::{Asset, safe_path},
    dsp,
    language::{Event, Track, Voice},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bank {
    pub version: u32,
    pub name: String,
    pub license: String,
    pub samples: Vec<Phoneme>,
    #[serde(default)]
    pub dictionary: BTreeMap<String, String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub inventory: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub generation: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phoneme {
    pub phoneme: String,
    pub file: String,
    pub root: f64,
    pub low: i32,
    pub high: i32,
    pub onset: f64,
    pub sustain: f64,
    #[serde(default)]
    pub release: Option<f64>,
    pub end: f64,
    pub voiced: bool,
    #[serde(default = "normal")]
    pub style: String,
    #[serde(default)]
    pub register: String,
    #[serde(default = "one")]
    pub gain: f32,
}
fn one() -> f32 {
    1.
}
fn normal() -> String {
    "normal".into()
}
pub struct LoadedBank {
    pub meta: Bank,
    pub audio: Vec<Asset>,
    pub profile: crate::phonetics::ContextProfile,
}
pub(crate) fn validate_sample(p: &Phoneme, a: &Asset) -> Result<()> {
    ensure!(
        crate::phonetics::lexical_label(&p.phoneme),
        "invalid phoneme identifier or compound label"
    );
    ensure!(
        p.root.is_finite()
            && (40.0..=2000.).contains(&p.root)
            && p.low >= 0
            && p.high <= 127
            && p.low <= p.high,
        "invalid pitch mapping"
    );
    ensure!(
        p.onset.is_finite()
            && p.sustain.is_finite()
            && p.end.is_finite()
            && p.onset >= 0.
            && p.sustain >= p.onset
            && p.end > p.sustain
            && p.end <= a.duration(),
        "invalid timings for {}",
        p.phoneme
    );
    ensure!(
        p.release
            .is_none_or(|r| r.is_finite() && r >= p.sustain && r <= p.end),
        "invalid release timing for {}",
        p.phoneme
    );
    ensure!(
        p.gain.is_finite() && (0.0..=16.).contains(&p.gain),
        "sample gain range 0..16"
    );
    if p.voiced {
        ensure!(
            p.release.unwrap_or(p.end) - p.sustain >= 3. / p.root,
            "voiced sustain needs at least three periods"
        );
    }
    Ok(())
}
impl LoadedBank {
    pub fn load(root: &Path, voice: &Voice, cache: &Path) -> Result<Self> {
        let manifest = safe_path(root, &format!("{}/voicebank.toml", voice.bank))?;
        ensure!(
            std::fs::metadata(&manifest)?.len() <= 1_048_576,
            "voicebank manifest exceeds 1 MiB"
        );
        let meta: Bank = toml::from_str(&std::fs::read_to_string(&manifest)?)?;
        ensure!(
            meta.version == 1 && !meta.name.is_empty() && !meta.license.is_empty(),
            "invalid voicebank metadata"
        );
        ensure!(
            !meta.samples.is_empty() && meta.samples.len() <= 2048,
            "voicebank needs 1..2048 samples"
        );
        let bank_root = manifest.parent().unwrap();
        let profile = crate::phonetics::bank_context(bank_root, &meta)?;
        let mut meta = meta;
        for p in &mut meta.samples {
            p.phoneme = profile.inventory.label(&p.phoneme)?;
        }
        let mut audio = vec![];
        for p in &meta.samples {
            let a = Asset::decode(&safe_path(bank_root, &p.file)?, cache)
                .with_context(|| format!("phoneme {}", p.phoneme))?;
            validate_sample(p, &a)?;
            audio.push(a);
        }
        Ok(Self {
            meta,
            audio,
            profile,
        })
    }
    pub(crate) fn choose(
        &self,
        phone: &str,
        note: f64,
        voice: &Voice,
        track: &Track,
    ) -> Result<usize> {
        self.meta
            .samples
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.phoneme == phone
                    && note >= p.low as f64
                    && note <= p.high as f64
                    && (voice.register.is_empty() || p.register == voice.register)
                    && p.style == track.expression
            })
            .min_by(|(_, a), (_, b)| {
                (dsp::hz(note) / a.root)
                    .ln()
                    .abs()
                    .total_cmp(&(dsp::hz(note) / b.root).ln().abs())
            })
            .map(|(i, _)| i)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "voicebank has no {phone} sample for note {note}, register {}, expression {}",
                    voice.register,
                    track.expression
                )
            })
    }
}
#[derive(Clone, Debug)]
pub struct VocalUnit {
    pub sample: usize,
    pub start: f64,
    pub end: f64,
    pub midi: i32,
    pub vowel: bool,
    pub phase: f64,
    pub last_time: f64,
    pub sustain_only: bool,
}
pub fn phonemes(lyrics: &str, dictionary: &BTreeMap<String, String>) -> Result<Vec<Vec<String>>> {
    let mut result = vec![];
    for word in lyrics.split_whitespace() {
        if word == "~" {
            result.push(vec!["~".into()]);
            continue;
        }
        if let Some(manual) = word.strip_prefix('@') {
            ensure!(!manual.is_empty(), "empty phoneme override");
            result.push(manual.split('.').map(str::to_owned).collect());
            continue;
        }
        let word = word
            .trim_matches(|c: char| c.is_ascii_punctuation())
            .to_ascii_uppercase();
        let automatic = english(&word);
        let pron = dictionary
            .get(&word)
            .map(String::as_str)
            .or_else(|| builtin(&word))
            .or(automatic.as_deref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown pronunciation {word}; add dictionary entry or @ARPABET.PHONES override"
                )
            })?;
        for syllable in pron.split('/') {
            let phones = syllable
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            ensure!(!phones.is_empty(), "empty dictionary syllable");
            result.push(phones);
        }
    }
    Ok(result)
}
fn builtin(word: &str) -> Option<&'static str> {
    Some(match word {
        "TAKE" => "T EY K",
        "ME" => "M IY",
        "BEYOND" => "B IH / AA N D",
        "THE" => "DH AH",
        "STARS" => "S T AA R Z",
        "HELLO" => "HH AH / L OW",
        "WORLD" => "W ER L D",
        "I" => "AY",
        "YOU" => "Y UW",
        "LOVE" => "L AH V",
        "A" => "AH",
        "SING" => "S IH NG",
        "SONG" => "S AO NG",
        "NIGHT" => "N AY T",
        "LIGHT" => "L AY T",
        "WE" => "W IY",
        "ARE" => "AA R",
        "DREAM" => "D R IY M",
        "IN" => "IH N",
        "MY" => "M AY",
        "HEART" => "HH AA R T",
        _ => return None,
    })
}
pub fn vowel(p: &str) -> bool {
    [
        "AA", "AE", "AH", "AO", "AW", "AY", "EH", "ER", "EY", "IH", "IY", "OW", "OY", "UH", "UW",
    ]
    .contains(&p)
}
pub fn align(
    bank: &LoadedBank,
    voice: &Voice,
    track: &Track,
    events: &[Event],
    tempo: f64,
) -> Result<Vec<VocalUnit>> {
    let syllables = bank
        .profile
        .phonemes(&track.lyrics, &bank.meta.dictionary)?;
    let notes = events
        .iter()
        .filter(|e| !e.notes.is_empty())
        .collect::<Vec<_>>();
    ensure!(
        syllables.len() == notes.len(),
        "{} syllables (including ~) need {} notes; provide one note per syllable, ~ for melisma",
        syllables.len(),
        notes.len()
    );
    let mut units = vec![];
    let mut last_vowel: Option<String> = None;
    let mut last_source: Option<String> = None;
    for (phones, event) in syllables.iter().zip(notes) {
        ensure!(event.notes.len() == 1, "singer notes must be monophonic");
        let midi = event.notes[0] + track.transpose;
        let start = event.beat * 60. / tempo;
        let duration = event.duration * 60. / tempo;
        if phones == &["~"] {
            let phone = last_vowel
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("melisma needs a preceding vowel"))?;
            units.push(VocalUnit {
                sample: bank
                    .choose(phone, midi as f64 + track.vocal_pitch as f64, voice, track)
                    .or_else(|error| match &last_source {
                        Some(label) => {
                            bank.choose(label, midi as f64 + track.vocal_pitch as f64, voice, track)
                        }
                        None => Err(error),
                    })?,
                start: (start - (bank.profile.pronunciation.timing.crossfade_ms / 1000.)).max(0.),
                end: start + duration + (bank.profile.pronunciation.timing.crossfade_ms / 1000.),
                midi,
                vowel: true,
                phase: 0.,
                last_time: -1.,
                sustain_only: true,
            });
            continue;
        }
        ensure!(
            phones.iter().any(|p| bank.profile.inventory.nucleus(p))
                || bank.profile.pronunciation.timing.allow_consonant_syllables,
            "syllable needs a nucleus in the selected inventory"
        );
        let combined = phones.join(".");
        if phones.len() > 1
            && let Ok(sample) = bank.choose(
                &combined,
                midi as f64 + track.vocal_pitch as f64,
                voice,
                track,
            )
        {
            last_vowel = phones
                .iter()
                .rev()
                .find(|p| bank.profile.inventory.nucleus(p))
                .cloned();
            last_source = Some(combined.clone());
            units.push(VocalUnit {
                sample,
                start,
                end: start + duration,
                midi,
                vowel: true,
                phase: 0.,
                last_time: -1.,
                sustain_only: false,
            });
            continue;
        }
        let consonants = phones
            .iter()
            .filter(|p| !bank.profile.inventory.nucleus(p))
            .count();
        let consonant = (duration * bank.profile.pronunciation.timing.consonant_fraction
            / (consonants.max(1) as f64))
            .min(bank.profile.pronunciation.timing.consonant_max);
        let vowels = phones.len() - consonants;
        let vowel_len = (duration - consonant * consonants as f64) / vowels.max(1) as f64;
        let mut at = start;
        for phone in phones {
            let is_vowel = bank.profile.inventory.nucleus(phone)
                || (vowels == 0
                    && bank
                        .profile
                        .inventory
                        .phone(phone)
                        .is_some_and(|p| p.voiced));
            let len = if vowels == 0 {
                duration / phones.len() as f64
            } else if is_vowel {
                vowel_len
            } else {
                consonant
            };
            let index = bank.choose(phone, midi as f64 + track.vocal_pitch as f64, voice, track)?;
            if is_vowel {
                last_vowel = Some(phone.clone());
                last_source = Some(phone.clone());
            }
            units.push(VocalUnit {
                sample: index,
                start: (at - (bank.profile.pronunciation.timing.crossfade_ms / 1000.)).max(0.),
                end: at + len + (bank.profile.pronunciation.timing.crossfade_ms / 1000.),
                midi,
                vowel: is_vowel,
                phase: 0.,
                last_time: -1.,
                sustain_only: false,
            });
            at += len;
        }
    }
    Ok(units)
}
// Time-domain pitch-synchronous overlap/add. Grain shape is retained when target pitch changes.
// Root metadata sets the analysis period; no resampling of the vocal tract envelope.
pub fn sample(
    bank: &mut LoadedBank,
    unit: &mut VocalUnit,
    time: f64,
    track: &Track,
    bend: f32,
    output_rate: u32,
) -> Result<[f32; 2]> {
    let p = &bank.meta.samples[unit.sample];
    let a = &mut bank.audio[unit.sample];
    let t = time - unit.start;
    let duration = unit.end - unit.start;
    let fade = (bank.profile.pronunciation.timing.crossfade_ms / 500.).max(1e-8);
    let edge = (t / fade).min((duration - t) / fade).clamp(0., 1.) as f32;
    let mut out = if p.voiced && unit.vowel {
        let target = dsp::hz(
            unit.midi as f64
                + bend as f64
                + track.vocal_pitch as f64
                + track.vibrato as f64 * 0.5 * (std::f64::consts::TAU * 5.5 * t).sin(),
        );
        let target_period = 1. / target;
        let source_period = 1. / p.root;
        if unit.last_time < 0.
            || time < unit.last_time
            || time - unit.last_time > 2. / output_rate as f64
        {
            unit.phase = t * target;
        } else {
            unit.phase += (time - unit.last_time) * target;
        }
        unit.last_time = time;
        let grain = unit.phase.round() as i64;
        let mut sum = [0.; 2];
        let mut weight = 0.;
        let grains = (target / (p.root * track.formant as f64))
            .ceil()
            .clamp(2., 64.) as i64
            + 1;
        for g in grain - grains..=grain + grains {
            let center = t + (g as f64 - unit.phase) * target_period;
            let offset = t - center;
            if offset.abs() * track.formant as f64 >= source_period {
                continue;
            }
            let progress = (center / duration).clamp(0., 1.);
            let natural = p.sustain
                + progress * (p.release.unwrap_or(p.end) - p.sustain - 2. * source_period).max(0.);
            let source_center =
                p.sustain + ((natural - p.sustain) / source_period).round() * source_period;
            let frame = a.sample(
                (source_center + offset * track.formant as f64) * a.rate as f64,
                track.formant as f64,
            )?;
            let w = (0.5
                + 0.5
                    * (std::f64::consts::PI * offset * track.formant as f64 / source_period).cos())
                as f32;
            sum[0] += frame[0] * w;
            sum[1] += frame[1] * w;
            weight += w;
        }
        if weight > 1e-6 {
            sum[0] /= weight;
            sum[1] /= weight;
        }
        let attack = if unit.sustain_only {
            0.
        } else {
            (p.sustain - p.onset).min(duration * 0.25)
        };
        let tail = if unit.sustain_only {
            0.
        } else {
            (p.end - p.release.unwrap_or(p.end)).min(duration * 0.25)
        };
        if attack > 0. && t < attack {
            let raw = a.sample(
                (p.onset + t * (p.sustain - p.onset) / attack) * a.rate as f64,
                1.,
            )?;
            let blend = (t / attack).powi(4) as f32;
            sum = [
                raw[0] * (1. - blend) + sum[0] * blend,
                raw[1] * (1. - blend) + sum[1] * blend,
            ];
        }
        if tail > 0. && t > duration - tail {
            let f = ((t - duration + tail) / tail).clamp(0., 1.);
            let at = p.release.unwrap_or(p.end) + f * (p.end - p.release.unwrap_or(p.end));
            let raw = a.sample(at * a.rate as f64, 1.)?;
            let blend = f as f32;
            sum = [
                sum[0] * (1. - blend) + raw[0] * blend,
                sum[1] * (1. - blend) + raw[1] * blend,
            ];
        }
        sum
    } else {
        // Preserve a consonant's local spectrum: do not squeeze an entire long
        // fricative or nasal take into a few milliseconds of a syllable.
        let speed = if p.voiced {
            dsp::hz(unit.midi as f64 + bend as f64 + track.vocal_pitch as f64) / p.root
        } else {
            1.
        };
        let length = p.end - p.onset;
        let natural = t * speed;
        let position = if natural < length {
            p.onset + natural
        } else if bank
            .profile
            .inventory
            .phone(&p.phoneme)
            .is_some_and(|p| p.stop)
        {
            return Ok([0.; 2]);
        } else {
            let from = p.sustain;
            let to = p.release.unwrap_or(p.end);
            let span = (to - from).max(1. / a.rate as f64);
            from + ((natural - length) % span)
        };
        a.sample(position * a.rate as f64, speed)?
    };
    let breath = dsp::noise((time * 48000.) as u64, 945) * track.breathiness * 0.025;
    for x in &mut out {
        *x = (*x + breath) * edge * track.dynamics * p.gain;
    }
    Ok(out)
}
pub fn write_template(path: &Path, name: &str) -> Result<()> {
    ensure!(!path.exists(), "voicebank already exists");
    std::fs::create_dir_all(path.join("samples"))?;
    let bank = Bank {
        version: 1,
        name: name.into(),
        license: "User-supplied recordings; specify your license".into(),
        samples: vec![],
        dictionary: BTreeMap::new(),
        language: None,
        inventory: None,
        provider: None,
        generation: None,
    };
    std::fs::write(path.join("voicebank.toml"), toml::to_string_pretty(&bank)?)?;
    Ok(())
}
pub fn validate(root: &Path, bank: &str) -> Result<String> {
    let v = Voice {
        bank: bank.into(),
        register: String::new(),
        style: String::new(),
        ..Default::default()
    };
    let b = LoadedBank::load(root, &v, &root.join("cache"))?;
    if b.meta.samples.is_empty() {
        bail!("empty bank");
    }
    Ok(format!(
        "{}: {} samples; metadata/audio valid; intelligibility requires listening",
        b.meta.name,
        b.meta.samples.len()
    ))
}

/// Binary search the pinned CMU dictionary without a heap-resident dictionary index.
pub fn english(word: &str) -> Option<String> {
    english_pronunciation(word, false)
}
/// Retain lexical stress for language-neutral pronunciation metadata.
pub fn english_stressed(word: &str) -> Option<String> {
    english_pronunciation(word, true)
}
fn english_pronunciation(word: &str, retain_stress: bool) -> Option<String> {
    const DATA: &str = include_str!(concat!(env!("OUT_DIR"), "/english.dict"));
    let key = word.to_ascii_lowercase();
    let mut low = 0;
    let mut high = DATA.len();
    while low < high {
        let mid = low + (high - low) / 2;
        let start = DATA[low..mid].rfind('\n').map_or(low, |i| low + i + 1);
        let end = DATA[start..].find('\n').map_or(DATA.len(), |i| start + i);
        let line = &DATA[start..end];
        let (entry, pron) = line.split_once(' ')?;
        match entry.cmp(key.as_str()) {
            std::cmp::Ordering::Less => low = (end + 1).min(DATA.len()),
            std::cmp::Ordering::Greater => high = start,
            std::cmp::Ordering::Equal => {
                let raw = pron.split('#').next().unwrap_or(pron);
                let phones = raw
                    .split_whitespace()
                    .map(|p| p.trim_end_matches(|c: char| c.is_ascii_digit()).to_owned())
                    .collect::<Vec<_>>();
                let nuclei = phones
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| vowel(p))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                if nuclei.is_empty() {
                    return None;
                }
                let mut cuts = vec![];
                for pair in nuclei.windows(2) {
                    let from = pair[0] + 1;
                    let to = pair[1];
                    let mut cut = to;
                    for at in from..to {
                        let cluster = phones[at..to].join(" ");
                        if legal_onset(&cluster) {
                            cut = at;
                            break;
                        }
                    }
                    cuts.push(cut);
                }
                let mut out = String::new();
                for (i, p) in phones.iter().enumerate() {
                    if cuts.contains(&i) {
                        out.push_str(" / ");
                    } else if i > 0 {
                        out.push(' ');
                    }
                    if retain_stress {
                        out.push_str(raw.split_whitespace().nth(i)?);
                    } else {
                        out.push_str(p);
                    }
                }
                return Some(out);
            }
        }
    }
    None
}
fn legal_onset(s: &str) -> bool {
    if !s.contains(' ') {
        return s != "NG";
    }
    [
        "P R", "P L", "B R", "B L", "T R", "D R", "K R", "K L", "G R", "G L", "F R", "F L", "TH R",
        "SH R", "S P", "S T", "S K", "S F", "S M", "S N", "S L", "S W", "S P R", "S P L", "S T R",
        "S K R", "S K W", "K W", "G W", "T W", "D W", "Y UW",
    ]
    .contains(&s)
}

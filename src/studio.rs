//! Integrated, non-destructive voicebank authoring. Disk/DSP operations run on the GUI worker.
use crate::{
    codec::{self, Asset},
    language::{self, Effect, Score, Track, Voice},
    project::{self, Project},
    voice::{self, Bank, Phoneme},
    voice_analysis::{self, Analysis},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

fn one() -> f32 {
    1.
}
pub const PHONES: &[(&str, &str, &str)] = &[
    ("AA", "father", "Open ah, as in father"),
    ("AE", "cat", "Short a, as in cat"),
    ("AH", "cup", "Uh, as in cup"),
    ("AO", "thought", "Aw, as in thought; dialects vary"),
    ("AW", "mouth", "Ah-to-oo glide, as in mouth"),
    ("AY", "my", "Ah-to-ee glide, as in my"),
    ("B", "bee", "Voiced lip stop; capture release into a vowel"),
    ("CH", "cheese", "Ch, as in cheese"),
    ("D", "day", "Voiced tongue-tip stop"),
    ("DH", "this", "Voiced th, as in this"),
    ("EH", "bed", "Short e, as in bed"),
    ("ER", "bird", "R-colored vowel, as in bird"),
    ("EY", "day", "Ay glide, as in day"),
    ("F", "fee", "Unvoiced lip/teeth friction"),
    ("G", "go", "Voiced back-of-mouth stop"),
    ("HH", "he", "Breathy h, as in he"),
    ("IH", "sit", "Short i, as in sit"),
    ("IY", "see", "Sustained ee, as in see"),
    ("JH", "judge", "J, as in judge"),
    ("K", "key", "Unvoiced back-of-mouth stop"),
    ("L", "lee", "L, as in lee"),
    ("M", "me", "Closed-lip voiced hum"),
    ("N", "knee", "Tongue-tip nasal hum"),
    ("NG", "sing", "Back-of-mouth nasal, as at end of sing"),
    ("OW", "go", "Oh-to-oo glide"),
    ("OY", "boy", "Aw-to-ee glide"),
    ("P", "pea", "Unvoiced lip stop"),
    ("R", "red", "English r, as in red"),
    ("S", "see", "Unvoiced s hiss"),
    ("SH", "she", "Unvoiced sh hiss"),
    ("T", "tea", "Unvoiced tongue-tip stop"),
    ("TH", "thin", "Unvoiced th, as in thin"),
    ("UH", "book", "Short oo, as in book"),
    ("UW", "blue", "Sustained oo, as in blue"),
    ("V", "vee", "Voiced lip/teeth friction"),
    ("W", "we", "Voiced oo-like onset"),
    ("Y", "yes", "Voiced ee-like onset"),
    ("Z", "zoo", "Voiced z buzz"),
    ("ZH", "vision", "Voiced zh, as in vision"),
];
pub fn valid_label(s: &str) -> bool {
    s.len() <= 63 && !s.is_empty() && s.split('.').all(|p| PHONES.iter().any(|v| v.0 == p))
}
fn slug(s: &str) -> Result<String> {
    ensure!(
        !s.is_empty()
            && s.len() <= 80
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' '),
        "name must use ASCII letters, digits, spaces, - or _"
    );
    Ok(s.replace(' ', "-"))
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let mut f = File::open(path)?;
    ensure!(f.metadata()?.len() <= 1_048_576, "metadata exceeds 1 MiB");
    let mut text = String::new();
    Read::by_ref(&mut f)
        .take(1_048_577)
        .read_to_string(&mut text)?;
    ensure!(text.len() <= 1_048_576, "metadata exceeds 1 MiB");
    Ok(toml::from_str(&text)?)
}
fn write<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<()> {
    let text = toml::to_string_pretty(value)?;
    ensure!(text.len() <= 1_048_576, "metadata exceeds 1 MiB");
    let path = dir.join(name);
    if path.exists()
        && fs::metadata(&path)?.len() <= 1_048_576
        && fs::read_to_string(&path)? == text
    {
        return Ok(());
    }
    let history = codec::directory(dir, "history")?;
    if path.exists() {
        let snapshot = codec::unique(&history, "metadata", "toml")?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(snapshot)?;
        std::io::copy(&mut File::open(&path)?, &mut out)?;
        out.sync_all()?;
    }
    let text = toml::to_string_pretty(value)?;
    ensure!(text.len() <= 1_048_576, "metadata exceeds 1 MiB");
    project::atomic(&path, text.as_bytes())
}
fn audio(path: &Path, cache: &Path) -> Result<Asset> {
    Asset::decode_bounded(path, cache, 5_760_000)
}
#[derive(Serialize)]
struct CaptureReceipt<'a> {
    version: u32,
    state: &'a str,
    file: String,
    capture: &'a Capture,
}
/// Store immutable capture context even if the app closes before finalization/import.
pub fn recording_started(root: &Path, bank: &str, file: &Path, capture: &Capture) -> Result<()> {
    let dir = bank_dir(root, bank)?;
    let receipt = CaptureReceipt {
        version: 1,
        state: "capture-started; completion requires successful finalization",
        file: codec::script_path(file.strip_prefix(&dir)?),
        capture,
    };
    let folder = codec::directory(&dir, "captures")?;
    let path = codec::unique(&folder, "capture", "toml")?;
    let text = toml::to_string_pretty(&receipt)?;
    let mut out = OpenOptions::new().write(true).create_new(true).open(path)?;
    use std::io::Write;
    out.write_all(text.as_bytes())?;
    out.sync_all()?;
    Ok(())
}
fn bank_dir(root: &Path, bank: &str) -> Result<PathBuf> {
    Ok(codec::safe_path(root, &format!("{bank}/voicebank.toml"))?
        .parent()
        .unwrap()
        .to_path_buf())
}
fn new_dir(root: &Path, relative: &str) -> Result<PathBuf> {
    let p = Path::new(relative);
    ensure!(
        !relative.is_empty()
            && !p.is_absolute()
            && p.components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "use a project-relative directory without traversal"
    );
    let parent = p.parent().unwrap_or(Path::new(""));
    codec::directory(root, &codec::script_path(parent))?;
    let target = root.join(p);
    ensure!(!target.exists(), "destination already exists");
    Ok(target)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
    pub format: String,
    pub tempo: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            rate: 48000,
            bits: 24,
            channels: 1,
            format: "flac".into(),
            tempo: 120.,
        }
    }
}
impl Settings {
    fn validate(&self) -> Result<()> {
        ensure!(
            [16000, 22050, 24000, 44100, 48000, 88200, 96000].contains(&self.rate)
                && [16, 24].contains(&self.bits)
                && [1, 2].contains(&self.channels)
                && ["flac", "wav"].contains(&self.format.as_str())
                && self.tempo.is_finite()
                && (20.0..=400.).contains(&self.tempo),
            "settings: rate 16..96 kHz supported by Songbird, bits 16|24, channels 1|2, format flac|wav, tempo 20..400"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capture {
    pub phoneme: String,
    pub register: String,
    pub expression: String,
    pub recorded: bool,
    #[serde(default)]
    pub phonetics: Option<crate::phonetics::Selection>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Take {
    pub file: String,
    pub capture: Capture,
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub format: String,
    pub onset: f64,
    pub sustain: f64,
    pub release: Option<f64>,
    pub end: f64,
    pub root: Option<f64>,
    pub voiced: Option<bool>,
    pub low: i32,
    pub high: i32,
    pub analysis: Analysis,
    #[serde(default)]
    pub sample_index: Option<usize>,
    #[serde(default = "one")]
    pub gain: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub version: u32,
    pub language: String,
    pub target: String,
    pub register: String,
    pub expression: String,
    pub settings: Settings,
    pub takes: Vec<Take>,
    pub selected: Option<usize>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            version: 1,
            language: "english".into(),
            target: "AA".into(),
            register: "alto".into(),
            expression: "normal".into(),
            settings: Settings::default(),
            takes: vec![],
            selected: None,
        }
    }
}
#[derive(Clone)]
pub struct View {
    pub bank: String,
    pub name: String,
    pub session: Session,
    pub samples: usize,
    pub language: String,
    pub inventory: String,
    pub provider: String,
    pub phones: Vec<crate::phonetics::Phone>,
    pub selection: crate::phonetics::Selection,
    pub aliases: std::collections::BTreeMap<String, String>,
    pub generation: Option<String>,
    pub waveform: Vec<f32>,
    pub duration: f64,
    pub report: Vec<String>,
}
pub struct Output {
    pub view: View,
    pub preview: Option<crate::engine::Renderer>,
}
pub struct Request {
    pub project: Project,
    pub bank: Option<String>,
    pub args: Vec<String>,
}
pub struct Studio {
    pub bank: String,
    pub dir: PathBuf,
    pub meta: Bank,
    pub session: Session,
    pub profile: crate::phonetics::ContextProfile,
}
impl Studio {
    pub fn create(root: &Path, bank: &str, name: &str, language: &str) -> Result<Self> {
        let selection = if language == "auto" {
            crate::phonetics::default_selection(root, None)?
        } else {
            crate::phonetics::Selection {
                language: Some(crate::locale::normalize(language)?),
                ..Default::default()
            }
        };
        Self::create_selected(root, bank, name, selection)
    }
    pub fn create_selected(
        root: &Path,
        bank: &str,
        name: &str,
        selection: crate::phonetics::Selection,
    ) -> Result<Self> {
        let profile = crate::phonetics::ContextProfile::load(root, &selection)?;
        ensure!(
            !profile.inventory.phones.is_empty(),
            "add inventory phones before creating a bank"
        );
        ensure!(
            !name.trim().is_empty() && name.len() <= 128,
            "bank name must have 1..128 characters"
        );
        let dir = new_dir(root, bank)?;
        voice::write_template(&dir, name)?;
        let studio = Self {
            bank: bank.into(),
            dir,
            meta: Bank {
                version: 1,
                name: name.into(),
                license:
                    "User-supplied authorized recordings; set your license before distribution"
                        .into(),
                samples: vec![],
                dictionary: Default::default(),
                language: Some(profile.language.clone()),
                inventory: Some(
                    selection
                        .inventory
                        .clone()
                        .unwrap_or(profile.pronunciation.language.clone()),
                ),
                provider: Some(profile.provider.clone()),
                generation: None,
            },
            session: Session {
                language: profile.language.clone(),
                target: profile.inventory.phones[0].id.clone(),
                ..Default::default()
            },
            profile,
        };
        crate::phonetics::save_profile(&studio.dir, &studio.profile.pronunciation)?;
        if studio.profile.inventory.language != studio.profile.pronunciation.language {
            crate::phonetics::save_profile(&studio.dir, &studio.profile.inventory)?;
        }
        studio.save()?;
        Ok(studio)
    }
    pub fn open(root: &Path, bank: &str) -> Result<Self> {
        let dir = bank_dir(root, bank)?;
        let meta: Bank = read(&dir.join("voicebank.toml"))?;
        ensure!(
            meta.version == 1 && meta.samples.len() <= 2048,
            "unsupported or oversized bank"
        );
        let profile = crate::phonetics::bank_context(&dir, &meta)?;
        for p in &meta.samples {
            profile.inventory.label(&p.phoneme)?;
        }
        let mut session: Session = if dir.join("studio.toml").exists() {
            read(&codec::safe_path(&dir, "studio.toml")?)?
        } else {
            Session {
                language: profile.language.clone(),
                target: profile
                    .inventory
                    .phones
                    .first()
                    .context("inventory has no phones")?
                    .id
                    .clone(),
                ..Default::default()
            }
        };
        ensure!(
            session.version == 1
                && session.takes.len() <= 2048
                && session.selected.is_none_or(|i| i < session.takes.len()),
            "invalid studio metadata"
        );
        ensure!(
            session
                .takes
                .iter()
                .all(|t| profile.inventory.label(&t.capture.phoneme).is_ok()
                    && t.capture.register.len() <= 128
                    && t.capture.expression.len() <= 128
                    && t.file.len() <= 4096
                    && t.analysis.duration.is_finite()
                    && t.analysis.duration > 0.
                    && t.analysis.duration <= 30.
                    && [t.onset, t.sustain, t.end].iter().all(|v| v.is_finite())
                    && t.root.is_none_or(f64::is_finite)
                    && t.release.is_none_or(f64::is_finite)),
            "invalid/nonfinite take metadata"
        );
        session.settings.validate()?;
        profile.inventory.label(&session.target)?;
        session.language = profile.language.clone();
        Ok(Self {
            bank: bank.into(),
            dir,
            meta,
            session,
            profile,
        })
    }
    pub fn save(&self) -> Result<()> {
        self.session.settings.validate()?;
        write(&self.dir, "studio.toml", &self.session)?;
        write(&self.dir, "voicebank.toml", &self.meta)
    }
    pub fn selected(&self) -> Result<&Take> {
        self.session
            .selected
            .and_then(|i| self.session.takes.get(i))
            .context("select a take with voicebank take NUMBER, or record/import one first")
    }
    fn selected_mut(&mut self) -> Result<&mut Take> {
        let i = self.session.selected.context("no take selected")?;
        self.session
            .takes
            .get_mut(i)
            .context("invalid selected take")
    }
    pub fn capture(&self) -> Capture {
        Capture {
            phoneme: self.session.target.clone(),
            register: self.session.register.clone(),
            expression: self.session.expression.clone(),
            recorded: false,
            phonetics: Some(crate::phonetics::bank_selection(&self.meta)),
        }
    }
    pub fn import(&mut self, source: &Path, capture: Capture) -> Result<()> {
        ensure!(
            self.profile.inventory.label(&capture.phoneme).is_ok(),
            "select a valid target phoneme or dot-separated syllable"
        );
        ensure!(self.session.takes.len() < 2048, "take limit 2048");
        let mut audio = audio(source, &self.dir.join("cache"))?;
        let analysis = voice_analysis::analyze(&mut audio)?;
        if let Some(selection) = &capture.phonetics {
            let context = crate::phonetics::ContextProfile::load(&self.dir, selection)?;
            ensure!(
                context.inventory.inventory == self.profile.inventory.inventory,
                "capture inventory differs from active bank; reopen its original bank"
            );
        }
        let extension = source
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let (bits, format) = if extension == "flac" {
            (
                claxon::FlacReader::open(source)?
                    .streaminfo()
                    .bits_per_sample as u16,
                "flac",
            )
        } else {
            (
                hound::WavReader::open(source)?.spec().bits_per_sample,
                "wav",
            )
        };
        let dest = codec::unique(&codec::directory(&self.dir, "originals")?, "take", format)?;
        let mut out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&dest)?;
        std::io::copy(&mut File::open(source)?, &mut out)?;
        out.sync_all()?;
        let midi = analysis
            .root
            .map(|f| (69. + 12. * (f / 440.).log2()).round() as i32)
            .unwrap_or(60);
        let take = Take {
            file: codec::script_path(dest.strip_prefix(&self.dir)?),
            capture,
            rate: audio.rate,
            channels: audio.channels,
            bits,
            format: format.into(),
            onset: analysis.onset.unwrap_or(0.),
            sustain: analysis.sustain.or(analysis.onset).unwrap_or(0.),
            release: analysis.release,
            end: analysis.end.unwrap_or(analysis.duration),
            root: analysis.root,
            voiced: analysis.voiced,
            low: if analysis.voiced == Some(false) {
                0
            } else {
                (midi - 3).max(0)
            },
            high: if analysis.voiced == Some(false) {
                127
            } else {
                (midi + 3).min(127)
            },
            analysis,
            sample_index: None,
            gain: 1.,
        };
        self.session.target = take.capture.phoneme.clone();
        self.session.takes.push(take);
        self.session.selected = Some(self.session.takes.len() - 1);
        self.save()
    }
    pub fn analyze(&mut self) -> Result<()> {
        let file = self.selected()?.file.clone();
        let mut audio = audio(
            &codec::safe_path(&self.dir, &file)?,
            &self.dir.join("cache"),
        )?;
        let report = voice_analysis::analyze(&mut audio)?;
        self.selected_mut()?.analysis = report;
        self.save()
    }
    pub fn publish(&mut self) -> Result<()> {
        let t = self.selected()?;
        let voiced = t
            .voiced
            .context("voicing is uncertain; voicebank set voiced true|false")?;
        let root = if voiced {
            t.root.context(
                "root pitch is uncertain; voicebank set root HZ or voicebank set note A3",
            )?
        } else {
            t.root
                .unwrap_or(crate::dsp::hz((t.low as f64 + t.high as f64) / 2.))
        };
        let p = Phoneme {
            phoneme: t.capture.phoneme.clone(),
            file: t.file.clone(),
            root,
            low: t.low,
            high: t.high,
            onset: t.onset,
            sustain: t.sustain,
            release: t.release,
            end: t.end,
            voiced,
            style: t.capture.expression.clone(),
            register: t.capture.register.clone(),
            gain: t.gain,
        };
        let audio = audio(
            &codec::safe_path(&self.dir, &p.file)?,
            &self.dir.join("cache"),
        )?;
        voice::validate_sample(&p, &audio)?;
        ensure!(
            audio.peak >= 0.001,
            "take is effectively silent; select another take"
        );
        let index = if let Some(index) = self.selected()?.sample_index {
            ensure!(
                index < self.meta.samples.len(),
                "selected sample mapping is missing"
            );
            self.meta.samples[index] = p;
            index
        } else if let Some(index) = self.meta.samples.iter().position(|s| {
            s.phoneme == p.phoneme
                && s.register == p.register
                && s.style == p.style
                && s.low == p.low
                && s.high == p.high
        }) {
            self.meta.samples[index] = p;
            index
        } else {
            ensure!(self.meta.samples.len() < 2048, "sample limit 2048");
            self.meta.samples.push(p);
            self.meta.samples.len() - 1
        };
        self.selected_mut()?.sample_index = Some(index);

        self.save()
    }
    pub fn view(&self, report: Vec<String>) -> Result<View> {
        let (waveform, duration) = if let Ok(take) = self.selected() {
            let a = audio(
                &codec::safe_path(&self.dir, &take.file)?,
                &self.dir.join("cache"),
            )?;
            (a.waveform.clone(), a.duration())
        } else {
            (vec![], 0.)
        };
        Ok(View {
            bank: self.bank.clone(),
            name: self.meta.name.clone(),
            session: self.session.clone(),
            samples: self.meta.samples.len(),
            language: self.profile.language.clone(),
            inventory: self.profile.inventory.inventory.clone(),
            provider: self.profile.provider.clone(),
            phones: self.profile.inventory.phones.clone(),
            selection: crate::phonetics::bank_selection(&self.meta),
            aliases: self.profile.inventory.aliases.clone(),
            generation: self.meta.generation.clone(),
            waveform,
            duration,
            report,
        })
    }
    pub fn select_sample(&mut self, index: usize) -> Result<()> {
        let p = self
            .meta
            .samples
            .get(index)
            .context("published sample number is missing")?
            .clone();
        if let Some(take) = self.session.takes.iter().position(|t| {
            t.file == p.file
                && t.capture.phoneme == p.phoneme
                && t.capture.register == p.register
                && t.capture.expression == p.style
        }) {
            self.session.selected = Some(take);
            self.session.takes[take].sample_index = Some(index);
        } else {
            ensure!(self.session.takes.len() < 2048, "take limit 2048");
            let source = codec::safe_path(&self.dir, &p.file)?;
            let mut audio = audio(&source, &self.dir.join("cache"))?;
            let analysis = voice_analysis::analyze(&mut audio)?;
            let format = source
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let bits = if format == "flac" {
                claxon::FlacReader::open(&source)?
                    .streaminfo()
                    .bits_per_sample as u16
            } else {
                hound::WavReader::open(&source)?.spec().bits_per_sample
            };
            self.session.takes.push(Take {
                file: p.file.clone(),
                capture: Capture {
                    phoneme: p.phoneme.clone(),
                    register: p.register.clone(),
                    expression: p.style.clone(),
                    recorded: false,
                    phonetics: Some(crate::phonetics::bank_selection(&self.meta)),
                },
                rate: audio.rate,
                channels: audio.channels,
                bits,
                format,
                onset: p.onset,
                sustain: p.sustain,
                release: p.release,
                end: p.end,
                root: Some(p.root),
                voiced: Some(p.voiced),
                low: p.low,
                high: p.high,
                analysis,
                sample_index: Some(index),
                gain: p.gain,
            });
            self.session.selected = Some(self.session.takes.len() - 1);
        }
        self.session.target = p.phoneme;
        self.session.register = p.register;
        self.session.expression = p.style;
        self.save()
    }
    pub fn coverage(
        &self,
        lyrics: Option<&str>,
        notes: Option<&str>,
        register: &str,
        expression: &str,
    ) -> Result<Vec<String>> {
        self.coverage_shift(lyrics, notes, register, expression, 0.)
    }
    fn coverage_shift(
        &self,
        lyrics: Option<&str>,
        notes: Option<&str>,
        register: &str,
        expression: &str,
        shift: f64,
    ) -> Result<Vec<String>> {
        let available = self
            .meta
            .samples
            .iter()
            .map(|s| s.phoneme.clone())
            .collect::<BTreeSet<_>>();
        let mut report = vec![format!(
            "Available labels: {}",
            available.iter().cloned().collect::<Vec<_>>().join(" ")
        )];
        let syllables = if let Some(lyrics) = lyrics {
            self.profile.phonemes(lyrics, &self.meta.dictionary)?
        } else {
            self.profile
                .inventory
                .phones
                .iter()
                .map(|p| vec![p.id.clone()])
                .collect()
        };
        let events = notes.map(|s| language::events(s, 0.5)).transpose()?;
        let sounding = events
            .as_ref()
            .map(|e| e.iter().filter(|e| !e.notes.is_empty()).collect::<Vec<_>>());
        if let Some(e) = &sounding {
            ensure!(
                e.len() == syllables.len(),
                "{} syllables require matching notes, but got {}; use ~ for melisma",
                syllables.len(),
                e.len()
            );
            ensure!(
                e.iter().all(|e| e.notes.len() == 1),
                "singing previews require monophonic notes"
            );
        }
        let mut required = BTreeSet::new();
        let mut missing = BTreeSet::new();
        let mut previous = None;
        let mut previous_source: Option<String> = None;
        for (i, syllable) in syllables.iter().enumerate() {
            let phones = if syllable == &["~"] {
                vec![previous.clone().context("~ needs a preceding vowel")?]
            } else {
                syllable.clone()
            };
            for p in &phones {
                required.insert(p.clone());
                if self.profile.inventory.nucleus(p) {
                    previous = Some(p.clone());
                }
            }
            let midi = sounding.as_ref().map(|e| e[i].notes[0] as f64 + shift);
            if syllable == &["~"]
                && let Some(label) = &previous_source
                && self.meta.samples.iter().any(|p| {
                    p.phoneme == *label
                        && (register.is_empty() || p.register == register)
                        && p.style == expression
                        && midi.is_none_or(|n| n >= p.low as f64 && n <= p.high as f64)
                })
            {
                report.push(format!("Melisma uses the stable vowel region of {label}"));
                continue;
            }
            let combined = phones.join(".");
            let matches = |s: &&Phoneme| {
                (register.is_empty() || s.register == register)
                    && s.style == expression
                    && midi.is_none_or(|n| n >= s.low as f64 && n <= s.high as f64)
            };
            if phones.len() > 1
                && self
                    .meta
                    .samples
                    .iter()
                    .filter(|s| s.phoneme == combined)
                    .any(|s| matches(&s))
            {
                previous_source = Some(combined.clone());
                report.push(format!("Using syllable sample {combined}"));
                continue;
            }
            for phone in &phones {
                let all = self
                    .meta
                    .samples
                    .iter()
                    .filter(|s| &s.phoneme == phone)
                    .collect::<Vec<_>>();
                if self.profile.inventory.nucleus(phone) && all.iter().any(&matches) {
                    previous_source = Some(phone.clone());
                }
                if all.is_empty() {
                    missing.insert(phone.clone());
                    report.push(format!("MISSING {phone}: voicebank target {phone}, then record/import and use a take"));
                } else if !all
                    .iter()
                    .any(|s| register.is_empty() || s.register == register)
                {
                    report.push(format!("UNSUPPORTED REGISTER {register} for {phone}"));
                } else if !all.iter().any(|s| {
                    (register.is_empty() || s.register == register) && s.style == expression
                }) {
                    report.push(format!(
                        "MISSING EXPRESSION {expression} / {register} for {phone}"
                    ));
                } else if !all.iter().any(matches) {
                    report.push(format!("OUT OF RANGE {phone}, note {}; record a closer root or correct low/high mapping",midi.unwrap_or(60.)));
                }
            }
        }
        report.insert(
            0,
            format!(
                "Required phonemes: {}",
                required.into_iter().collect::<Vec<_>>().join(" ")
            ),
        );
        report.push(format!(
            "Missing labels: {}",
            if missing.is_empty() {
                "none".into()
            } else {
                missing.into_iter().collect::<Vec<_>>().join(" ")
            }
        ));
        if let Some(lyrics) = lyrics {
            for word in lyrics
                .split_whitespace()
                .filter(|w| !w.starts_with('@') && *w != "~")
            {
                let key = word
                    .trim_matches(|c: char| c.is_ascii_punctuation())
                    .to_ascii_uppercase();
                if !self.meta.dictionary.contains_key(&key)
                    && self.profile.provider == "cmudict"
                    && voice::english(&format!("{key}(2)")).is_some()
                {
                    report.push(format!("AMBIGUOUS pronunciation {word}: default selected; set voicebank dictionary WORD \"PHONES / PHONES\" to override"));
                }
            }
        }
        if self.meta.samples.is_empty() {
            report
                .push("No published samples yet; voicebank use publishes a preferred take".into());
        }
        Ok(report)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VocalPreset {
    pub version: u32,
    pub name: String,
    pub bank: String,
    pub register: String,
    pub expression: String,
    pub gain: f32,
    pub pan: f32,
    pub vibrato: f32,
    pub breathiness: f32,
    pub pitch: f32,
    pub dynamics: f32,
    pub formant: f32,
    pub harmonies: Vec<i32>,
    pub effects: Vec<Effect>,
}
impl Default for VocalPreset {
    fn default() -> Self {
        Self {
            version: 1,
            name: "Vocal preset".into(),
            bank: ".".into(),
            register: "alto".into(),
            expression: "normal".into(),
            gain: 0.5,
            pan: 0.,
            vibrato: 0.15,
            breathiness: 0.,
            pitch: 0.,
            dynamics: 1.,
            formant: 1.,
            harmonies: vec![0],
            effects: vec![],
        }
    }
}
impl VocalPreset {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && !self.name.is_empty() && self.name.len() <= 128,
            "invalid preset version/name"
        );
        ensure!(
            [
                self.gain,
                self.pan,
                self.vibrato,
                self.breathiness,
                self.pitch,
                self.dynamics,
                self.formant
            ]
            .iter()
            .all(|v| v.is_finite()),
            "nonfinite preset value"
        );
        ensure!(
            (0.0..=4.).contains(&self.gain)
                && self.pan.abs() <= 1.
                && (0.0..=1.).contains(&self.vibrato)
                && (0.0..=1.).contains(&self.breathiness)
                && self.pitch.abs() <= 24.
                && (0.0..=2.).contains(&self.dynamics)
                && (0.5..=2.).contains(&self.formant),
            "preset parameters outside supported ranges"
        );
        ensure!(
            !self.harmonies.is_empty()
                && self.harmonies.len() <= 8
                && self.harmonies.iter().all(|n| (-24..=24).contains(n)),
            "harmony needs 1..8 offsets in -24..24"
        );
        ensure!(self.effects.len() <= 16, "preset effect limit 16");
        for e in &self.effects {
            let mut args = vec![e.kind.clone()];
            args.extend(e.args.iter().map(ToString::to_string));
            language::effect(&args)?;
        }
        Ok(())
    }
}
pub fn apply_presets(root: &Path, score: &mut Score) -> Result<()> {
    let mut expanded = vec![];
    for (index, original) in score.tracks.iter().enumerate() {
        let mut track = original.clone();
        if let Some(path) = &track.vocal_preset {
            let file = codec::safe_path(root, path)?;
            let preset: VocalPreset = read(&file)?;
            preset.validate()?;
            let manifest = codec::safe_path(
                file.parent().unwrap(),
                &format!("{}/voicebank.toml", preset.bank),
            )?;
            let bank = codec::script_path(
                manifest
                    .parent()
                    .unwrap()
                    .strip_prefix(root.canonicalize()?)?,
            );
            let key = format!("__preset_{index}");
            score.voices.insert(
                key.clone(),
                Voice {
                    bank,
                    register: preset.register.clone(),
                    style: preset.expression.clone(),
                    ..Default::default()
                },
            );
            track.voice = Some(key);
            macro_rules! setting {
                ($name:literal,$field:ident,$value:expr) => {
                    if !track.vocal_overrides.contains($name) {
                        track.$field = $value;
                    }
                };
            }
            setting!("gain", gain, preset.gain);
            setting!("pan", pan, preset.pan);
            setting!("vibrato", vibrato, preset.vibrato);
            setting!("breathiness", breathiness, preset.breathiness);
            setting!("expression", expression, preset.expression);
            setting!("formant", formant, preset.formant);
            setting!("dynamics", dynamics, preset.dynamics);
            setting!("vocal-pitch", vocal_pitch, preset.pitch);
            setting!("harmony", harmonies, preset.harmonies);
            let mut effects = preset.effects;
            effects.extend(track.effects);
            track.effects = effects;
        }
        let harmonies = if track.harmonies.is_empty() {
            vec![0]
        } else {
            track.harmonies.clone()
        };
        ensure!(
            harmonies.len() <= 8 && harmonies.iter().all(|h| (-24..=24).contains(h)),
            "invalid harmonies"
        );
        if harmonies != [0] {
            ensure!(
                track.voice.is_some(),
                "harmony requires a singer or vocal preset"
            );
        }
        for (part, offset) in harmonies.into_iter().enumerate() {
            let mut t = track.clone();
            t.transpose += offset;
            if part > 0 {
                t.name = format!("{}/harmony{part}", t.name);
            }
            t.harmonies.clear();
            expanded.push(t);
            ensure!(expanded.len() <= 128, "expanded track limit 128");
        }
    }
    score.tracks = expanded;
    Ok(())
}
fn take_number(s: &str, len: usize) -> Result<usize> {
    let n: usize = s.parse()?;
    ensure!(n > 0 && n <= len, "take number must be between 1 and {len}");
    Ok(n - 1)
}
fn arg(args: &[String], i: usize) -> Result<&str> {
    args.get(i)
        .map(String::as_str)
        .context("missing argument; voicebank help")
}
fn preset_path(name: &str) -> Result<String> {
    Ok(format!("preset-{}.toml", slug(name)?))
}
fn copy_bank(source: &Path, dest: &Path) -> Result<()> {
    ensure!(!dest.exists(), "destination already exists");
    let parent = dest
        .parent()
        .context("destination needs parent")?
        .canonicalize()?;
    ensure!(
        !parent.starts_with(source.canonicalize()?),
        "destination must be outside source bank"
    );
    fs::create_dir(dest)?;
    let mut todo = vec![PathBuf::new()];
    let mut count = 0;
    while let Some(rel) = todo.pop() {
        for entry in fs::read_dir(source.join(&rel))? {
            let entry = entry?;
            let child = rel.join(entry.file_name());
            if rel.as_os_str().is_empty() && entry.file_name() == "cache" {
                continue;
            }
            let meta = entry.file_type()?;
            ensure!(!meta.is_symlink(), "voicebank copy refuses symlinks");
            count += 1;
            ensure!(count <= 10000, "bank copy exceeds 10000 files");
            if meta.is_dir() {
                fs::create_dir(dest.join(&child))?;
                todo.push(child);
            } else if meta.is_file() {
                let mut f = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dest.join(&child))?;
                std::io::copy(&mut File::open(entry.path())?, &mut f)?;
                f.sync_all()?;
            }
        }
    }
    let meta: Bank = read(&codec::safe_path(source, "voicebank.toml")?)?;
    for sample in &meta.samples {
        let src = codec::safe_path(source, &sample.file)?;
        let target = dest.join(&sample.file);
        if !target.exists() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)?;
            std::io::copy(&mut File::open(src)?, &mut out)?;
            out.sync_all()?;
        }
    }
    Ok(())
}
pub fn execute(request: Request) -> Result<Output> {
    let root = &request.project.root;
    let args = &request.args;
    let op = arg(args, 0)?;
    if op == "import-bank" {
        let source = Path::new(arg(args, 1)?).canonicalize()?;
        let target = arg(args, 2)?;
        let voice = Voice {
            bank: ".".into(),
            ..Default::default()
        };
        voice::LoadedBank::load(&source, &voice, &root.join("cache"))?;
        let dest = new_dir(root, target)?;
        copy_bank(&source, &dest)?;
        let s = Studio::open(root, target)?;
        return Ok(Output {
            view: s.view(vec!["Imported portable voicebank".into()])?,
            preview: None,
        });
    }
    if op == "default" || op == "generate" {
        let language = if op == "default" {
            args.get(1)
        } else {
            args.get(3)
        };
        let selection = if let Some(tag) = language.filter(|s| s.as_str() != "auto") {
            crate::phonetics::Selection {
                language: Some(crate::locale::default_language(tag)?),
                ..Default::default()
            }
        } else {
            crate::phonetics::default_selection(root, Some(&request.project.source))?
        };
        let path = if op == "default" {
            crate::procedural::default_bank(root, &selection)?
        } else {
            crate::procedural::generate(root, arg(args, 1)?, arg(args, 2)?, &selection)?
        };
        let bank = Studio::open(root, &path)?;
        return Ok(Output{view:bank.view(vec!["Opened original procedural defaults. These are synthetic approximations, not human recordings or proof of intelligibility.".into()])?,preview:None});
    }
    let mut s = match op {
        "new" => {
            let selection = if let Some(tag) = args.get(3).filter(|tag| tag.as_str() != "auto") {
                let mut selection =
                    crate::phonetics::default_selection(root, Some(&request.project.source))?;
                selection.language = Some(crate::locale::normalize(tag)?);
                selection
            } else {
                crate::phonetics::default_selection(root, Some(&request.project.source))?
            };
            Studio::create_selected(root, arg(args, 1)?, arg(args, 2)?, selection)?
        }
        "open" => Studio::open(root, arg(args, 1)?)?,
        _ => Studio::open(
            root,
            request
                .bank
                .as_deref()
                .context("voicebank new PATH NAME english, or voicebank open PATH first")?,
        )?,
    };
    let mut report = vec![];
    let mut preview = None;
    match op {
        "language" | "inventory" | "provider" => {
            let value = arg(args, 1)?;
            let mut selection = crate::phonetics::bank_selection(&s.meta);
            match op {
                "language" => selection.language = Some(crate::locale::default_language(value)?),
                "inventory" => {
                    selection.inventory = if value == "auto" {
                        None
                    } else {
                        Some(crate::locale::normalize(value)?)
                    }
                }
                _ => {
                    selection.provider = if value == "auto" {
                        None
                    } else {
                        Some(value.into())
                    }
                }
            };
            let context = crate::phonetics::ContextProfile::load(root, &selection)
                .or_else(|_| crate::phonetics::ContextProfile::load(&s.dir, &selection))?;
            for p in &s.meta.samples {
                context.inventory.label(&p.phoneme)?;
            }
            for t in &s.session.takes {
                context.inventory.label(&t.capture.phoneme)?;
            }
            ensure!(!context.inventory.phones.is_empty(), "inventory is empty");
            s.meta.language = Some(context.language.clone());
            s.meta.inventory = Some(
                selection
                    .inventory
                    .clone()
                    .unwrap_or(context.pronunciation.language.clone()),
            );
            s.meta.provider = Some(context.provider.clone());
            s.session.language = context.language.clone();
            if context.inventory.label(&s.session.target).is_err() {
                s.session.target = context.inventory.phones[0].id.clone();
            }
            crate::phonetics::save_profile(&s.dir, &context.pronunciation)?;
            if context.inventory.language != context.pronunciation.language {
                crate::phonetics::save_profile(&s.dir, &context.inventory)?;
            }
            s.profile = context;
            s.save()?;
        }
        "normalize" => {
            let target = args
                .get(1)
                .map(|s| s.parse::<f32>())
                .transpose()?
                .unwrap_or(0.95);
            ensure!(
                target.is_finite() && target > 0. && target <= 1.,
                "normalization target must be in (0,1]"
            );
            let t = s.selected_mut()?;
            ensure!(
                t.analysis.peak >= 0.001,
                "recording is too quiet to normalize safely"
            );
            let gain = target / t.analysis.peak;
            ensure!(
                gain <= 16.,
                "normalization would require more than 16x gain; record a louder take"
            );
            t.gain = gain;
            s.save()?;
        }
        "silence" => {
            let t = s.selected_mut()?;
            t.onset = t
                .analysis
                .onset
                .context("no reliable onset; set timings manually")?;
            t.end = t.analysis.end.context("no reliable end")?;
            t.sustain = t.sustain.max(t.onset).min(t.end - 0.0001);
            if t.release.is_some_and(|v| v < t.sustain || v > t.end) {
                t.release = None;
            }
            s.save()?;
        }

        "new" | "open" => report.push(format!("Opened {} ({})", s.meta.name, s.bank)),
        "save" => {
            s.save()?;
            report.push(
                "Saved bank and studio metadata; takes and previous metadata retained".into(),
            );
        }
        "target" => {
            let p = arg(args, 1)?;
            ensure!(
                s.profile.inventory.label(p).is_ok(),
                "use ARPABET (AA) or a dot-separated syllable (M.IY)"
            );
            s.session.target = p.into();
            s.save()?;
        }
        "register" => {
            s.session.register = slug(arg(args, 1)?)?;
            s.save()?;
        }
        "expression" => {
            s.session.expression = slug(arg(args, 1)?)?;
            s.save()?;
        }
        "settings" => {
            s.session.settings.rate = arg(args, 1)?.parse()?;
            s.session.settings.bits = arg(args, 2)?.parse()?;
            s.session.settings.channels = arg(args, 3)?.parse()?;
            s.session.settings.format = args.get(4).map(String::as_str).unwrap_or("flac").into();
            s.save()?;
        }
        "tempo" => {
            s.session.settings.tempo = arg(args, 1)?.parse()?;
            s.save()?;
        }
        "import" => {
            let mut capture = s.capture();
            if let Some(p) = args.get(2) {
                capture.phoneme = p.clone();
            }
            s.import(Path::new(arg(args, 1)?), capture)?;
            report.push("Imported original audio; analysis values are editable suggestions. Inspect, then voicebank use".into());
        }
        "take" => {
            s.session.selected = Some(take_number(arg(args, 1)?, s.session.takes.len())?);
            s.save()?;
        }
        "samples" => {
            for (i, p) in s.meta.samples.iter().enumerate() {
                report.push(format!(
                    "{}: {} · {} / {} · {:.1} Hz · MIDI {}..{} · {}",
                    i + 1,
                    p.phoneme,
                    p.register,
                    p.style,
                    p.root,
                    p.low,
                    p.high,
                    p.file
                ));
            }
        }
        "sample" => {
            let index = take_number(arg(args, 1)?, s.meta.samples.len())?;
            s.select_sample(index)?;
        }
        "takes" => {
            for (i, t) in s.session.takes.iter().enumerate() {
                report.push(format!(
                    "{}: {} {} / {} · {:.3}s · {}",
                    i + 1,
                    t.capture.phoneme,
                    t.capture.register,
                    t.capture.expression,
                    t.analysis.duration,
                    t.file
                ));
            }
        }
        "analyze" => {
            s.analyze()?;
            report.push("Refreshed analysis suggestions; existing manual metadata kept. Use voicebank suggest to copy suggestions".into());
        }
        "suggest" => {
            let t = s.selected_mut()?;
            let a = &t.analysis;
            t.onset = a.onset.unwrap_or(0.);
            t.sustain = a.sustain.unwrap_or(t.onset);
            t.release = a.release;
            t.end = a.end.unwrap_or(a.duration);
            t.root = a.root;
            t.voiced = a.voiced;
            s.save()?;
        }
        "trim" => {
            let onset = arg(args, 1)?.parse()?;
            let sustain = arg(args, 2)?.parse()?;
            let end = arg(args, 3)?.parse()?;
            let t = s.selected_mut()?;
            t.onset = onset;
            t.sustain = sustain;
            t.end = end;
            ensure!(
                [onset, sustain, end].iter().all(|n: &f64| n.is_finite())
                    && onset >= 0.
                    && sustain >= onset
                    && end > sustain
                    && end <= t.analysis.duration,
                "trim requires 0 <= onset <= sustain < end <= duration"
            );
            if t.release.is_some_and(|r| r > end || r < sustain) {
                t.release = None;
            }
            s.save()?;
        }
        "set" => {
            let key = arg(args, 1)?;
            let value = arg(args, 2)?;
            let inventory = s.profile.inventory.clone();
            let t = s.selected_mut()?;
            match key {
                "root" => {
                    let n: f64 = value.parse()?;
                    ensure!(
                        n.is_finite() && (40.0..=2000.).contains(&n),
                        "root Hz range 40..2000"
                    );
                    t.root = Some(n);
                }
                "note" => t.root = Some(crate::dsp::hz(language::note(value)? as f64)),
                "low" => t.low = language::note(value)?,
                "high" => t.high = language::note(value)?,
                "voiced" => t.voiced = Some(value.parse()?),
                "onset" | "sustain" | "release" | "end" => {
                    let n: f64 = value.parse()?;
                    ensure!(
                        n.is_finite() && n >= 0. && n <= t.analysis.duration,
                        "timing outside source recording"
                    );
                    match key {
                        "onset" => t.onset = n,
                        "sustain" => t.sustain = n,
                        "release" => t.release = Some(n),
                        _ => t.end = n,
                    }
                }
                "phoneme" => {
                    inventory.label(value)?;
                    t.capture.phoneme = value.into();
                }
                "register" => t.capture.register = slug(value)?,
                "expression" => t.capture.expression = slug(value)?,
                _ => bail!(
                    "set root|note|low|high|voiced|onset|sustain|release|end|phoneme|register|expression VALUE"
                ),
            };
            s.save()?;
        }
        "use" => {
            s.publish()?;
            report.push("Published selected take; other takes and original audio preserved".into());
        }
        "dictionary" => {
            let word = arg(args, 1)?.to_ascii_uppercase();
            ensure!(
                word.len() <= 128 && !word.contains(char::is_whitespace),
                "dictionary word must be one word"
            );
            let pron = arg(args, 2)?;
            let test = std::collections::BTreeMap::from([(word.clone(), pron.into())]);
            let syllables = s.profile.phonemes(&word, &test)?;
            ensure!(
                syllables.iter().all(|syll| syll.iter().all(|p| s
                    .profile
                    .inventory
                    .label(p)
                    .is_ok()
                    && !p.contains('.'))
                    && (syll.iter().any(|p| s.profile.inventory.nucleus(p))
                        || s.profile.pronunciation.timing.allow_consonant_syllables)),
                "dictionary entry needs selected inventory phones and a nucleus or explicitly supported syllabic consonant"
            );
            s.meta.dictionary.insert(word, pron.into());
            s.save()?;
        }
        "license" => {
            s.meta.license = arg(args, 1)?.into();
            ensure!(!s.meta.license.is_empty(), "license cannot be empty");
            s.save()?;
        }
        "guide" => {
            if let Some(guide) = args.get(1) {
                let targets = if guide == "next" {
                    s.profile
                        .inventory
                        .phones
                        .iter()
                        .map(|p| p.id.clone())
                        .collect::<Vec<_>>()
                } else {
                    s.profile
                        .phonemes(guide, &s.meta.dictionary)?
                        .into_iter()
                        .filter(|phones| {
                            !s.meta.samples.iter().any(|p| {
                                p.phoneme == phones.join(".")
                                    && p.register == s.session.register
                                    && p.style == s.session.expression
                            })
                        })
                        .flatten()
                        .filter(|p| p != "~")
                        .collect()
                };
                if let Some(phone) = targets.into_iter().find(|phone| {
                    !s.meta.samples.iter().any(|p| {
                        p.phoneme == *phone
                            && p.register == s.session.register
                            && p.style == s.session.expression
                    })
                }) {
                    s.session.target = phone;
                    s.save()?;
                } else {
                    report.push("Requested phone labels are covered in this register/expression; check coverage with notes for pitch limits".into());
                }
            }
            for p in s.session.target.split('.') {
                if let Some(v) = s.profile.inventory.phone(p) {
                    report.push(format!(
                        "{p}: {} — reference word / prompt: {}",
                        v.description, v.prompt
                    ));
                }
            }
            report.push("Speak/sing the displayed target; hold vowels steadily, keep stop consonants natural. Pitch suggestions never identify the phoneme.".into());
        }
        "coverage" | "missing" | "validate" => {
            if let Some(name) = args.get(3) {
                let path = codec::safe_path(&s.dir, &preset_path(name)?)?;
                let p: VocalPreset = read(&path)?;
                p.validate()?;
                let manifest = codec::safe_path(&s.dir, &format!("{}/voicebank.toml", p.bank))?;
                let bank = codec::script_path(
                    manifest
                        .parent()
                        .unwrap()
                        .strip_prefix(root.canonicalize()?)?,
                );
                let target = Studio::open(root, &bank)?;
                for offset in &p.harmonies {
                    report.push(format!(
                        "Preset {}: register {} / expression {}, pitch {:+.2} semitones",
                        p.name,
                        p.register,
                        p.expression,
                        *offset as f32 + p.pitch
                    ));
                    report.extend(target.coverage_shift(
                        args.get(1).map(String::as_str),
                        args.get(2).map(String::as_str),
                        &p.register,
                        &p.expression,
                        *offset as f64 + p.pitch as f64,
                    )?);
                }
                if !target.meta.samples.is_empty() {
                    match voice::validate(root, &target.bank) {
                        Ok(v) => report.push(v),
                        Err(e) => report.push(format!("INVALID AUDIO/METADATA: {e:#}")),
                    }
                }
            } else {
                report.extend(s.coverage(
                    args.get(1).map(String::as_str),
                    args.get(2).map(String::as_str),
                    &s.session.register,
                    &s.session.expression,
                )?);
                if !s.meta.samples.is_empty() {
                    match voice::validate(root, &s.bank) {
                        Ok(v) => report.push(v),
                        Err(e) => report.push(format!("INVALID AUDIO/METADATA: {e:#}")),
                    }
                }
            }
        }
        "replay" => {
            let t = s.selected()?;
            let mut score = Score {
                rate: s.session.settings.rate,
                bits: s.session.settings.bits,
                ..Default::default()
            };
            let mut track = Track::new("Studio take".into());
            let mut c = language::Clip::new(format!("{}/{}", s.bank, t.file));
            if args.get(1).map(String::as_str) != Some("raw") {
                ensure!(
                    t.end > t.onset && t.end <= t.analysis.duration,
                    "correct take trim before replay, or voicebank replay raw"
                );
                c.start = t.onset;
                c.end = Some(t.end);
            }
            if args.get(1).map(String::as_str) != Some("raw") {
                c.gain = t.gain;
            }
            let duration = c.end.unwrap_or(t.analysis.duration) - c.start;
            track.clips.push(c);
            score.bars = ((duration + 0.2) / 2.).ceil().max(1.) as u32;
            score.tracks.push(track);
            preview = Some(crate::engine::Renderer::prepare(root, score)?);
        }
        "test" => {
            let lyrics = arg(args, 1)?;
            let notes = arg(args, 2)?;
            let mut score = Score {
                rate: s.session.settings.rate,
                bits: s.session.settings.bits,
                tempo: s.session.settings.tempo,
                ..Default::default()
            };
            let mut track = Track::new("Studio singer".into());
            track.lyrics = lyrics.into();
            track.sequence = notes.into();
            track.expression = s.session.expression.clone();
            if let Some(name) = args.get(3) {
                track.vocal_preset = Some(format!("{}/{}", s.bank, preset_path(name)?));
                track.vocal_overrides.clear();
            } else {
                score.voices.insert(
                    "Studio".into(),
                    Voice {
                        bank: s.bank.clone(),
                        register: s.session.register.clone(),
                        style: s.session.expression.clone(),
                        ..Default::default()
                    },
                );
                track.voice = Some("Studio".into());
            }
            let events = language::events(notes, 0.5)?;
            let beats = events.last().map_or(0., |e| e.beat + e.duration);
            ensure!(
                beats > 0. && beats <= 256.,
                "preview must contain 0..256 beats"
            );
            score.bars = (beats / 4.).ceil() as u32 + 1;
            score.tracks.push(track);
            preview=Some(crate::engine::Renderer::prepare(root,score).map_err(|e|anyhow::anyhow!("Preview: {e:#}. Run voicebank coverage \"{lyrics}\" \"{notes}\" to inspect requirements"))?);
            report.push(
                "Preview prepared through the song renderer; no full-song export required".into(),
            );
        }
        "presets" => {
            let mut files = fs::read_dir(&s.dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with("preset-") && n.ends_with(".toml"))
                })
                .map(|e| e.path())
                .collect::<Vec<_>>();
            files.sort();
            ensure!(files.len() <= 2048, "preset limit 2048");
            for file in files {
                let p: VocalPreset = read(&file)?;
                p.validate()?;
                report.push(format!(
                    "{} — {}",
                    p.name,
                    file.file_name().unwrap().to_string_lossy()
                ));
            }
        }
        "preset" => {
            let action = arg(args, 1)?;
            let name = arg(args, 2)?;
            let filename = preset_path(name)?;
            let mut p = if action == "new" {
                ensure!(!s.dir.join(&filename).exists(), "preset already exists");
                VocalPreset {
                    name: name.into(),
                    register: s.session.register.clone(),
                    expression: s.session.expression.clone(),
                    ..Default::default()
                }
            } else {
                read::<VocalPreset>(&codec::safe_path(&s.dir, &filename)?)?
            };
            match action {
                "new" => {}
                "show" => {
                    report.extend(toml::to_string_pretty(&p)?.lines().map(str::to_owned));
                }
                "set" => {
                    let key = arg(args, 3)?;
                    let value = arg(args, 4)?;
                    match key {
                        "register" => p.register = slug(value)?,
                        "expression" => p.expression = slug(value)?,
                        "gain" => p.gain = language::gain(value)?,
                        "pan" => p.pan = value.parse()?,
                        "vibrato" => p.vibrato = value.parse()?,
                        "breathiness" => p.breathiness = value.parse()?,
                        "pitch" => p.pitch = value.parse()?,
                        "dynamics" => p.dynamics = value.parse()?,
                        "formant" => p.formant = value.parse()?,
                        "harmony" => {
                            p.harmonies = value
                                .split_whitespace()
                                .map(str::parse)
                                .collect::<std::result::Result<Vec<_>, _>>()?
                        }
                        _ => bail!("unknown preset parameter {key}"),
                    }
                }
                "effect" => {
                    p.effects.push(language::effect(&args[3..])?);
                }
                "clear-effects" => p.effects.clear(),
                _ => bail!("preset new|show|set|effect|clear-effects NAME ..."),
            };
            p.validate()?;
            write(&s.dir, &filename, &p)?;
            report.push(format!(
                "Preset saved. Track syntax: vocal-preset \"{}/{}\"",
                s.bank, filename
            ));
        }
        "export" => {
            let target = Path::new(arg(args, 1)?);
            copy_bank(&s.dir, target)?;
            report.push(format!("Portable bank exported to {}", target.display()));
        }
        _ => bail!("unknown voicebank command {op}; voicebank help"),
    }
    Ok(Output {
        view: s.view(report)?,
        preview,
    })
}
pub fn finish_capture(
    project: Project,
    bank: String,
    source: PathBuf,
    capture: Capture,
) -> Result<Output> {
    let mut s = Studio::open(&project.root, &bank)?;
    s.import(&source, capture)?;
    Ok(Output{view:s.view(vec!["Hardware take imported. Inspect analysis and timing; voicebank use publishes the selected take.".into()])?,preview:None})
}

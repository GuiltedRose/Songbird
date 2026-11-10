//! Selected-only, offline phonetic profiles and interchangeable pronunciation providers.
use crate::locale;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Phone {
    pub id: String,
    pub vowel: bool,
    pub voiced: bool,
    pub syllabic: bool,
    pub stop: bool,
    pub description: String,
    pub prompt: String,
    pub formants: Option<[f32; 3]>,
    pub formants_end: Option<[f32; 3]>,
    pub manner: String,
    pub nasal: bool,
    pub noise_cutoff: f32,
}
impl Default for Phone {
    fn default() -> Self {
        Self {
            id: String::new(),
            vowel: false,
            voiced: false,
            syllabic: false,
            stop: false,
            description: String::new(),
            prompt: String::new(),
            formants: None,
            formants_end: None,
            manner: "auto".into(),
            nasal: false,
            noise_cutoff: 2000.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    pub consonant_fraction: f64,
    pub consonant_max: f64,
    pub crossfade_ms: f64,
    pub allow_consonant_syllables: bool,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            consonant_fraction: 0.35,
            consonant_max: 0.075,
            crossfade_ms: 4.,
            allow_consonant_syllables: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub text: String,
    pub output: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub language: String,
    pub name: String,
    pub inventory: String,
    pub provider: String,
    pub provider_version: String,
    pub license: String,
    pub attribution: String,
    pub capabilities: Vec<String>,
    pub scripts: Vec<String>,
    pub case_sensitive: bool,
    pub syllabification: String,
    pub tokenization: String,
    pub phones: Vec<Phone>,
    pub aliases: BTreeMap<String, String>,
    pub dictionary: BTreeMap<String, String>,
    pub dictionary_file: Option<String>,
    pub dictionary_format: String,
    #[serde(skip)]
    pub lexicon: crate::lexicon::Lexicon,
    pub rules: Vec<Rule>,
    pub timing: Timing,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            version: 1,
            language: "x-custom".into(),
            name: "Custom".into(),
            inventory: "x-custom".into(),
            provider: "manual".into(),
            provider_version: "1".into(),
            license: "User-authored; specify a license".into(),
            attribution: String::new(),
            capabilities: vec!["manual".into()],
            scripts: vec!["Unicode".into()],
            case_sensitive: false,
            syllabification: "explicit".into(),
            tokenization: "whitespace".into(),
            phones: vec![],
            aliases: Default::default(),
            dictionary: Default::default(),
            dictionary_file: None,
            dictionary_format: "phones".into(),
            lexicon: Default::default(),
            rules: vec![],
            timing: Timing::default(),
        }
    }
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.chars().all(|c| {
            !c.is_whitespace()
                && !c.is_control()
                && !['.', '/', '@', '~', '\'', 'ˈ', 'ˌ', ','].contains(&c)
        })
}
pub fn lexical_label(s: &str) -> bool {
    s.len() <= 255 && !s.is_empty() && s.split('.').all(identifier)
}
pub fn read_resource<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T> {
    let f = fs::File::open(file).with_context(|| format!("resource {}", file.display()))?;
    ensure!(
        f.metadata()?.is_file() && f.metadata()?.len() <= 1_048_576,
        "resource must be a regular file up to 1 MiB"
    );
    let mut text = String::new();
    f.take(1_048_577).read_to_string(&mut text)?;
    ensure!(text.len() <= 1_048_576, "resource exceeds 1 MiB");
    toml::from_str(&text).with_context(|| format!("resource {}", file.display()))
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && !self.name.is_empty()
                && !self.license.is_empty()
                && !self.provider_version.is_empty(),
            "invalid profile metadata"
        );
        locale::normalize(&self.language)?;
        ensure!(identifier(&self.inventory), "invalid inventory identifier");
        ensure!(
            ["cmudict", "dictionary", "rules", "manual"].contains(&self.provider.as_str()),
            "unsupported pronunciation provider"
        );
        ensure!(
            ["whitespace", "dictionary"].contains(&self.tokenization.as_str()),
            "tokenization must be whitespace or dictionary"
        );
        ensure!(
            ["explicit", "vowels"].contains(&self.syllabification.as_str()),
            "syllabification must be explicit or vowels"
        );
        ensure!(
            self.phones.len() <= 512
                && self.aliases.len() <= 2048
                && self.dictionary.len() <= 20000
                && self.rules.len() <= 4096,
            "profile resource limits exceeded"
        );
        ensure!(
            ["phones", "ipa"].contains(&self.dictionary_format.as_str()),
            "dictionary_format must be phones or ipa"
        );
        if let Some(file) = &self.dictionary_file {
            ensure!(
                !Path::new(file).is_absolute()
                    && Path::new(file)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "dictionary file must be resource-relative"
            );
        }
        let mut ids = BTreeSet::new();
        for p in &self.phones {
            ensure!(
                identifier(&p.id) && ids.insert(p.id.clone()),
                "duplicate or invalid phoneme identifier {}",
                p.id
            );
            ensure!(
                [
                    "auto",
                    "vowel",
                    "nasal",
                    "approximant",
                    "fricative",
                    "stop",
                    "affricate",
                    "trill",
                    "tap"
                ]
                .contains(&p.manner.as_str())
                    && p.noise_cutoff.is_finite()
                    && (80.0..=7000.).contains(&p.noise_cutoff),
                "invalid procedural phone shape"
            );
            if let Some(f) = p.formants_end {
                ensure!(
                    f.iter()
                        .all(|v| v.is_finite() && (80.0..=7500.).contains(v)),
                    "invalid end formants"
                );
            }
            if let Some(f) = p.formants {
                ensure!(
                    f.iter()
                        .all(|v| v.is_finite() && (80.0..=7500.).contains(v)),
                    "invalid procedural formants"
                );
            }
        }
        for (alias, target) in &self.aliases {
            ensure!(
                identifier(alias) && !ids.contains(alias) && ids.contains(target),
                "alias must reference a canonical phoneme and not shadow it"
            );
        }
        let t = &self.timing;
        ensure!(
            t.consonant_fraction.is_finite()
                && (0.0..=0.8).contains(&t.consonant_fraction)
                && t.consonant_max.is_finite()
                && (0.005..=0.25).contains(&t.consonant_max)
                && t.crossfade_ms.is_finite()
                && (0.0..=25.).contains(&t.crossfade_ms),
            "invalid language timing"
        );
        for r in &self.rules {
            ensure!(
                !r.text.is_empty() && r.text.len() <= 128 && r.output.len() <= 512,
                "invalid literal pronunciation rule"
            );
        }
        Ok(())
    }
    pub fn load(root: &Path, id: &str) -> Result<Self> {
        let id = locale::normalize(id)?;
        let resources = resource_root();
        // Vocal resources must never fall back to English merely because the requested profile is absent.
        for tag in locale::fallback(&id)
            .into_iter()
            .filter(|s| s != "en" || id.split('-').next() == Some("en"))
        {
            let relative = format!("languages/{tag}.toml");
            let path = root.join(&relative);
            if path.exists() {
                let path = crate::codec::safe_path(root, &relative)?;
                let mut p: Self = read_resource(&path)?;
                p.lexicon.base = path.parent().map(Path::to_path_buf);
                p.validate()?;
                return Ok(p);
            }
            if let Some(dir) = &resources {
                let file = dir.join("languages").join(format!("{tag}.toml"));
                if file.exists() {
                    let mut p: Self = read_resource(&file)?;
                    p.lexicon.base = file.parent().map(Path::to_path_buf);
                    p.validate()?;
                    return Ok(p);
                }
            }
            let text = match tag.as_str() {
                "en" => Some(include_str!("../resources/languages/en.toml")),
                "de" => Some(include_str!("../resources/languages/de.toml")),
                "fr" => Some(include_str!("../resources/languages/fr.toml")),
                "ja" => Some(include_str!("../resources/languages/ja.toml")),
                "la" => Some(include_str!("../resources/languages/la.toml")),
                _ => None,
            };
            if let Some(text) = text {
                let p: Self = toml::from_str(text)?;
                p.validate()?;
                return Ok(p);
            }
        }
        bail!(
            "language profile unavailable: {id}; use language new {id} NAME, add phones, and choose manual/dictionary/rules pronunciation"
        )
    }
    pub fn canonical(&self, id: &str) -> Result<String> {
        let value = self.aliases.get(id).map(String::as_str).unwrap_or(id);
        ensure!(
            self.phones.iter().any(|p| p.id == value),
            "phoneme {id:?} is not in inventory {}; select a compatible inventory or add an explicit alias",
            self.inventory
        );
        Ok(value.into())
    }
    pub fn label(&self, label: &str) -> Result<String> {
        ensure!(lexical_label(label), "invalid phoneme label");
        label
            .split('.')
            .map(|p| self.canonical(p))
            .collect::<Result<Vec<_>>>()
            .map(|p| p.join("."))
    }
    pub fn phone(&self, id: &str) -> Option<&Phone> {
        let id = self.aliases.get(id).map(String::as_str).unwrap_or(id);
        self.phones.iter().find(|p| p.id == id)
    }
    pub fn nucleus(&self, id: &str) -> bool {
        self.phone(id).is_some_and(|p| p.vowel || p.syllabic)
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Selection {
    pub language: Option<String>,
    pub inventory: Option<String>,
    pub provider: Option<String>,
}
#[derive(Clone)]
pub struct ContextProfile {
    pub language: String,
    pub inventory: Profile,
    pub pronunciation: Profile,
    pub provider: String,
}
impl ContextProfile {
    pub fn load(root: &Path, selection: &Selection) -> Result<Self> {
        let language = selection.language.as_deref().unwrap_or("en");
        let language = locale::normalize(language)?;
        let mut pronunciation = Profile::load(root, &language)?;
        let inventory = if let Some(id) = &selection.inventory {
            Profile::load(root, id)?
        } else {
            pronunciation.clone()
        };
        let provider = selection
            .provider
            .clone()
            .unwrap_or_else(|| pronunciation.provider.clone());
        ensure!(
            ["cmudict", "dictionary", "rules", "manual"].contains(&provider.as_str()),
            "unknown pronunciation provider {provider}"
        );
        ensure!(
            provider != "cmudict" || pronunciation.language.split('-').next() == Some("en"),
            "CMUdict is an English provider; choose dictionary, rules, or manual"
        );
        pronunciation.provider = provider.clone();
        Ok(Self {
            language,
            inventory,
            pronunciation,
            provider,
        })
    }
    pub fn legacy(root: &Path) -> Result<Self> {
        Self::load(
            root,
            &Selection {
                language: Some("en".into()),
                ..Default::default()
            },
        )
    }
    pub fn phonemes(
        &self,
        lyrics: &str,
        overrides: &BTreeMap<String, String>,
    ) -> Result<Vec<Vec<String>>> {
        Ok(self
            .pronounce(lyrics, overrides)?
            .syllables
            .into_iter()
            .map(|s| s.phones)
            .collect())
    }
    pub fn pronounce(
        &self,
        lyrics: &str,
        overrides: &BTreeMap<String, String>,
    ) -> Result<Pronunciation> {
        ensure!(lyrics.len() <= 1_048_576, "lyrics exceed 1 MiB");
        let mut result = vec![];
        let mut phrase_marks = vec![];
        for token in lyric_tokens(lyrics) {
            let token = match token {
                LyricToken::Mark(mark) => {
                    phrase_marks.push(PhraseMark {
                        after_syllable: result.len(),
                        mark,
                    });
                    continue;
                }
                LyricToken::Word(word) => word,
            };
            if token == "~" {
                result.push(Syllable {
                    phones: vec!["~".into()],
                    stress: None,
                });
                continue;
            }
            if let Some(explicit) = token.strip_prefix('@') {
                result.extend(self.parse_output(&explicit.replace('.', " "))?);
                continue;
            }
            let word = token
                .trim_matches(|c: char| ["\"", "'", "‘", "’"].iter().any(|s| s.starts_with(c)));
            if word.is_empty() {
                continue;
            }
            let word = word.replace('’', "'");
            let mut rest = word.as_str();
            while !rest.is_empty() {
                let lookup = |map: &BTreeMap<String, String>, word: &str| {
                    map.get(word)
                        .or_else(|| {
                            if self.pronunciation.case_sensitive {
                                None
                            } else {
                                map.get(&word.to_lowercase())
                                    .or_else(|| map.get(&word.to_uppercase()))
                            }
                        })
                        .cloned()
                };
                let matched = rest
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(rest.len()))
                    .filter(|i| {
                        *i > 0
                            && (self.pronunciation.tokenization == "dictionary" || *i == rest.len())
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .find_map(|i| {
                        lookup(overrides, &rest[..i])
                            .or_else(|| lookup(&self.pronunciation.dictionary, &rest[..i]))
                            .map(|value| (i, value))
                    });
                if let Some((bytes, mapped)) = matched {
                    result.extend(self.parse_output(&mapped)?);
                    rest = &rest[bytes..];
                } else {
                    if let Some(file) = &self.pronunciation.dictionary_file
                        && let Some(text) = self.pronunciation.lexicon.lookup(
                            file,
                            rest,
                            self.pronunciation.case_sensitive,
                        )?
                    {
                        result.extend(if self.pronunciation.dictionary_format == "ipa" {
                            self.parse_ipa(&text)?
                        } else {
                            self.parse_output(&text)?
                        });
                        break;
                    }
                    let provider: &dyn PronunciationProvider = match self.provider.as_str() {
                        "cmudict" => &CmuProvider,
                        "rules" => &RuleProvider,
                        _ => &ManualProvider,
                    };
                    result.extend(provider.word(self, rest)?);
                    break;
                }
            }
        }
        Ok(Pronunciation {
            language: self.language.clone(),
            provider: self.provider.clone(),
            syllables: result,
            phrase_marks,
        })
    }
    fn parse_ipa(&self, text: &str) -> Result<Vec<Syllable>> {
        let text = text.trim();
        ensure!(
            !text.contains(", /") && !text.contains("/,"),
            "multiple IPA variants; choose one using a bank dictionary override"
        );
        let text = text.trim_matches('/');
        let mut rest = text;
        let mut output = String::new();
        let mut ids = self
            .inventory
            .phones
            .iter()
            .map(|p| p.id.as_str())
            .chain(self.inventory.aliases.keys().map(String::as_str))
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| std::cmp::Reverse(id.len()));
        while !rest.is_empty() {
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() {
                rest = &rest[c.len_utf8()..];
                continue;
            }
            if c == '.' {
                output.push_str(" / ");
                rest = &rest[1..];
                continue;
            }
            if c == 'ˈ' || c == 'ˌ' {
                output.push_str(if c == 'ˈ' { " ' " } else { " , " });
                rest = &rest[c.len_utf8()..];
                continue;
            }
            let id = ids
                .iter()
                .find(|id| rest.starts_with(**id))
                .with_context(|| {
                    format!(
                        "IPA sequence {:?} is not in inventory {}; add an explicit alias or phone",
                        rest.chars().take(8).collect::<String>(),
                        self.inventory.inventory
                    )
                })?;
            output.push_str(id);
            output.push(' ');
            rest = &rest[id.len()..];
        }
        self.parse_output(&output)
    }
    fn parse_output(&self, text: &str) -> Result<Vec<Syllable>> {
        let mut result = vec![];
        for syllable in text.split('/') {
            let mut phones = vec![];
            let mut stress = None;
            for token in syllable.split_whitespace() {
                if ["'", "ˈ"].contains(&token) {
                    stress = Some(1);
                    continue;
                }
                if [",", "ˌ"].contains(&token) {
                    stress = Some(2);
                    continue;
                }
                let phone = if self.inventory.phone(token).is_none()
                    && token.len() > 1
                    && token.as_bytes().last().is_some_and(|b| b"012".contains(b))
                {
                    let stem = &token[..token.len() - 1];
                    if self.inventory.phone(stem).is_some() {
                        stress = Some(token.as_bytes()[token.len() - 1] - b'0');
                        stem
                    } else {
                        token
                    }
                } else {
                    token
                };
                phones.push(self.inventory.canonical(phone)?);
            }
            ensure!(!phones.is_empty(), "empty pronunciation syllable");
            if self.pronunciation.syllabification == "vowels" {
                let mut current = vec![];
                let mut nucleus = false;
                for p in phones {
                    if self.inventory.nucleus(&p) && nucleus {
                        result.push(Syllable {
                            phones: std::mem::take(&mut current),
                            stress,
                        });
                        nucleus = false;
                    }
                    nucleus |= self.inventory.nucleus(&p);
                    current.push(p);
                }
                if !current.is_empty() {
                    result.push(Syllable {
                        phones: current,
                        stress,
                    });
                }
            } else {
                result.push(Syllable { phones, stress });
            }
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Syllable {
    pub phones: Vec<String>,
    pub stress: Option<u8>,
}
#[derive(Clone, Debug)]
pub struct PhraseMark {
    pub after_syllable: usize,
    pub mark: String,
}
#[derive(Clone, Debug)]
pub struct Pronunciation {
    pub language: String,
    pub provider: String,
    pub syllables: Vec<Syllable>,
    pub phrase_marks: Vec<PhraseMark>,
}
enum LyricToken {
    Word(String),
    Mark(String),
}
fn lyric_tokens(text: &str) -> Vec<LyricToken> {
    let mut result = vec![];
    for token in text.split_whitespace() {
        if token.starts_with('@') || token == "~" {
            result.push(LyricToken::Word(token.into()));
            continue;
        }
        let mut word = String::new();
        for c in token.chars() {
            if ",.;:!?…。、，；：！？「」『』“”«»—–()[]{}".contains(c) {
                if !word.is_empty() {
                    result.push(LyricToken::Word(std::mem::take(&mut word)));
                }
                result.push(LyricToken::Mark(c.to_string()));
            } else {
                word.push(c);
            }
        }
        if !word.is_empty() {
            result.push(LyricToken::Word(word));
        }
    }
    result
}
pub trait PronunciationProvider {
    fn word(&self, context: &ContextProfile, word: &str) -> Result<Vec<Syllable>>;
}
struct CmuProvider;
impl PronunciationProvider for CmuProvider {
    fn word(&self, c: &ContextProfile, word: &str) -> Result<Vec<Syllable>> {
        let pronunciation = crate::voice::english_stressed(word).with_context(|| {
            format!("unknown English word {word:?}; add a dictionary override or manual phonemes")
        })?;
        c.parse_output(&pronunciation)
    }
}
struct ManualProvider;
impl PronunciationProvider for ManualProvider {
    fn word(&self, c: &ContextProfile, word: &str) -> Result<Vec<Syllable>> {
        bail!(
            "unknown pronunciation {word:?} for {} ({}); add voicebank dictionary WORD \\\"PHONES / PHONES\\\", language dictionary, or explicit @phone.phone notation",
            c.language,
            c.provider
        )
    }
}
struct RuleProvider;
impl PronunciationProvider for RuleProvider {
    fn word(&self, c: &ContextProfile, word: &str) -> Result<Vec<Syllable>> {
        let text = if c.pronunciation.case_sensitive {
            word.into()
        } else {
            word.to_lowercase()
        };
        let mut rules = c.pronunciation.rules.iter().collect::<Vec<_>>();
        rules.sort_by_key(|r| std::cmp::Reverse(r.text.len()));
        let mut rest = text.as_str();
        let mut output = String::new();
        while !rest.is_empty() {
            let rule = rules
                .iter()
                .find(|r| rest.starts_with(&r.text))
                .with_context(|| {
                    format!(
                        "no pronunciation rule for {:?} in {}; add a mapping or use manual phones",
                        rest.chars().take(12).collect::<String>(),
                        c.language
                    )
                })?;
            output.push_str(&rule.output);
            output.push(' ');
            rest = &rest[rule.text.len()..];
        }
        c.parse_output(&output)
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectPreferences {
    pub version: u32,
    pub language: Option<String>,
    pub inventory: Option<String>,
    pub provider: Option<String>,
}
pub fn project_preferences(root: &Path) -> Result<ProjectPreferences> {
    if root.join("language.toml").exists() {
        let p: ProjectPreferences =
            read_resource(&crate::codec::safe_path(root, "language.toml")?)?;
        ensure!(p.version == 1, "unsupported project language preferences");
        Ok(p)
    } else {
        Ok(ProjectPreferences {
            version: 1,
            ..Default::default()
        })
    }
}
pub fn app_config() -> crate::config::Config {
    crate::config::Watcher::default().load().unwrap_or_default()
}
pub fn default_selection(root: &Path, source: Option<&str>) -> Result<Selection> {
    let app = app_config();
    let project = project_preferences(root)?;
    let parsed = source.map(crate::language::parse).transpose()?;
    let requested = parsed
        .as_ref()
        .and_then(|s| s.language.clone())
        .or(project.language)
        .unwrap_or_else(|| {
            if app.voicebank.default_language != "auto" {
                app.voicebank.default_language.clone()
            } else {
                app.language.project_default.clone()
            }
        });
    let language = if requested == "auto" {
        locale::preferred()
            .into_iter()
            .find(|id| Profile::load(root, id).is_ok())
            .unwrap_or("en".into())
    } else {
        locale::normalize(&requested)?
    };
    let inventory = parsed
        .as_ref()
        .and_then(|s| s.inventory.clone())
        .or(project.inventory)
        .or_else(|| {
            (app.voicebank.default_inventory != "auto").then_some(app.voicebank.default_inventory)
        });
    let provider = parsed
        .as_ref()
        .and_then(|s| s.provider.clone())
        .or(project.provider)
        .or_else(|| {
            (app.language.pronunciation_default != "auto")
                .then_some(app.language.pronunciation_default)
        });
    Ok(Selection {
        language: Some(language),
        inventory,
        provider,
    })
}
pub fn bank_selection(bank: &crate::voice::Bank) -> Selection {
    Selection {
        language: Some(bank.language.clone().unwrap_or("en".into())),
        inventory: bank.inventory.clone(),
        provider: bank.provider.clone(),
    }
}
pub fn bank_context(root: &Path, bank: &crate::voice::Bank) -> Result<ContextProfile> {
    ContextProfile::load(root, &bank_selection(bank))
}
pub fn for_track(
    root: &Path,
    bank: &crate::voice::Bank,
    voice: &crate::language::Voice,
    track: &crate::language::Track,
    score: &crate::language::Score,
) -> Result<ContextProfile> {
    let dir = crate::codec::safe_path(root, &format!("{}/voicebank.toml", voice.bank))?
        .parent()
        .unwrap()
        .to_path_buf();
    let physical = bank_context(&dir, bank)?;
    let selected = track
        .language
        .as_deref()
        .or(voice.language.as_deref())
        .or(if bank.language.is_some() {
            score.language.as_deref()
        } else {
            Some("en")
        })
        .or(bank.language.as_deref())
        .unwrap_or("en");
    let language = locale::normalize(selected)?;
    let pronunciation = if language == physical.language {
        physical.pronunciation.clone()
    } else {
        Profile::load(root, &language).or_else(|_| Profile::load(&dir, &language))?
    };
    let inventory = if let Some(id) = track
        .inventory
        .as_ref()
        .or(voice.inventory.as_ref())
        .or(score.inventory.as_ref())
    {
        Profile::load(root, id).or_else(|_| Profile::load(&dir, id))?
    } else {
        physical.inventory
    };
    let provider = track
        .provider
        .as_ref()
        .or(voice.provider.as_ref())
        .filter(|p| p.as_str() != "auto")
        .cloned()
        .or_else(|| {
            if locale::normalize(bank.language.as_deref().unwrap_or("en")).ok()
                == Some(language.clone())
            {
                bank.provider.clone()
            } else {
                None
            }
        })
        .or_else(|| score.provider.clone())
        .unwrap_or_else(|| pronunciation.provider.clone());
    ensure!(
        ["cmudict", "dictionary", "rules", "manual"].contains(&provider.as_str()),
        "unknown provider"
    );
    ensure!(
        provider != "cmudict" || pronunciation.language == "en",
        "CMUdict is English-only; choose another provider"
    );
    Ok(ContextProfile {
        language,
        inventory,
        pronunciation,
        provider,
    })
}
pub fn save_profile(root: &Path, p: &Profile) -> Result<()> {
    p.validate()?;
    let folder = crate::codec::directory(root, "languages")?;
    let path = folder.join(format!("{}.toml", locale::normalize(&p.language)?));
    let mut stored = p.clone();
    if let Some(file) = &p.dictionary_file {
        stored.dictionary_file = Some(crate::lexicon::copy_into(
            p.lexicon
                .base
                .as_deref()
                .context("dictionary resource directory is missing")?,
            file,
            &folder,
        )?);
    }
    let text = toml::to_string_pretty(&stored)?;
    ensure!(text.len() <= 1_048_576, "profile exceeds resource limit");
    if path.exists() {
        let history = crate::codec::directory(root, "languages/history")?;
        let backup = crate::codec::unique(&history, "profile", "toml")?;
        fs::copy(&path, backup)?;
    }
    crate::project::atomic(&path, text.as_bytes())
}

pub fn resource_root() -> Option<PathBuf> {
    let config = app_config();
    config.language.resources.or_else(|| {
        crate::locale::settings_path()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("resources")))
    })
}
/// Discover filenames only. Dictionaries are parsed only when their language is selected.
pub fn discover(root: &Path) -> Result<Vec<String>> {
    let mut ids = BTreeSet::from([
        "en".into(),
        "de".into(),
        "fr".into(),
        "ja".into(),
        "la".into(),
    ]);
    let mut folders = vec![root.join("languages")];
    if let Some(resources) = resource_root() {
        folders.push(resources.join("languages"));
    }
    for folder in folders {
        if folder.is_dir() {
            for (i, entry) in fs::read_dir(folder)?.enumerate() {
                ensure!(i < 2048, "language pack limit 2048");
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("toml")
                    && let Some(id) = entry.path().file_stem().and_then(|s| s.to_str())
                    && let Ok(id) = locale::normalize(id)
                {
                    ids.insert(id);
                }
            }
        }
    }
    Ok(ids.into_iter().collect())
}
fn argument(args: &[String], index: usize) -> Result<&str> {
    args.get(index)
        .map(String::as_str)
        .context("missing argument; help language")
}
pub fn command(root: &Path, args: &[String]) -> Result<Vec<String>> {
    let op = argument(args, 0)?;
    if op == "reload" {
        return Ok(vec![
            "Language resources reloaded; re-evaluate to apply pronunciation changes".into(),
        ]);
    }
    if op == "list" {
        return Ok(discover(root)?
            .into_iter()
            .map(|id| format!("{id}: installed profile; help {id} for bundled guides"))
            .collect());
    }
    if op == "interface" {
        locale::save_preference("language", "interface", argument(args, 1)?)?;
        return Ok(vec![
            "Interface preference saved; singing language is unchanged".into(),
        ]);
    }
    if op == "default" {
        locale::save_preference("voicebank", "default_language", argument(args, 1)?)?;
        return Ok(vec![
            "Default bank language saved; existing banks keep their inventories".into(),
        ]);
    }
    if ["project", "project-provider", "project-inventory"].contains(&op) {
        let mut p = project_preferences(root)?;
        let value = argument(args, 1)?;
        match op {
            "project" => p.language = Some(locale::default_language(value)?),
            "project-inventory" => {
                p.inventory = if value == "auto" {
                    None
                } else {
                    Some(locale::normalize(value)?)
                }
            }
            _ => {
                ensure!(
                    ["auto", "manual", "dictionary", "rules", "cmudict"].contains(&value),
                    "unknown provider"
                );
                p.provider = (value != "auto").then_some(value.into());
            }
        }
        crate::project::atomic(
            &root.join("language.toml"),
            toml::to_string_pretty(&p)?.as_bytes(),
        )?;
        return Ok(vec![
            "Project language preferences saved; source-level settings still take precedence"
                .into(),
        ]);
    }
    if op == "install" {
        let source = Path::new(argument(args, 1)?).canonicalize()?;
        let dest = resource_root().context("no language resource directory")?;
        let mut pending = vec![];
        for folder in ["languages", "locales"] {
            let directory = source.join(folder);
            if !directory.is_dir() {
                continue;
            }
            for (i, entry) in fs::read_dir(directory)?.enumerate() {
                ensure!(i < 2048, "resource pack file limit");
                let entry = entry?;
                ensure!(
                    !entry.file_type()?.is_symlink(),
                    "resource packs cannot contain symlinks"
                );
                if !entry.file_type()?.is_file()
                    || entry.path().extension().and_then(|s| s.to_str()) != Some("toml")
                {
                    continue;
                }
                let file = entry.path();
                let tag = locale::normalize(
                    file.file_stem()
                        .and_then(|s| s.to_str())
                        .context("invalid resource filename")?,
                )?;
                if folder == "languages" {
                    let p: Profile = read_resource(&file)?;
                    p.validate()?;
                    if let Some(file) = &p.dictionary_file {
                        let input = crate::codec::safe_path(&source.join(folder), file)?;
                        ensure!(
                            input.metadata()?.len() <= 128 * 1024 * 1024,
                            "dictionary limit exceeded"
                        );
                        let target = dest.join(folder).join(file);
                        ensure!(!target.exists(), "dictionary destination already exists");
                        if !pending.iter().any(|(_, p)| p == &target) {
                            pending.push((input, target));
                        }
                    }
                    ensure!(
                        locale::normalize(&p.language)? == tag,
                        "profile filename/language mismatch"
                    );
                } else {
                    crate::locale::validate_catalog(&file, &tag)?;
                }
                let target = dest.join(folder).join(format!("{tag}.toml"));
                ensure!(
                    !target.exists(),
                    "resource already exists: {}; select a new pack location or edit it deliberately",
                    target.display()
                );
                pending.push((file, target));
            }
        }
        ensure!(
            !pending.is_empty(),
            "pack needs languages/*.toml and/or locales/*.toml"
        );
        // Validate all destinations before writing; failed installs remove only files
        // created by this invocation. Existing packs are never replaced.
        let mut targets = BTreeSet::new();
        for (_, target) in &pending {
            ensure!(
                targets.insert(target.clone()),
                "resource pack destination collision"
            );
        }
        fs::create_dir_all(&dest)?;
        let mut created = Vec::new();
        let installed: Result<()> = (|| {
            for (source, target) in &pending {
                let relative_parent = target.parent().unwrap().strip_prefix(&dest)?;
                crate::codec::directory(&dest, &crate::codec::script_path(relative_parent))?;
                let mut out = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(target)?;
                created.push(target.clone());
                std::io::copy(&mut fs::File::open(source)?, &mut out)?;
                out.sync_all()?;
            }
            Ok(())
        })();
        if let Err(error) = installed {
            let mut leftovers = Vec::new();
            for file in created.into_iter().rev() {
                if let Err(cleanup) = fs::remove_file(&file) {
                    leftovers.push(format!("{}: {cleanup}", file.display()));
                }
            }
            if !leftovers.is_empty() {
                return Err(error.context(format!(
                    "incomplete install cleanup: {}",
                    leftovers.join("; ")
                )));
            }
            return Err(error);
        }
        return Ok(vec![format!(
            "Installed {} resource files; no downloads or voice recordings were inferred",
            pending.len()
        )]);
    }
    let id = locale::normalize(argument(args, 1)?)?;
    if op == "inspect" {
        let p = Profile::load(root, &id)?;
        return Ok(vec![
            format!(
                "{}: language {}, inventory {}, provider {} v{}, {} phones",
                p.name,
                p.language,
                p.inventory,
                p.provider,
                p.provider_version,
                p.phones.len()
            ),
            format!(
                "Capabilities: {}. License: {}. {}",
                p.capabilities.join(", "),
                p.license,
                p.attribution
            ),
        ]);
    }
    if op == "pronounce" {
        let c = ContextProfile::load(
            root,
            &Selection {
                language: Some(id),
                ..Default::default()
            },
        )?;
        let p = c.pronounce(argument(args, 2)?, &Default::default())?;
        return Ok(p
            .syllables
            .into_iter()
            .enumerate()
            .map(|(i, s)| format!("{}: {} (stress {:?})", i + 1, s.phones.join(" "), s.stress))
            .collect());
    }
    let mut p = if op == "new" {
        ensure!(
            !root.join(format!("languages/{id}.toml")).exists(),
            "profile already exists"
        );
        Profile {
            language: id.clone(),
            inventory: id.clone(),
            name: argument(args, 2)?.into(),
            ..Default::default()
        }
    } else {
        Profile::load(root, &id)?
    };
    match op {
        "new" => {}
        "phone" => {
            let phone = argument(args, 2)?;
            ensure!(identifier(phone), "invalid phoneme identifier");
            let kind = argument(args, 3)?;
            ensure!(
                ["vowel", "consonant", "syllabic"].contains(&kind),
                "phone kind vowel|consonant|syllabic"
            );
            let voicing = argument(args, 4)?;
            ensure!(
                ["voiced", "unvoiced"].contains(&voicing),
                "voicing voiced|unvoiced"
            );
            let row = Phone {
                id: phone.into(),
                vowel: kind == "vowel",
                syllabic: kind == "syllabic",
                voiced: voicing == "voiced",
                description: argument(args, 5)?.into(),
                prompt: argument(args, 6)?.into(),
                ..Default::default()
            };
            if let Some(old) = p.phones.iter_mut().find(|p| p.id == phone) {
                *old = row;
            } else {
                p.phones.push(row);
            }
        }
        "phone-formants" => {
            let phone = argument(args, 2)?;
            let f = [
                argument(args, 3)?.parse()?,
                argument(args, 4)?.parse()?,
                argument(args, 5)?.parse()?,
            ];
            p.phones
                .iter_mut()
                .find(|p| p.id == phone)
                .context("add this phone first")?
                .formants = Some(f);
        }
        "phone-shape" => {
            let phone = argument(args, 2)?;
            let row = p
                .phones
                .iter_mut()
                .find(|p| p.id == phone)
                .context("add this phone first")?;
            row.manner = argument(args, 3)?.into();
            row.noise_cutoff = argument(args, 4)?.parse()?;
            row.nasal = argument(args, 5)?.parse()?;
        }
        "phone-end-formants" => {
            let phone = argument(args, 2)?;
            let row = p
                .phones
                .iter_mut()
                .find(|p| p.id == phone)
                .context("add this phone first")?;
            row.formants_end = Some([
                argument(args, 3)?.parse()?,
                argument(args, 4)?.parse()?,
                argument(args, 5)?.parse()?,
            ]);
        }
        "alias" => {
            p.aliases
                .insert(argument(args, 2)?.into(), argument(args, 3)?.into());
        }
        "dictionary" => {
            p.dictionary
                .insert(argument(args, 2)?.into(), argument(args, 3)?.into());
        }
        "dictionary-file" => {
            let file = Path::new(argument(args, 2)?).canonicalize()?;
            let folder = crate::codec::directory(root, "languages")?;
            let relative = crate::lexicon::copy_into(
                file.parent().context("dictionary needs parent")?,
                file.file_name()
                    .unwrap()
                    .to_str()
                    .context("dictionary filename must be UTF-8")?,
                &folder,
            )?;
            p.dictionary_file = Some(relative);
            p.lexicon.base = Some(folder);
            p.dictionary_format = args.get(3).map(String::as_str).unwrap_or("phones").into();
        }
        "dictionary-import" => {
            let file = Path::new(argument(args, 2)?);
            ensure!(
                fs::metadata(file)?.len() <= 1_048_576,
                "dictionary import limit 1 MiB"
            );
            for row in fs::read_to_string(file)?
                .lines()
                .filter(|s| !s.trim().is_empty() && !s.starts_with('#'))
            {
                let (word, phones) = row.split_once('\t').context(
                    "dictionary TSV requires WORD<TAB>space-separated phones / syllables",
                )?;
                p.dictionary.insert(word.into(), phones.into());
            }
        }
        "rule" => {
            p.rules.push(Rule {
                text: argument(args, 2)?.into(),
                output: argument(args, 3)?.into(),
            });
        }
        "set" => {
            let key = argument(args, 2)?;
            let value = argument(args, 3)?;
            match key {
                "provider" => p.provider = value.into(),
                "inventory" => {
                    ensure!(identifier(value), "invalid inventory identifier");
                    p.inventory = value.into();
                }
                "license" => p.license = value.into(),
                "attribution" => p.attribution = value.into(),
                "syllabification" => p.syllabification = value.into(),
                "tokenization" => p.tokenization = value.into(),
                "consonant-fraction" => p.timing.consonant_fraction = value.parse()?,
                "consonant-max" => p.timing.consonant_max = value.parse()?,
                "crossfade-ms" => p.timing.crossfade_ms = value.parse()?,
                "consonant-syllables" => p.timing.allow_consonant_syllables = value.parse()?,
                _ => bail!("unknown profile setting {key}"),
            }
        }
        _ => bail!(
            "language list|interface|default|project|project-provider|project-inventory|install|new|phone|phone-formants|alias|dictionary|dictionary-import|rule|set|inspect|pronounce; help language"
        ),
    }
    save_profile(root, &p)?;
    Ok(vec![format!(
        "Saved project language profile {} without changing existing banks",
        p.language
    )])
}

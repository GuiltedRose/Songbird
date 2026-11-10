//! Offline locale detection and small, layered UI catalogs. No audio-thread work.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn auto() -> String {
    "auto".into()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub interface: String,
    pub project_default: String,
    pub pronunciation_default: String,
    pub resources: Option<PathBuf>,
    pub font_file: Option<PathBuf>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            interface: auto(),
            project_default: auto(),
            pronunciation_default: auto(),
            resources: None,
            font_file: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BankPreferences {
    pub default_language: String,
    pub default_inventory: String,
}
impl Default for BankPreferences {
    fn default() -> Self {
        Self {
            default_language: auto(),
            default_inventory: auto(),
        }
    }
}
pub fn normalize(value: &str) -> Result<String> {
    let v = value.trim();
    let alias = match v.to_ascii_lowercase().as_str() {
        "english" => Some("en"),
        "german" => Some("de"),
        "french" => Some("fr"),
        "japanese" => Some("ja"),
        "latin" => Some("la"),
        "c" | "posix" => Some("en"),
        _ => None,
    };
    if let Some(v) = alias {
        return Ok(v.into());
    }
    let v = v.split('.').next().unwrap_or(v);
    if v.eq_ignore_ascii_case("C") || v.eq_ignore_ascii_case("POSIX") {
        return Ok("en".into());
    }
    let (v, modifier) = v.split_once('@').unwrap_or((v, ""));
    let v = v.replace('_', "-");
    ensure!(
        !v.is_empty()
            && v.len() <= 63
            && v.split('-').all(|s| !s.is_empty()
                && s.len() <= 8
                && s.bytes().all(|b| b.is_ascii_alphanumeric())),
        "invalid language identifier {value:?}"
    );
    let mut result = Vec::new();
    for (i, s) in v.split('-').enumerate() {
        result.push(if i == 0 {
            s.to_ascii_lowercase()
        } else if s.len() == 4 {
            let mut s = s.to_ascii_lowercase();
            s[..1].make_ascii_uppercase();
            s
        } else if s.len() == 2 || s.bytes().all(|c| c.is_ascii_digit()) {
            s.to_ascii_uppercase()
        } else {
            s.to_ascii_lowercase()
        });
    }
    if modifier.eq_ignore_ascii_case("latin") && !result.iter().any(|s| s == "Latn") {
        result.insert(1, "Latn".into());
    }
    Ok(result.join("-"))
}
pub fn fallback(tag: &str) -> Vec<String> {
    let mut result = vec![];
    if let Ok(mut value) = normalize(tag) {
        loop {
            result.push(value.clone());
            if let Some(i) = value.rfind('-') {
                value.truncate(i);
            } else {
                break;
            }
        }
    }
    if !result.iter().any(|s| s == "en") {
        result.push("en".into());
    }
    result
}
pub fn linux_locale(all: Option<&str>, messages: Option<&str>, lang: Option<&str>) -> String {
    all.filter(|s| !s.trim().is_empty())
        .or(messages.filter(|s| !s.trim().is_empty()))
        .or(lang.filter(|s| !s.trim().is_empty()))
        .and_then(|s| normalize(s).ok())
        .unwrap_or("en".into())
}
pub fn utf16_languages(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|c| *c == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| String::from_utf16(s).ok().and_then(|s| normalize(&s).ok()))
        .collect()
}
#[cfg(windows)]
pub fn preferred() -> Vec<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserPreferredUILanguages(
            flags: u32,
            count: *mut u32,
            buffer: *mut u16,
            size: *mut u32,
        ) -> i32;
    }
    let mut count = 0;
    let mut size = 0;
    // Native language-name mode; query the bounded required UTF-16 buffer first.
    unsafe {
        if GetUserPreferredUILanguages(8, &mut count, std::ptr::null_mut(), &mut size) == 0
            || size > 32768
        {
            return vec!["en".into()];
        }
    }
    let mut buffer = vec![0u16; size as usize];
    unsafe {
        if GetUserPreferredUILanguages(8, &mut count, buffer.as_mut_ptr(), &mut size) == 0 {
            return vec!["en".into()];
        }
    }
    let result = utf16_languages(&buffer);
    if result.is_empty() {
        vec!["en".into()]
    } else {
        result
    }
}
#[cfg(target_os = "macos")]
pub fn preferred() -> Vec<String> {
    use std::ffi::{c_char, c_void};
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> *const c_void;
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: *const c_void);
    }
    let mut result = vec![];
    // The copied CF array is owned here; its string elements remain borrowed until release.
    unsafe {
        let array = CFLocaleCopyPreferredLanguages();
        if !array.is_null() {
            for i in 0..CFArrayGetCount(array).clamp(0, 32) {
                let item = CFArrayGetValueAtIndex(array, i);
                let mut buffer = [0u8; 256];
                if !item.is_null()
                    && CFStringGetCString(item, buffer.as_mut_ptr().cast(), 256, 0x08000100) != 0
                {
                    if let Ok(text) = std::ffi::CStr::from_ptr(buffer.as_ptr().cast()).to_str() {
                        if let Ok(tag) = normalize(text) {
                            result.push(tag);
                        }
                    }
                }
            }
            CFRelease(array);
        }
    }
    if result.is_empty() {
        vec!["en".into()]
    } else {
        result
    }
}
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn preferred() -> Vec<String> {
    vec![linux_locale(
        std::env::var("LC_ALL").ok().as_deref(),
        std::env::var("LC_MESSAGES").ok().as_deref(),
        std::env::var("LANG").ok().as_deref(),
    )]
}
pub fn default_language(request: &str) -> Result<String> {
    if request == "auto" {
        Ok(preferred().into_iter().next().unwrap_or("en".into()))
    } else {
        normalize(request)
    }
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Resource {
    language: String,
    messages: BTreeMap<String, String>,
}
#[derive(Clone)]
pub struct Catalog {
    pub language: String,
    messages: BTreeMap<String, String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self::load("en", None).expect("bundled English catalog")
    }
}
impl Catalog {
    pub fn load(request: &str, resources: Option<&Path>) -> Result<Self> {
        let tags = if request == "auto" {
            preferred()
        } else {
            vec![normalize(request)?]
        };
        let mut messages = BTreeMap::new();
        let mut selected = "en".to_owned();
        let english: Resource = toml::from_str(include_str!("../resources/locales/en.toml"))?;
        messages.extend(english.messages);
        let mut candidates = vec![];
        for tag in tags {
            for c in fallback(&tag) {
                if c == "en" && tag.split('-').next() != Some("en") {
                    continue;
                }
                if !candidates.contains(&c) {
                    candidates.push(c);
                }
            }
        }
        if !candidates.contains(&"en".into()) {
            candidates.push("en".into());
        }
        for tag in candidates.iter().rev() {
            let builtin = match tag.as_str() {
                "en" => Some(include_str!("../resources/locales/en.toml")),
                "de" => Some(include_str!("../resources/locales/de.toml")),
                "fr" => Some(include_str!("../resources/locales/fr.toml")),
                "ja" => Some(include_str!("../resources/locales/ja.toml")),
                _ => None,
            };
            if let Some(text) = builtin {
                let r: Resource = toml::from_str(text)?;
                messages.extend(r.messages.into_iter().filter(|(_, v)| !v.trim().is_empty()));
                selected = tag.clone();
            }
            if let Some(dir) = resources {
                let path = dir.join("locales").join(format!("{tag}.toml"));
                if path.exists() {
                    let r: Resource = crate::phonetics::read_resource(&path)?;
                    ensure!(
                        normalize(&r.language)? == *tag,
                        "catalog language does not match filename"
                    );
                    messages.extend(r.messages.into_iter().filter(|(_, v)| !v.trim().is_empty()));
                    selected = tag.clone();
                }
            }
        }
        Ok(Self {
            language: selected,
            messages,
        })
    }
    pub fn text(&self, key: &str) -> String {
        self.messages
            .get(key)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| key.into())
    }
    pub fn format(&self, key: &str, values: &[String]) -> String {
        let mut output = self.text(key);
        for (i, value) in values.iter().enumerate() {
            output = output.replace(&format!("{{{i}}}"), value);
        }
        output
    }
    pub fn message(&self, text: &str) -> String {
        if let Some(rest) = text.strip_prefix("ERROR: ") {
            return format!("{} {}", self.text("ERROR:"), self.message(rest));
        }
        if let Some(value) = self.messages.get(text) {
            return value.clone();
        }
        self.messages
            .iter()
            .filter(|(key, _)| key.ends_with(':') || key.ends_with(' '))
            .filter(|(key, _)| text.starts_with(key.as_str()))
            .max_by_key(|(key, _)| key.len())
            .map(|(key, value)| format!("{value}{}", &text[key.len()..]))
            .unwrap_or_else(|| text.into())
    }
}
pub fn validate_catalog(path: &Path, tag: &str) -> Result<()> {
    let r: Resource = crate::phonetics::read_resource(path)?;
    ensure!(
        normalize(&r.language)? == tag,
        "catalog filename/language mismatch"
    );
    ensure!(r.messages.len() <= 4096, "translation key limit 4096");
    Ok(())
}
pub fn settings_path() -> Result<PathBuf> {
    crate::config::Watcher::default()
        .path
        .context("no user config directory available; set SONGBIRD_CONFIG")
}
pub fn save_preference(group: &str, key: &str, value: &str) -> Result<()> {
    ensure!(
        [
            ("language", "interface"),
            ("language", "project_default"),
            ("language", "pronunciation_default"),
            ("voicebank", "default_language"),
            ("voicebank", "default_inventory")
        ]
        .contains(&(group, key)),
        "unknown language preference"
    );
    if value != "auto"
        && !(key == "pronunciation_default"
            && ["manual", "dictionary", "rules", "cmudict"].contains(&value))
    {
        normalize(value)?;
    }
    let path = settings_path()?;
    let mut table = if path.exists() {
        ensure!(
            std::fs::metadata(&path)?.len() <= 65536,
            "config exceeds 64 KiB"
        );
        toml::from_str::<toml::Value>(&std::fs::read_to_string(&path)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    let table_root = table
        .as_table_mut()
        .context("config must be a TOML table")?;
    let entry = table_root
        .entry(group)
        .or_insert(toml::Value::Table(Default::default()));
    entry
        .as_table_mut()
        .context("language preferences must be a table")?
        .insert(key.into(), toml::Value::String(value.into()));
    let text = toml::to_string_pretty(&table)?;
    crate::config::Config::parse(&text)?;
    let parent = path.parent().context("config needs parent")?;
    std::fs::create_dir_all(parent)?;
    crate::project::atomic(&path, text.as_bytes())
}
pub fn font(
    ctx: &eframe::egui::Context,
    config: &crate::config::Config,
    needed: bool,
) -> Result<bool> {
    let candidates = if let Some(file) = &config.language.font_file {
        vec![file.clone()]
    } else if needed {
        vec![
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc"),
            PathBuf::from("/System/Library/Fonts/Hiragino Sans.ttc"),
            PathBuf::from("C:/Windows/Fonts/YuGothR.ttc"),
        ]
    } else {
        return Ok(false);
    };
    let Some(path) = candidates.into_iter().find(|p| p.is_file()) else {
        ensure!(
            config.language.font_file.is_none(),
            "configured language font is missing"
        );
        return Ok(false);
    };
    ensure!(
        std::fs::metadata(&path)?.len() <= 32 * 1024 * 1024,
        "language font exceeds 32 MiB"
    );
    let mut fonts = eframe::egui::FontDefinitions::default();
    fonts.font_data.insert(
        "language-fallback".into(),
        eframe::egui::FontData::from_owned(std::fs::read(path)?).into(),
    );
    for family in [
        eframe::egui::FontFamily::Proportional,
        eframe::egui::FontFamily::Monospace,
    ] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("language-fallback".into());
    }
    ctx.set_fonts(fonts);
    Ok(true)
}

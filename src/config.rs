//! User appearance preferences, separate from project and audio state.
use anyhow::{Context, Result, ensure};
use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};
use serde::Deserialize;
use std::{
    fs,
    io::{ErrorKind, Read},
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub appearance: Appearance,
    pub theme: Theme,
    pub language: crate::locale::Preferences,
    pub voicebank: crate::locale::BankPreferences,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: Appearance::default(),
            theme: Theme::default(),
            language: Default::default(),
            voicebank: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub font_size: f32,
    pub editor_font_size: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub show_tracks: bool,
    pub show_console: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: 14.,
            editor_font_size: 14.,
            window_width: 1120.,
            window_height: 780.,
            show_tracks: true,
            show_console: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    #[default]
    Dark,
    Light,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub base: Base,
    pub background: Option<String>,
    pub panel: Option<String>,
    pub text: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub selection: Option<String>,
    pub keyword: Option<String>,
    pub string: Option<String>,
    pub number: Option<String>,
    pub waveform: Option<String>,
    pub recording: Option<String>,
}
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub keyword: Color32,
    pub string: Color32,
    pub number: Color32,
    pub waveform: Color32,
    pub recording: Color32,
}
fn color(value: &Option<String>, fallback: Color32) -> Result<Color32> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    ensure!(
        value.len() == 7
            && value.starts_with('#')
            && value[1..].bytes().all(|b| b.is_ascii_hexdigit()),
        "colors must use #RRGGBB, got {value:?}"
    );
    Ok(Color32::from_rgb(
        u8::from_str_radix(&value[1..3], 16)?,
        u8::from_str_radix(&value[3..5], 16)?,
        u8::from_str_radix(&value[5..7], 16)?,
    ))
}
impl Config {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= 65536, "config exceeds 64 KiB");
        let config: Self = toml::from_str(source).context("invalid Songbird config")?;
        ensure!(
            config.version == 1,
            "unsupported config version {}",
            config.version
        );
        let a = &config.appearance;
        ensure!(
            [a.font_size, a.editor_font_size]
                .iter()
                .all(|n| n.is_finite() && (10.0..=32.).contains(n)),
            "font sizes must be between 10 and 32"
        );
        ensure!(
            a.window_width.is_finite()
                && (640.0..=3840.).contains(&a.window_width)
                && a.window_height.is_finite()
                && (480.0..=2160.).contains(&a.window_height),
            "window dimensions must be 640..3840 by 480..2160"
        );
        for value in [
            &config.language.interface,
            &config.language.project_default,
            &config.voicebank.default_language,
            &config.voicebank.default_inventory,
        ] {
            if value != "auto" {
                crate::locale::normalize(value)?;
            }
        }
        ensure!(
            ["auto", "cmudict", "dictionary", "manual", "rules"]
                .contains(&config.language.pronunciation_default.as_str()),
            "unsupported pronunciation_default"
        );
        config.palette()?;
        Ok(config)
    }
    pub fn palette(&self) -> Result<Palette> {
        let t = &self.theme;
        let dark = matches!(t.base, Base::Dark);
        let hex = |n: u32| Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8);
        Ok(Palette {
            background: color(&t.background, hex(if dark { 0x101615 } else { 0xf8faf9 }))?,
            panel: color(&t.panel, hex(if dark { 0x1a2421 } else { 0xe8efec }))?,
            text: color(&t.text, hex(if dark { 0xe0ebe6 } else { 0x20382e }))?,
            muted: color(&t.muted, hex(if dark { 0x879a90 } else { 0x53675d }))?,
            accent: color(&t.accent, hex(if dark { 0x7de2be } else { 0x14664a }))?,
            selection: color(&t.selection, hex(if dark { 0x315e4f } else { 0xb7dfcf }))?,
            keyword: color(&t.keyword, hex(if dark { 0x70cdb5 } else { 0x17624d }))?,
            string: color(&t.string, hex(if dark { 0xd1b878 } else { 0x825b12 }))?,
            number: color(&t.number, hex(if dark { 0xb69be6 } else { 0x7141a1 }))?,
            waveform: color(&t.waveform, hex(if dark { 0x5fb9a0 } else { 0x216d55 }))?,
            recording: color(&t.recording, hex(if dark { 0xff7e86 } else { 0xae2337 }))?,
        })
    }
    pub fn apply(&self, ctx: &egui::Context) -> Result<()> {
        let p = self.palette()?;
        let mut v = match self.theme.base {
            Base::Dark => egui::Visuals::dark(),
            Base::Light => egui::Visuals::light(),
        };
        v.panel_fill = p.panel;
        v.window_fill = p.panel;
        v.extreme_bg_color = p.background;
        v.code_bg_color = p.background;
        v.override_text_color = Some(p.text);
        v.weak_text_color = Some(p.muted);
        v.selection.bg_fill = p.selection;
        v.selection.stroke.color = p.text;
        v.widgets.active.fg_stroke.color = p.accent;
        v.widgets.hovered.fg_stroke.color = p.accent;
        v.hyperlink_color = p.accent;
        ctx.set_visuals(v);
        ctx.style_mut(|style| {
            for (kind, scale) in [
                (TextStyle::Body, 1.),
                (TextStyle::Button, 1.),
                (TextStyle::Small, 0.85),
                (TextStyle::Heading, 1.35),
            ] {
                style.text_styles.insert(
                    kind,
                    FontId::new(self.appearance.font_size * scale, FontFamily::Proportional),
                );
            }
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::monospace(self.appearance.editor_font_size),
            );
        });
        Ok(())
    }
}
// XDG relative paths are invalid; fall back to the user's home directory.
#[cfg(any(all(not(windows), not(target_os = "macos")), test))]
fn resolve(
    explicit: Option<PathBuf>,
    xdg: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit
        .or_else(|| {
            xdg.filter(|p| p.is_absolute())
                .map(|p| p.join("songbird/config.toml"))
        })
        .or_else(|| {
            home.filter(|p| p.is_absolute())
                .map(|p| p.join(".config/songbird/config.toml"))
        })
}
#[cfg(any(windows, test))]
fn resolve_windows(
    explicit: Option<PathBuf>,
    appdata: Option<PathBuf>,
    profile: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit
        .or_else(|| {
            appdata
                .filter(|p| p.is_absolute())
                .map(|p| p.join("Songbird/config.toml"))
        })
        .or_else(|| {
            profile
                .filter(|p| p.is_absolute())
                .map(|p| p.join("AppData/Roaming/Songbird/config.toml"))
        })
}
#[cfg(any(target_os = "macos", test))]
fn resolve_macos(explicit: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| {
        home.filter(|p| p.is_absolute())
            .map(|p| p.join("Library/Application Support/Songbird/config.toml"))
    })
}
#[derive(PartialEq, Eq)]
enum Stamp {
    Absent,
    File(Option<SystemTime>, u64, u64),
    Error(ErrorKind),
}
fn stamp(path: &std::path::Path) -> Stamp {
    match fs::metadata(path) {
        Ok(m) => {
            #[cfg(unix)]
            let identity = {
                use std::os::unix::fs::MetadataExt;
                m.ino()
            };
            #[cfg(not(unix))]
            let identity = 0;
            Stamp::File(m.modified().ok(), m.len(), identity)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Stamp::Absent,
        Err(e) => Stamp::Error(e.kind()),
    }
}
pub struct Watcher {
    pub path: Option<PathBuf>,
    explicit: bool,
    last: Option<Stamp>,
    checked: Instant,
}
impl Default for Watcher {
    fn default() -> Self {
        let explicit = std::env::var_os("SONGBIRD_CONFIG").map(PathBuf::from);
        #[cfg(windows)]
        let path = resolve_windows(
            explicit.clone(),
            std::env::var_os("APPDATA").map(PathBuf::from),
            std::env::var_os("USERPROFILE").map(PathBuf::from),
        );
        #[cfg(target_os = "macos")]
        let path = resolve_macos(
            explicit.clone(),
            std::env::var_os("HOME").map(PathBuf::from),
        );
        #[cfg(all(not(windows), not(target_os = "macos")))]
        let path = resolve(
            explicit.clone(),
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        );
        Self {
            path,
            explicit: explicit.is_some(),
            last: None,
            checked: Instant::now(),
        }
    }
}
impl Watcher {
    pub fn load(&mut self) -> Result<Config> {
        let Some(path) = &self.path else {
            return Ok(Config::default());
        };
        self.last = Some(stamp(path));
        self.checked = Instant::now();
        if let Ok(meta) = fs::metadata(path) {
            ensure!(meta.is_file(), "config must be a regular file");
        }
        let mut f = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound && !self.explicit => {
                return Ok(Config::default());
            }
            Err(e) => {
                return Err(e).with_context(|| format!("cannot read config {}", path.display()));
            }
        };
        ensure!(f.metadata()?.is_file(), "config must be a regular file");
        let mut source = String::new();
        Read::by_ref(&mut f)
            .take(65537)
            .read_to_string(&mut source)?;
        Config::parse(&source).with_context(|| format!("config {}", path.display()))
    }
    pub fn poll(&mut self) -> Option<Result<Config>> {
        if self.checked.elapsed() < Duration::from_millis(500) {
            return None;
        }
        self.checked = Instant::now();
        let path = self.path.as_ref()?;
        let next = stamp(path);
        if self.last.as_ref() == Some(&next) {
            return None;
        }
        Some(self.load())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_example() {
        Config::parse("").unwrap();
        let c = Config::parse(include_str!("../config.example.toml")).unwrap();
        assert_eq!(c.appearance.editor_font_size, 15.);
        assert_eq!(
            c.palette().unwrap().accent,
            Color32::from_rgb(125, 226, 190)
        );
    }
    #[test]
    fn appearance_applies_to_egui() {
        let c=Config::parse("[theme]\nbase='light'\naccent='#112233'\npanel='#abcdef'\n[appearance]\nfont_size=18.0\neditor_font_size=20.0").unwrap();
        let ctx = egui::Context::default();
        c.apply(&ctx).unwrap();
        let style = ctx.style();
        assert!(!style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, Color32::from_rgb(171, 205, 239));
        assert_eq!(style.visuals.hyperlink_color, Color32::from_rgb(17, 34, 51));
        assert_eq!(style.text_styles[&TextStyle::Monospace].size, 20.);
    }
    #[test]
    fn atomic_saves_and_deletion_are_detected() {
        let file = crate::codec::unique(&std::env::temp_dir(), "songbird-theme", "toml").unwrap();
        fs::write(&file, "[theme]\nbase='dark'\n").unwrap();
        let mut w = Watcher {
            path: Some(file.clone()),
            explicit: false,
            last: None,
            checked: Instant::now(),
        };
        w.load().unwrap();
        let swap = file.with_extension("swap");
        fs::write(&swap, "[theme]\nbase='light'").unwrap();
        fs::rename(swap, &file).unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(matches!(w.poll().unwrap().unwrap().theme.base, Base::Light));
        fs::remove_file(&file).unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(matches!(w.poll().unwrap().unwrap().theme.base, Base::Dark));
    }
    #[test]
    fn invalid_settings() {
        for s in [
            "version=2",
            "unknown=true",
            "[theme]\naccent='red'",
            "[theme]\naccent='#１２３'",
            "[theme]\nbase='purple'",
            "[appearance]\nfont_size=nan",
            "[appearance]\neditor_font_size=100",
            "[appearance]\nwindow_width=1",
        ] {
            assert!(Config::parse(s).is_err(), "{s}");
        }
    }
    #[test]
    fn location_precedence() {
        let root = std::env::temp_dir();
        let explicit = root.join("custom/theme.toml");
        let xdg = root.join("xdg");
        let home = root.join("home/user");
        assert_eq!(
            resolve(
                Some(explicit.clone()),
                Some(xdg.clone()),
                Some(home.clone())
            ),
            Some(explicit)
        );
        assert_eq!(
            resolve(None, Some(xdg.clone()), Some(home.clone())),
            Some(xdg.join("songbird/config.toml"))
        );
        assert_eq!(
            resolve(None, Some("relative".into()), Some(home.clone())),
            Some(home.join(".config/songbird/config.toml"))
        );
    }
    #[test]
    fn macos_location_precedence() {
        let home = std::env::temp_dir();
        let explicit = home.join("theme.toml");
        assert_eq!(
            resolve_macos(Some(explicit.clone()), Some(home.clone())),
            Some(explicit)
        );
        assert_eq!(
            resolve_macos(None, Some(home.clone())),
            Some(home.join("Library/Application Support/Songbird/config.toml"))
        );
        assert_eq!(resolve_macos(None, None), None);
    }
    #[test]
    fn windows_location_precedence() {
        let root = std::env::temp_dir();
        let explicit = root.join("theme.toml");
        let appdata = root.join("Roaming");
        let profile = root.join("User");
        assert_eq!(
            resolve_windows(
                Some(explicit.clone()),
                Some(appdata.clone()),
                Some(profile.clone())
            ),
            Some(explicit)
        );
        assert_eq!(
            resolve_windows(None, Some(appdata.clone()), Some(profile.clone())),
            Some(appdata.join("Songbird/config.toml"))
        );
        assert_eq!(
            resolve_windows(None, Some("relative".into()), Some(profile.clone())),
            Some(profile.join("AppData/Roaming/Songbird/config.toml"))
        );
        assert_eq!(resolve_windows(None, None, None), None);
    }

    #[test]
    fn reload_recovers_after_invalid_edit() {
        let file = crate::codec::unique(&std::env::temp_dir(), "songbird-config", "toml").unwrap();
        fs::write(&file, "[theme]\naccent='#112233'").unwrap();
        let mut w = Watcher {
            path: Some(file.clone()),
            explicit: true,
            last: None,
            checked: Instant::now(),
        };
        let mut current = w.load().unwrap();
        assert_eq!(
            current.palette().unwrap().accent,
            Color32::from_rgb(17, 34, 51)
        );
        fs::write(&file, "invalid config").unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().unwrap().is_err());
        assert_eq!(
            current.palette().unwrap().accent,
            Color32::from_rgb(17, 34, 51)
        );
        fs::write(&file, "[theme]\nbase='light'\naccent='#445566'").unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        current = w.poll().unwrap().unwrap();
        assert_eq!(
            current.palette().unwrap().accent,
            Color32::from_rgb(68, 85, 102)
        );
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().is_none());
        fs::remove_file(file).unwrap();
    }
}

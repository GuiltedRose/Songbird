//! User appearance preferences, separate from project and audio state.
use anyhow::{Context, Result, ensure};
use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Read},
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub appearance: Appearance,
    pub theme: Theme,
    pub language: crate::locale::Preferences,
    pub voicebank: crate::locale::BankPreferences,
    pub recording: crate::recording::Settings,
    pub storage: crate::storage::Limits,
    pub audio: AudioPreferences,
    pub session: Session,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: Appearance::default(),
            theme: Theme::default(),
            language: Default::default(),
            voicebank: Default::default(),
            recording: Default::default(),
            storage: Default::default(),
            audio: Default::default(),
            session: Default::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Session {
    pub restore_last_project: bool,
    pub last_project: Option<PathBuf>,
}
impl Default for Session {
    fn default() -> Self {
        Self { restore_last_project: true, last_project: None }
    }
}
impl Session {
    /// Startup only: never replaces an editor document or recovers/writes drafts.
    pub fn restore(&self) -> Result<Option<crate::project::Project>> {
        if !self.restore_last_project {
            return Ok(None);
        }
        self.last_project.as_deref().map(|path| {
            ensure!(path.is_absolute(), "last project path must be absolute");
            crate::project::Project::open(path)
                .with_context(|| format!("cannot restore project {}", path.display()))
        }).transpose()
    }
}

/// Merge into the latest config, retaining the on-disk restore preference.
pub fn save_last_project(path: &std::path::Path, project: &std::path::Path) -> Result<Session> {
    if let Ok(meta) = fs::metadata(path) {
        ensure!(meta.is_file() && meta.len() <= 65536,
            "config must be a regular file of at most 64 KiB");
    }
    let mut session = match fs::read_to_string(path) {
        Ok(source) => Config::parse(&source)?.session,
        Err(e) if e.kind() == ErrorKind::NotFound => Session::default(),
        Err(e) => return Err(e.into()),
    };
    session.last_project = Some(project.canonicalize()?);
    save_section(path, "session", toml::Value::try_from(&session)?)?;
    Ok(session)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AudioPreferences {
    pub input: Option<String>,
    pub output: Option<String>,
    pub channels: u16,
    pub format: String,
}
impl Default for AudioPreferences {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            channels: 1,
            format: "flac".into(),
        }
    }
}
/// Merge only audio preferences; invalid existing files are never overwritten.
pub fn save_audio(path: &std::path::Path, audio: &AudioPreferences) -> Result<()> {
    save_section(path, "audio", toml::Value::try_from(audio)?)
}

pub fn save_theme(path: &std::path::Path, theme: &Theme) -> Result<()> {
    save_section(path, "theme", toml::Value::try_from(theme)?)
}

fn save_section(path: &std::path::Path, section: &str, settings: toml::Value) -> Result<()> {
    if let Ok(meta) = fs::metadata(path) {
        ensure!(
            meta.is_file() && meta.len() <= 65536,
            "config must be a regular file of at most 64 KiB"
        );
    }
    let mut value = match fs::read_to_string(path) {
        Ok(source) => {
            Config::parse(&source)?;
            toml::from_str::<toml::Value>(&source)?
        }
        Err(e) if e.kind() == ErrorKind::NotFound => toml::Value::Table(Default::default()),
        Err(e) => return Err(e.into()),
    };
    value
        .as_table_mut()
        .context("config must be a table")?
        .insert(section.into(), settings);
    let text = toml::to_string_pretty(&value)?;
    Config::parse(&text)?;
    fs::create_dir_all(path.parent().context("config needs parent")?)?;
    crate::project::atomic(path, text.as_bytes())
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    #[default]
    Dark,
    Light,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    pub widget: Option<String>,
    pub widget_hovered: Option<String>,
    pub widget_active: Option<String>,
    pub border: Option<String>,
}

/// Built-ins are embedded so installed binaries do not need the source tree.
pub fn builtin_themes() -> &'static [(String, Theme)] {
    static THEMES: std::sync::OnceLock<Vec<(String, Theme)>> = std::sync::OnceLock::new();
    THEMES.get_or_init(|| vec![
        ("Light".into(), Theme { base: Base::Light, ..Theme::default() }),
        ("Dark".into(), Theme::default()),
        ("Bubblegum".into(), Config::parse(include_str!("../themes/bubblegum.toml"))
            .expect("bundled Bubblegum theme must be valid").theme),
    ])
}

/// User themes live beside the active config in themes/*.toml.
pub fn user_themes(path: &std::path::Path) -> Result<Vec<(String, Result<Theme>)>> {
    let directory = path.parent().context("config needs parent")?.join("themes");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("cannot list themes"),
    };
    let mut themes = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let theme = (|| -> Result<Theme> {
            let mut file = fs::File::open(&path)
                .with_context(|| format!("theme {}", path.display()))?;
            ensure!(file.metadata()?.is_file(), "theme must be a regular file");
            let mut source = String::new();
            Read::by_ref(&mut file).take(65537).read_to_string(&mut source)?;
            Ok(Config::parse(&source)
                .with_context(|| format!("theme {}", path.display()))?.theme)
        })();
        let name = path.file_stem().context("theme needs a filename")?.to_string_lossy();
        themes.push((format!("{name} (file)"), theme));
    }
    themes.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(themes)
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
        ensure!(
            config.recording.latency_ms.is_finite()
                && config.recording.latency_ms.abs() <= 2000.
                && config.recording.count_in_bars <= 8
                && config.recording.pre_roll_bars <= 8,
            "recording latency range +/-2000 ms; count-in/pre-roll maximum 8 bars"
        );
        ensure!(
            config.storage.cache_bytes >= 1048576
                && config.storage.cache_bytes <= 1024 * 1024 * 1024 * 1024
                && config.storage.minimum_free_bytes <= 1024 * 1024 * 1024 * 1024,
            "invalid storage limits"
        );
        ensure!(
            [1, 2].contains(&config.audio.channels),
            "audio channels must be 1 or 2"
        );
        ensure!(
            ["wav", "flac"].contains(&config.audio.format.as_str()),
            "audio format must be wav or flac"
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
        for value in [&t.widget, &t.widget_hovered, &t.widget_active, &t.border] {
            color(value, Color32::TRANSPARENT)?;
        }
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
        self.apply_changed(ctx, None)
    }
    pub fn apply_changed(&self, ctx: &egui::Context, previous: Option<&Config>) -> Result<()> {
        let p = self.palette()?;
        if previous.is_none_or(|old| old.theme != self.theme) {
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
            for (widgets, value) in [
                (&mut v.widgets.inactive, &self.theme.widget),
                (&mut v.widgets.hovered, &self.theme.widget_hovered),
                (&mut v.widgets.active, &self.theme.widget_active),
                (&mut v.widgets.open, &self.theme.widget_active),
            ] {
                if value.is_some() {
                    let fill = color(value, widgets.bg_fill)?;
                    widgets.bg_fill = fill;
                    widgets.weak_bg_fill = fill;
                }
            }
            if self.theme.border.is_some() {
                let border = color(&self.theme.border, v.window_stroke.color)?;
                v.window_stroke.color = border;
                for widgets in [
                    &mut v.widgets.noninteractive,
                    &mut v.widgets.inactive,
                    &mut v.widgets.hovered,
                    &mut v.widgets.active,
                    &mut v.widgets.open,
                ] {
                    widgets.bg_stroke.color = border;
                }
            }
            ctx.set_visuals(v);
        }
        if previous.is_none_or(|old| {
            old.appearance.font_size != self.appearance.font_size
                || old.appearance.editor_font_size != self.appearance.editor_font_size
        }) {
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
        }
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
    pending: Option<Stamp>,
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
            pending: None,
        }
    }
}
impl Watcher {
    pub fn load(&mut self) -> Result<Config> {
        let Some(path) = &self.path else {
            return Ok(Config::default());
        };
        self.last = Some(stamp(path));
        self.pending = None;
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
            self.pending = None;
            return None;
        }
        // Require a stable stamp across two polls (500 ms quiet period).
        if self.pending.as_ref() != Some(&next) {
            self.pending = Some(next);
            return None;
        }
        Some(self.load())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_restores_switched_and_saved_as_projects_without_touching_drafts() {
        let root = crate::codec::unique(&std::env::temp_dir(), "songbird-session", "dir").unwrap();
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        fs::write(&config, "[theme]\naccent='#112233'\n[recording]\nlatency_ms=12.0").unwrap();
        let first = crate::project::Project::create(&root.join("first"), crate::gui::DEFAULT.into()).unwrap();
        let mut unsaved = first.clone();
        unsaved.source.push_str("\n# unsaved draft\n");
        unsaved.save_draft().unwrap();
        let draft = fs::read(first.root.join("recovery.sb")).unwrap();
        save_last_project(&config, &first.root).unwrap();
        let loaded = Config::parse(&fs::read_to_string(&config).unwrap()).unwrap();
        assert!(loaded.session.restore_last_project);
        let restored = loaded.session.restore().unwrap().unwrap();
        assert_eq!(restored.root, first.root);
        assert_eq!(restored.source, first.source);
        assert_eq!(fs::read(first.root.join("recovery.sb")).unwrap(), draft);
        let second = crate::project::Project::create(&root.join("second"), crate::gui::DEFAULT.into()).unwrap();
        let saved_as = second.save_as(&root.join("copy")).unwrap();
        for project in [&second, &saved_as] {
            save_last_project(&config, &project.root).unwrap();
            let loaded = Config::parse(&fs::read_to_string(&config).unwrap()).unwrap();
            assert_eq!(loaded.session.restore().unwrap().unwrap().root, project.root);
            assert_eq!(loaded.theme.accent.as_deref(), Some("#112233"));
            assert_eq!(loaded.recording.latency_ms, 12.0);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_missing_invalid_and_disabled_restoration() {
        let root = crate::codec::unique(&std::env::temp_dir(), "songbird-restore", "dir").unwrap();
        let mut session = Session { last_project: Some(root.clone()), ..Session::default() };
        assert!(session.restore().is_err());
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("project.toml"), "invalid manifest").unwrap();
        assert!(session.restore().is_err());
        assert_eq!(fs::read_to_string(root.join("project.toml")).unwrap(), "invalid manifest");
        session.restore_last_project = false;
        assert!(session.restore().unwrap().is_none());
        let config = root.join("config.toml");
        fs::write(&config, "[session]\nrestore_last_project=false").unwrap();
        let saved = save_last_project(&config, &root).unwrap();
        assert!(!saved.restore_last_project);
        assert!(saved.restore().unwrap().is_none());
        assert!(Session::default().restore().unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn theme_selection_preserves_preferences_and_rejects_invalid_config() {
        let file = crate::codec::unique(&std::env::temp_dir(), "songbird-theme-save", "toml").unwrap();
        fs::write(&file, "[recording]\nlatency_ms=12.0\n[audio]\ninput='USB mic'\n").unwrap();
        for (_, theme) in builtin_themes() {
            save_theme(&file, theme).unwrap();
            let loaded = Config::parse(&fs::read_to_string(&file).unwrap()).unwrap();
            assert_eq!(&loaded.theme, theme);
            assert_eq!(loaded.recording.latency_ms, 12.0);
            assert_eq!(loaded.audio.input.as_deref(), Some("USB mic"));
        }
        fs::write(&file, "[broken").unwrap();
        assert!(save_theme(&file, &Theme::default()).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "[broken");
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn user_theme_discovery_keeps_valid_files_when_another_is_invalid() {
        let root = crate::codec::unique(&std::env::temp_dir(), "songbird-themes", "dir").unwrap();
        fs::create_dir_all(root.join("themes")).unwrap();
        fs::write(root.join("themes/valid.toml"), "[theme]\ntext='#112233'").unwrap();
        fs::write(root.join("themes/broken.toml"), "[broken").unwrap();
        let themes = user_themes(&root.join("config.toml")).unwrap();
        assert_eq!(themes.len(), 2);
        assert!(themes[0].1.is_err());
        assert_eq!(themes[1].1.as_ref().unwrap().text.as_deref(), Some("#112233"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn audio_preferences_round_trip_and_preserve_other_settings() {
        let file = crate::codec::unique(&std::env::temp_dir(), "songbird-audio", "toml").unwrap();
        fs::write(
            &file,
            "[theme]\naccent='#112233'\n[recording]\nlatency_ms=12.0",
        )
        .unwrap();
        let audio = AudioPreferences {
            input: Some("Disconnected USB mic".into()),
            output: Some("Speakers".into()),
            channels: 2,
            format: "wav".into(),
        };
        save_audio(&file, &audio).unwrap();
        let loaded = Config::parse(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(loaded.audio, audio);
        assert_eq!(loaded.recording.latency_ms, 12.);
        assert_eq!(loaded.theme.accent.as_deref(), Some("#112233"));
        assert!(!crate::audio::DeviceList::available(
            &[],
            loaded.audio.input.as_deref()
        ));
        assert!(crate::audio::DeviceList::available(&[], None));
        assert!(crate::audio::DeviceList::available(
            &["Speakers".into()],
            loaded.audio.output.as_deref()
        ));
        fs::write(&file, "[theme").unwrap();
        assert!(save_audio(&file, &audio).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "[theme");
        fs::remove_file(file).unwrap();
        assert!(Config::parse("[audio]\nchannels=3").is_err());
        assert!(Config::parse("[audio]\nformat='mp3'").is_err());
    }
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
            pending: None,
        };
        w.load().unwrap();
        let swap = file.with_extension("swap");
        fs::write(&swap, "[theme]\nbase='light'").unwrap();
        fs::rename(swap, &file).unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().is_none());
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(matches!(w.poll().unwrap().unwrap().theme.base, Base::Light));
        fs::remove_file(&file).unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().is_none());
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
            pending: None,
        };
        let mut current = w.load().unwrap();
        assert_eq!(
            current.palette().unwrap().accent,
            Color32::from_rgb(17, 34, 51)
        );
        fs::write(&file, "invalid config").unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().is_none());
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().unwrap().is_err());
        assert_eq!(
            current.palette().unwrap().accent,
            Color32::from_rgb(17, 34, 51)
        );
        fs::write(&file, "[theme]\nbase='light'\naccent='#445566'").unwrap();
        w.checked = Instant::now() - Duration::from_secs(1);
        assert!(w.poll().is_none());
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

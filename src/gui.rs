use crate::{
    audio::{self, Playback, Recording},
    config::{Config, Palette, Watcher},
    engine::Renderer,
    language,
    project::{History, Project},
    studio::{self, Capture, View as StudioView},
};
use anyhow::{Context, Result, bail, ensure};
use eframe::egui::{self, TextFormat};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};
const HELP: &str = "play | stop | pause | resume | seek POSITION | jump MARKER | range START END | loop on|off | restart | marker NAME | eval | save | open DIR | save-as DIR | import FILE | render FILE | render-region FILE START END | stems NEW_DIRECTORY | devices | input NAME | output NAME | record NAME flac|wav | record-at NAME START [END] | record-loop NAME | count-in BARS | pre-roll BARS | latency MS | metronome on|off | takes | take-replay ID | take-use ID [AT [TRIM_START TRIM_END]] | record-stop | voicebank help | channels 1|2 | monitor on|off | voice-new PATH NAME | voice-check PATH | undo | redo | tracks | console | find TEXT | recover-draft";
pub const DEFAULT: &str = include_str!("../examples/garden/composition.sb");
enum Job {
    Analyze(Project),
    Waveform(Project, PathBuf),
    Language(Project, Vec<String>),
    Studio(studio::Request),
    Capture(Project, String, PathBuf, Capture),
    New(Project, PathBuf),
    Convert(Project, String, String, u32, u16),
    Draft(Project),
    Prepare(Project, Option<audio::LiveContext>),
    Transport(Project, f64, Option<f64>, bool, Option<f64>, u32, bool),
    Render(Project, PathBuf),
    Region(Project, PathBuf, String, String),
    Stems(Project, PathBuf),
    Save(Project),
    SaveAs(Project, PathBuf),
    Open(Project, PathBuf),
    Import(Project, PathBuf),
    Validate(Project, String),
    TakePreview(Project, u32),
}
enum Reply {
    Waveform(Vec<f32>),
    Language(Vec<String>),
    Studio(Box<studio::Output>),
    Drafted,
    Saved(String),
    Prepared(Box<Renderer>, String),
    Transport(Box<Renderer>, String),
    Message(String),
    Project(Project),
    Imported(String),
}
struct Work {
    tx: SyncSender<Job>,
    rx: Receiver<Result<Reply>>,
    busy: bool,
    replacing_document: bool,
}
impl Work {
    fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(2);
        let (ot, or) = mpsc::sync_channel(2);
        thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                let r = (|| -> Result<Reply> {
                    Ok(match job {
                        Job::Analyze(project) => {
                            Reply::Message(crate::analysis::project(&project)?)
                        }
                        Job::Waveform(p, path) => {
                            let asset = crate::codec::Asset::decode(&path, &p.root.join("cache"))?;
                            Reply::Waveform(compact_peaks(&asset.waveform, 2048))
                        }
                        Job::Language(project, args) => {
                            Reply::Language(crate::phonetics::command(&project.root, &args)?)
                        }
                        Job::Studio(request) => Reply::Studio(Box::new(studio::execute(request)?)),
                        Job::Capture(p, bank, path, capture) => {
                            Reply::Studio(Box::new(studio::finish_capture(p, bank, path, capture)?))
                        }
                        Job::New(current, path) => {
                            current.save_draft()?;
                            Reply::Project(Project::create(&path, DEFAULT.into())?)
                        }
                        Job::Convert(p, input, output, rate, bits) => {
                            let input = crate::codec::safe_path(&p.root, &input)?;
                            let output = p.root.join("exports").join(output);
                            crate::codec::convert(
                                &input,
                                &output,
                                &p.root.join("cache"),
                                rate,
                                bits,
                            )?;
                            Reply::Message(format!("Converted to {}", output.display()))
                        }
                        Job::Draft(p) => {
                            p.save_draft()?;
                            Reply::Drafted
                        }
                        Job::Transport(p, start, end, looping, anchor, count_in, metronome) => {
                            let mut r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
                            let rate = r.score.rate as f64;
                            let end = end.map_or(r.length, |end| (end * rate).round() as u64);
                            r.transport(
                                (anchor.unwrap_or(start) * rate).round() as u64,
                                end,
                                looping,
                            )?;
                            r.seek((start * rate).round() as u64)?;
                            r.count_in(count_in);
                            r.metronome = metronome;
                            Reply::Transport(Box::new(r), p.source)
                        }
                        Job::Prepare(p, live) => {
                            let mut r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
                            if let Some(live) = live {
                                live.preroll(&mut r)?;
                            }
                            Reply::Prepared(Box::new(r), p.source)
                        }
                        Job::Region(p, path, start, end) => {
                            let score = language::parse(&p.source)?;
                            let start = score.clock()?.parse(&start, &score.markers)?;
                            let end = score.clock()?.parse(&end, &score.markers)?;
                            Reply::Message(format!(
                                "Export: {:?}",
                                crate::export::region(&p.root, score, &path, start, end)?
                            ))
                        }
                        Job::Stems(p, path) => {
                            let reports =
                                crate::export::stems(&p.root, &language::parse(&p.source)?, &path)?;
                            Reply::Message(format!(
                                "Exported {} post-fader stems without master effects to {}",
                                reports.len(),
                                path.display()
                            ))
                        }
                        Job::Render(p, path) => {
                            let mut r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
                            r.render(&path)?;
                            Reply::Message(format!(
                                "Exported {}; peak {:.3}, {} samples over full scale before PCM clipping",
                                path.display(),
                                r.peak,
                                r.clipped
                            ))
                        }
                        Job::Save(p) => {
                            p.save()?;
                            Reply::Saved(p.source)
                        }
                        Job::SaveAs(p, path) => Reply::Project(p.save_as(&path)?),
                        Job::Open(current, path) => {
                            current.save_draft()?;
                            Reply::Project(Project::open(&path)?)
                        }
                        Job::Import(p, path) => Reply::Imported(p.import(&path)?),
                        Job::TakePreview(p, id) => {
                            let take = crate::recording::takes(&p.root)?
                                .into_iter()
                                .find(|t| t.id == id)
                                .context("unknown take")?;
                            let source = format!(
                                "rate 48000; bars 4096; {}",
                                crate::recording::clip(&take, "preview", 0., None)?
                            );
                            let mut score = language::parse(&source)?;
                            score.bars = 1800;
                            let mut renderer = Renderer::prepare(&p.root, score)?;
                            if let Some(clip) =
                                renderer.tracks.first().and_then(|t| t.clips.first())
                            {
                                renderer.length = ((clip.spec.end.unwrap_or(clip.audio.duration())
                                    - clip.spec.start)
                                    * renderer.score.rate as f64)
                                    .round()
                                    as u64;
                            }
                            Reply::Transport(Box::new(renderer), p.source)
                        }
                        Job::Validate(p, name) => {
                            Reply::Message(crate::voice::validate(&p.root, &name)?)
                        }
                    })
                })();
                if ot.send(r).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx: or,
            busy: false,
            replacing_document: false,
        }
    }
    fn send(&mut self, job: Job) -> Result<()> {
        ensure!(!self.busy, "background operation in progress");
        let replacing_document = matches!(&job, Job::New(..) | Job::Open(..) | Job::SaveAs(..));
        self.tx
            .try_send(job)
            .map_err(|_| anyhow::anyhow!("background queue unavailable"))?;
        self.busy = true;
        self.replacing_document = replacing_document;
        Ok(())
    }
}
pub struct App {
    catalog: crate::locale::Catalog,
    help: crate::help::Help,
    font_attempted: bool,
    studio: Option<StudioView>,
    tuner_enabled: bool,
    tuner_target_enabled: bool,
    tuner_target: i32,
    bank_capture: Option<(String, Capture)>,
    bank_pending: Option<(String, PathBuf, Capture)>,
    audition: bool,
    config: Config,
    palette: Palette,
    watcher: Watcher,
    theme_pending: bool,
    project: Project,
    work: Work,
    playback: Option<Playback>,
    recording: Option<Recording>,
    finishing: Option<std::thread::JoinHandle<Result<PathBuf>>>,
    preview: Option<language::Score>,
    preview_source: String,
    console: Vec<String>,
    command: String,
    commands: Vec<String>,
    history_index: usize,
    history: History,
    last_evaluated: String,
    input: Option<String>,
    output: Option<String>,
    monitor: bool,
    channels: u16,
    show_tracks: bool,
    show_console: bool,
    search: String,
    search_open: bool,
    search_from: usize,
    pending_find: bool,
    waveform: Vec<f32>,
    asset_pending: Option<PathBuf>,
    devices: audio::DeviceList,
    record_format: String,
    record_name: String,
    scope_source: usize,
    live_waveform: std::collections::VecDeque<f32>,
    scope_peak: f32,
    scope_samples: usize,
    pending_play: bool,
    cursor_seconds: f64,
    playback_range: Option<(f64, f64)>,
    loop_enabled: bool,
    pending_record: Option<(String, String, f64, f64)>,
    recording_placement: Option<f64>,
    dirty: bool,
    draft_at: Instant,
    smoke_frames: Option<u32>,
    readout_at: Instant,
    readout_seconds: f64,
    readout_levels: [f32; 2],
    readout_pending: [f32; 2],
    pub output_peak: f32,
    pub input_peak: f32,
}
impl App {
    pub fn new(project: Project) -> Self {
        let last_evaluated = project.source.clone();
        let preview = language::parse(&project.source).ok();
        let preview_source = project.source.clone();
        let mut watcher = Watcher::default();
        let mut warning = None;
        let config = watcher.load().unwrap_or_else(|e| {
            warning = Some(format!("CONFIG: {e:#}; using defaults"));
            Config::default()
        });
        let palette = config.palette().expect("validated theme");
        let catalog = crate::locale::Catalog::load(
            &config.language.interface,
            crate::phonetics::resource_root().as_deref(),
        )
        .unwrap_or_else(|e| {
            warning = Some(format!("Language resources: {e:#}; English fallback"));
            Default::default()
        });
        let mut app = Self {
            catalog,
            help: Default::default(),
            font_attempted: false,
            studio: None,
            tuner_enabled: true,
            tuner_target_enabled: false,
            tuner_target: 57,
            bank_capture: None,
            bank_pending: None,
            audition: false,
            show_tracks: config.appearance.show_tracks,
            show_console: config.appearance.show_console,
            input: config.audio.input.clone(),
            output: config.audio.output.clone(),
            channels: config.audio.channels,
            record_format: config.audio.format.clone(),
            config,
            palette,
            watcher,
            theme_pending: true,
            project,
            work: Work::new(),
            playback: None,
            recording: None,
            finishing: None,
            preview,
            preview_source,
            console: vec![
                "SONGBIRD / Ctrl+Enter evaluate · Ctrl+P play/stop · Ctrl+S save · :help".into(),
            ],
            command: String::new(),
            commands: vec![],
            history_index: 0,
            history: History::default(),
            last_evaluated,
            monitor: false,
            search: String::new(),
            search_open: false,
            search_from: 0,
            pending_find: false,
            waveform: vec![],
            asset_pending: None,
            devices: audio::DeviceList::default(),
            record_name: "take".into(),
            scope_source: 0,
            live_waveform: Default::default(),
            scope_peak: 0.,
            scope_samples: 0,
            pending_play: false,
            cursor_seconds: 0.,
            playback_range: None,
            loop_enabled: false,
            pending_record: None,
            recording_placement: None,
            dirty: false,
            draft_at: Instant::now(),
            smoke_frames: None,
            readout_at: Instant::now() - Duration::from_secs(1),
            readout_seconds: 0.,
            readout_levels: [0.; 2],
            readout_pending: [0.; 2],
            output_peak: 0.,
            input_peak: 0.,
        };
        if let Some(warning) = warning {
            app.log(warning);
        }
        app.refresh_devices();
        app.queue_first_asset();
        match crate::timeline::Transport::load(&app.project.root) {
            Ok(state) => {
                app.cursor_seconds = state.cursor;
                app.playback_range = state.range;
                app.loop_enabled = state.looping;
            }
            Err(error) => app.log(format!("Transport preferences: {error:#}")),
        }
        app
    }
    fn remember_project(&mut self) {
        let result = self.watcher.path.as_deref()
            .context("no configuration path available")
            .and_then(|path| crate::config::save_last_project(path, &self.project.root));
        match result {
            Ok(session) => self.config.session = session,
            Err(e) => self.log(format!("Remember project: {e:#}")),
        }
    }
    fn queue_first_asset(&mut self) {
        self.asset_pending = self
            .preview
            .as_ref()
            .and_then(|score| score.tracks.iter().flat_map(|t| &t.clips).next())
            .and_then(|clip| crate::codec::safe_path(&self.project.root, &clip.path).ok());
    }
    fn voicebank_tools(&mut self, ui: &mut egui::Ui) -> Option<Vec<String>> {
        let mut action = None;
        ui.heading("Voicebank tools");
        if ui.small_button("Beginner guide").clicked() {
            self.help.topic = "voicebank".into();
            self.help.open = true;
        }
        let idle = !self.work.busy
            && self.recording.is_none()
            && self.finishing.is_none()
            && self.bank_capture.is_none()
            && self.bank_pending.is_none();
        if let Some(view) = &self.studio {
            ui.add_enabled_ui(idle, |ui| {
                let mut target = view.session.target.clone();
                egui::ComboBox::from_id_salt("bank-target").selected_text(format!("Sound: {target}")).show_ui(ui, |ui| {
                    for phone in &view.phones {
                        ui.selectable_value(&mut target, phone.id.clone(), format!("{} — {}", phone.id, phone.description));
                    }
                });
                if target != view.session.target { action = Some(vec!["target".into(), target]); }
                if ui.button("Next missing sound").on_hover_text("Choose a missing sound for the current bank language and recording profile").clicked() {
                    action = Some(vec!["guide".into(), "next".into()]);
                }
                let mut selected = view.session.selected;
                egui::ComboBox::from_id_salt("bank-take").selected_text(selected.map_or("Take: none".into(), |i| format!("Take {}", i + 1))).show_ui(ui, |ui| {
                    for (i, take) in view.session.takes.iter().enumerate() {
                        ui.selectable_value(&mut selected, Some(i), format!("{} — {}", i + 1, take.capture.phoneme));
                    }
                });
                if selected != view.session.selected && let Some(i) = selected { action = Some(vec!["take".into(), (i + 1).to_string()]); }
                ui.add_enabled_ui(view.session.selected.is_some(), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Listen original").clicked() { action = Some(vec!["replay".into(), "raw".into()]); }
                        if ui.button("Listen trimmed").clicked() { action = Some(vec!["replay".into()]); }
                        if ui.button("Re-analyze").on_hover_text("Refresh measurements without replacing your manual edits").clicked() { action = Some(vec!["analyze".into()]); }
                    });
                    if ui.button("Use selected take").on_hover_text("Validate and publish this take into the playable bank. A matching mapping may be replaced; original takes are retained.").clicked() {
                        action = Some(vec!["use".into()]);
                    }
                });
            });
        }
        ui.separator();
        action
    }
    fn pitch_coach(&mut self, ui: &mut egui::Ui) -> Option<Vec<String>> {
        let mut action = None;
        egui::CollapsingHeader::new("Pitch coach").default_open(true).show(ui, |ui| {
            ui.add_enabled_ui(self.recording.is_none(), |ui| {
                ui.checkbox(&mut self.tuner_enabled, "Live tuner while recording");
            });
            ui.checkbox(&mut self.tuner_target_enabled, "Use a target note");
            if self.tuner_target_enabled {
                ui.add(egui::Slider::new(&mut self.tuner_target, 28..=95).text("Target").custom_formatter(|n, _| crate::tuner::note_name(n.round() as i32)));
                ui.small(format!("{} · {:.1} Hz (A4 = 440 Hz)", crate::tuner::note_name(self.tuner_target), crate::dsp::hz(self.tuner_target as f64)));
            }
            ui.small("Choose a comfortable target. Pitch is one aspect of voice; no gender target is assigned.");
            let reading = self.recording.as_ref().filter(|_| self.bank_capture.is_some())
                .and_then(|r| r.tuner.as_ref()).and_then(crate::tuner::LiveTuner::reading);
            if let Some(reading) = reading {
                if let Some(hz) = reading.hz {
                    let note = crate::tuner::nearest_note(hz);
                    ui.monospace(format!("{} · {hz:.1} Hz", crate::tuner::note_name(note)));
                    let reference = if self.tuner_target_enabled { self.tuner_target } else { note };
                    let cents = crate::tuner::cents(hz, crate::dsp::hz(reference as f64));
                    let direction = if cents.abs() <= 10. { "In tune" } else if cents < 0. { "Flat" } else { "Sharp" };
                    ui.monospace(format!("{direction} · {cents:+.0} cents"));
                    ui.small(if self.tuner_target_enabled { "Compared with your target note" } else { "Compared with the nearest note" });
                } else if reading.clipped { ui.label("Clipped input — lower recording gain"); }
                else { ui.label("Unvoiced / uncertain pitch"); }
                ui.small(format!("Periodicity {:.0}% · RMS {:.0} dB · peak {:.0} dB", reading.confidence * 100., reading.rms_db, 20. * reading.peak.max(1e-6).log10()));
                if let Some(stability) = reading.stability_cents { ui.small(format!("Recent pitch spread: {stability:.0} cents SD")); }
                else { ui.small("Hold a comfortable voiced sound to measure stability"); }
                let reference = if self.tuner_target_enabled { Some(crate::dsp::hz(self.tuner_target as f64)) } else { reading.hz.map(|hz| crate::dsp::hz(crate::tuner::nearest_note(hz) as f64)) };
                pitch_history(ui, &reading.history, reference, self.palette.waveform);
                if reading.dropped > 0 { ui.small(format!("Tuner skipped {} samples; recording is independent", reading.dropped)); }
            } else if self.bank_capture.is_some() && self.recording.is_some() {
                ui.small("Waiting for fresh pitch analysis. Silence and unvoiced consonants may have no note.");
            } else { ui.small("Record a bank take to see pitch. Silence and unvoiced consonants may have no note."); }
            if self.bank_capture.is_some() && self.recording.is_some() {
                if ui.button("Stop bank recording").clicked() { action = Some(vec!["record-stop".into()]); }
            } else if ui.add_enabled(self.recording.is_none() && self.finishing.is_none() && self.bank_pending.is_none() && !self.work.busy,
                egui::Button::new("Record bank target")).clicked() { action = Some(vec!["record".into()]); }
            if let Some(take) = self.studio.as_ref().and_then(|v| v.session.selected.and_then(|i| v.session.takes.get(i))) {
                if let Some(hz) = take.analysis.suggested_root() {
                    ui.small(format!("Measured root: {} · {hz:.1} Hz", crate::tuner::note_name(crate::tuner::nearest_note(hz))));
                    if ui.add_enabled(self.recording.is_none() && self.finishing.is_none() && self.bank_pending.is_none() && !self.work.busy,
                        egui::Button::new("Apply measured root")).on_hover_text("Explicitly replaces only the selected take's root frequency. Original audio and published samples are unchanged.").clicked() {
                        action = Some(vec!["set".into(), "root".into(), format!("{hz:.8}")]);
                    }
                }
            }
            ui.separator();
        });
        action
    }
    fn refresh_devices(&mut self) {
        match audio::DeviceList::refresh() {
            Ok(devices) => self.devices = devices,
            Err(e) => self.log(format!("Audio device discovery: {e:#}")),
        }
    }
    fn save_audio_preferences(&mut self) -> Result<()> {
        let audio = crate::config::AudioPreferences {
            input: self.input.clone(),
            output: self.output.clone(),
            channels: self.channels,
            format: self.record_format.clone(),
        };
        let path = self
            .watcher
            .path
            .as_ref()
            .context("no configuration path available")?;
        crate::config::save_audio(path, &audio)?;
        self.config.audio = audio;
        Ok(())
    }
    /// Apply deltas without touching document, editor memory, transport, or workers.
    fn accept_config(&mut self, ctx: &egui::Context, config: Config) {
        if config == self.config {
            return;
        }
        if config.theme != self.config.theme
            || config.appearance.font_size != self.config.appearance.font_size
            || config.appearance.editor_font_size != self.config.appearance.editor_font_size
        {
            if let Err(e) = config.apply_changed(ctx, Some(&self.config)) {
                self.log(format!("CONFIG: {e:#}"));
                return;
            }
            self.palette = config.palette().expect("validated theme");
        }
        if config.appearance.show_tracks != self.config.appearance.show_tracks {
            self.show_tracks = config.appearance.show_tracks;
        }
        if config.appearance.show_console != self.config.appearance.show_console {
            self.show_console = config.appearance.show_console;
        }
        if config.language != self.config.language {
            match crate::locale::Catalog::load(
                &config.language.interface,
                crate::phonetics::resource_root().as_deref(),
            ) {
                Ok(catalog) => self.catalog = catalog,
                Err(e) => self.log(format!("Language resources: {e:#}")),
            }
            self.font_attempted = false;
        }
        if config.audio != self.config.audio {
            self.input = config.audio.input.clone();
            self.output = config.audio.output.clone();
            self.channels = config.audio.channels;
            self.record_format = config.audio.format.clone();
        }
        self.config = config;
    }
    fn audio_controls(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Audio devices & recording")
            .id_salt("audio-controls")
            .show(ui, |ui| {
                let before = (
                    self.input.clone(),
                    self.output.clone(),
                    self.channels,
                    self.record_format.clone(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(self.recording.is_none(), |ui| {
                        device_combo(ui, "Input", &self.devices.inputs, &mut self.input);
                    });
                    ui.add_enabled_ui(self.recording.is_none() && self.playback.is_none(), |ui| {
                        device_combo(ui, "Output", &self.devices.outputs, &mut self.output);
                    });
                    if ui.button("Refresh devices").clicked() {
                        self.refresh_devices();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(self.recording.is_none() && self.finishing.is_none(), |ui| {
                        egui::ComboBox::from_id_salt("record-channels")
                            .selected_text(if self.channels == 1 { "Mono" } else { "Stereo" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.channels, 1, "Mono");
                                ui.selectable_value(&mut self.channels, 2, "Stereo");
                            });
                        egui::ComboBox::from_id_salt("record-format")
                            .selected_text(self.record_format.to_uppercase())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.record_format, "wav".into(), "WAV");
                                ui.selectable_value(&mut self.record_format, "flac".into(), "FLAC");
                            });
                        ui.checkbox(&mut self.monitor, "Monitor input (use headphones)");
                        ui.label("Take name");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.record_name).desired_width(120.),
                        );
                    });
                    if self.recording.is_some() {
                        if ui.button("Stop recording").clicked()
                            && let Err(e) = self.command("record-stop")
                        {
                            self.log(format!("Recording: {e:#}"));
                        }
                    } else if ui
                        .add_enabled(
                            self.finishing.is_none()
                                && self.bank_pending.is_none()
                                && self.pending_record.is_none(),
                            egui::Button::new("Record"),
                        )
                        .clicked()
                    {
                        let command =
                            format!("record {:?} {}", self.record_name, self.record_format);
                        if let Err(e) = self.command(&command) {
                            self.log(format!("Recording: {e:#}"));
                        }
                    }
                    if self.finishing.is_some() {
                        ui.label("Finalizing take…");
                    }
                });
                if before
                    != (
                        self.input.clone(),
                        self.output.clone(),
                        self.channels,
                        self.record_format.clone(),
                    )
                    && let Err(e) = self.save_audio_preferences()
                {
                    self.log(format!("Audio preferences not saved: {e:#}"));
                }
            });
    }
    fn log(&mut self, s: impl Into<String>) {
        self.console.push(self.catalog.message(&s.into()));
        if self.console.len() > 200 {
            self.console.remove(0);
        }
    }
    fn start_transport(&mut self) -> Result<()> {
        ensure!(
            self.recording.is_none(),
            "stop timeline recording before changing transport"
        );
        let (start, end) = if let Some((a, b)) = self.playback_range {
            (
                if self.cursor_seconds >= a && self.cursor_seconds < b {
                    self.cursor_seconds
                } else {
                    a
                },
                Some(b),
            )
        } else {
            (self.cursor_seconds, None)
        };
        self.work.send(Job::Transport(
            self.project.clone(),
            start,
            end,
            self.loop_enabled,
            self.playback_range.map(|r| r.0),
            0,
            self.config.recording.metronome,
        ))
    }
    fn evaluate(&mut self, play: bool) -> Result<()> {
        ensure!(
            !self.audition,
            "stop the voicebank preview before evaluating the song"
        );
        self.history.checkpoint(&self.last_evaluated);
        if play || (self.loop_enabled && self.playback.is_some()) {
            return self.start_transport();
        }
        self.pending_play = play;
        self.work.send(Job::Prepare(
            self.project.clone(),
            self.playback.as_ref().map(Playback::live_context),
        ))
    }
    fn stop(&mut self) -> Result<()> {
        self.audition = false;
        if let Some(mut p) = self.playback.take() {
            p.stop()?;
        }
        Ok(())
    }
    fn voicebank_command(&mut self, args: &[String]) -> Result<()> {
        let op = args.first().map(String::as_str).unwrap_or("help");
        if op == "help" {
            self.help.open = true;
            self.help.topic = "voicebank-reference".into();
            self.log("voicebank new PATH NAME [LANGUAGE|auto] | open PATH | target PHONE | guide [next|LYRICS] | register NAME | expression NAME | settings RATE BITS CHANNELS [flac|wav] | record [PHONE] | record-stop | import FILE [PHONE] | takes | take NUMBER | samples | sample NUMBER | replay [raw] | analyze | suggest | trim ONSET SUSTAIN END | set FIELD VALUE | use | coverage [LYRICS [NOTES [PRESET]]] | validate | missing | dictionary WORD PHONES | test LYRICS NOTES [PRESET] | presets | preset new|show|set|effect NAME ... | save | export NEW_DIRECTORY | import-bank SOURCE PATH | license TEXT | close");
            return Ok(());
        }
        if op == "close" {
            ensure!(
                self.bank_capture.is_none() && self.bank_pending.is_none(),
                "finish the bank take before closing the studio"
            );
            self.studio = None;
            return Ok(());
        }
        if op == "record-stop" {
            return self.command("record-stop");
        }
        if op == "record" {
            ensure!(
                self.recording.is_none()
                    && self.finishing.is_none()
                    && self.bank_pending.is_none()
                    && !self.work.busy,
                "finish the current operation/take first"
            );
            let view = self
                .studio
                .as_ref()
                .context("open or create a voicebank first")?;
            let target = args
                .get(1)
                .map(String::as_str)
                .unwrap_or(&view.session.target)
                .to_owned();
            ensure!(
                target
                    .split('.')
                    .all(|id| view.phones.iter().any(|p| p.id == id)
                        || view.aliases.contains_key(id)),
                "invalid recording target; voicebank guide"
            );
            let settings = &view.session.settings;
            let capture = Capture {
                phoneme: target.clone(),
                register: view.session.register.clone(),
                expression: view.session.expression.clone(),
                recorded: true,
                phonetics: Some(view.selection.clone()),
            };
            let manifest = crate::codec::safe_path(
                &self.project.root,
                &format!("{}/voicebank.toml", view.bank),
            )?;
            let start = if self.tuner_enabled {
                Recording::start_with_tuner
            } else {
                Recording::start
            };
            self.recording = Some(start(
                manifest.parent().unwrap(),
                &audio::RecordingConfig {
                    name: "phoneme",
                    format: &settings.format,
                    rate: settings.rate,
                    bits: settings.bits,
                    channels: settings.channels,
                    input_name: self.input.as_deref(),
                    output_name: self.output.as_deref(),
                    monitoring: self.monitor,
                    gate: None,
                },
            )?);
            self.bank_capture = Some((view.bank.clone(), capture.clone()));
            if let Err(e) = studio::recording_started(
                &self.project.root,
                &view.bank,
                &self.recording.as_ref().unwrap().take,
                &capture,
            ) {
                self.log(format!("Capture metadata: {e:#}. Recording is active; record-stop still preserves the audio."));
            }
            if let Some(view) = &mut self.studio {
                view.session.target = target.to_owned();
            }
            self.log(
                "Recording bank target. record-stop preserves this take and prepares its analysis.",
            );
            return Ok(());
        }
        ensure!(
            self.bank_capture.is_none() && self.bank_pending.is_none(),
            "finish the bank recording before editing or switching voicebanks"
        );
        if ["replay", "test"].contains(&op) {
            ensure!(
                self.playback.is_none(),
                "stop playback before a voicebank preview"
            );
        }
        self.work.send(Job::Studio(studio::Request {
            project: self.project.clone(),
            bank: self.studio.as_ref().map(|v| v.bank.clone()),
            args: args.to_vec(),
        }))
    }
    fn command(&mut self, src: &str) -> Result<()> {
        let rows = language::lex(src.trim_start_matches(':'))?;
        ensure!(rows.len() == 1, "enter one console command");
        let v = &rows[0];
        let arg = |i: usize| {
            v.get(i)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing argument; :help"))
        };
        match v[0].as_str() {
            "voicebank" => self.voicebank_command(&v[1..])?,
            "help" => {
                let topic = v.get(1).map(String::as_str).unwrap_or("music");
                if topic == "search" {
                    self.help.search = v.get(2).cloned().unwrap_or_default();
                } else {
                    ensure!(
                        crate::help::document(topic).is_some(),
                        "unknown help topic; help languages"
                    );
                    self.help.topic = topic.into();
                }
                self.help.open = true;
                self.log(format!(
                    "{}: {HELP}",
                    self.catalog.text("Available commands")
                ));
            }
            "language" => {
                if v.get(1).is_none_or(|s| s == "help") {
                    self.help.open = true;
                    self.help.topic = "language".into();
                } else {
                    self.work
                        .send(Job::Language(self.project.clone(), v[1..].to_vec()))?;
                }
            }
            "new" => {
                ensure!(
                    self.playback.is_none()
                        && self.recording.is_none()
                        && self.finishing.is_none()
                        && self.bank_pending.is_none(),
                    "stop audio and finish takes before creating a project"
                );
                self.work
                    .send(Job::New(self.project.clone(), PathBuf::from(arg(1)?)))?;
            }
            "convert" => {
                let output = arg(2)?;
                ensure!(
                    std::path::Path::new(&output).components().count() == 1
                        && !output.starts_with('.'),
                    "conversion output must be a filename in exports/"
                );
                let score = language::parse(&self.project.source)?;
                let rate = v
                    .get(3)
                    .map(|v| v.parse::<u32>())
                    .transpose()?
                    .unwrap_or(score.rate);
                let bits = v
                    .get(4)
                    .map(|v| v.parse::<u16>())
                    .transpose()?
                    .unwrap_or(score.bits);
                self.work.send(Job::Convert(
                    self.project.clone(),
                    arg(1)?,
                    output,
                    rate,
                    bits,
                ))?;
            }
            "play" => self.evaluate(true)?,
            "eval" => self.evaluate(false)?,
            "stop" => {
                if let Some(p) = &self.playback {
                    self.cursor_seconds = p.state.frames.load(std::sync::atomic::Ordering::Relaxed)
                        as f64
                        / p.rate as f64;
                }
                self.stop()?;
            }
            "seek" | "jump" => {
                let score = language::parse(&self.project.source)?;
                let value = arg(1)?;
                let position = if v[0] == "jump" {
                    format!("@{value}")
                } else {
                    value
                };
                let seconds = score.clock()?.parse(&position, &score.markers)?;
                ensure!(
                    seconds < score.duration(),
                    "position lies beyond arrangement end"
                );
                if let Some((a, b)) = self.playback_range {
                    ensure!(
                        seconds >= a && seconds < b,
                        "seek lies outside playback range; use range clear first"
                    );
                }
                self.cursor_seconds = seconds;
                if self.playback.is_some() {
                    self.start_transport()?;
                }
            }
            "restart" | "home" => {
                self.playback_range = None;
                self.loop_enabled = false;
                self.cursor_seconds = 0.;
                if self.playback.is_some() {
                    self.start_transport()?;
                }
            }
            "range" => {
                if arg(1)? == "clear" {
                    if let Some(p) = &self.playback {
                        self.cursor_seconds =
                            p.state.frames.load(std::sync::atomic::Ordering::Relaxed) as f64
                                / p.rate as f64;
                    }
                    self.playback_range = None;
                    self.loop_enabled = false;
                } else {
                    let score = language::parse(&self.project.source)?;
                    let clock = score.clock()?;
                    let a = clock.parse(&arg(1)?, &score.markers)?;
                    let b = clock.parse(&arg(2)?, &score.markers)?;
                    ensure!(
                        a < b && b <= score.duration(),
                        "range requires start < end <= arrangement end"
                    );
                    self.playback_range = Some((a, b));
                    self.cursor_seconds = a;
                }
                if self.playback.is_some() {
                    self.start_transport()?;
                }
            }
            "loop" => {
                let value = arg(1)?;
                ensure!(["on", "off"].contains(&value.as_str()), "loop on|off");
                ensure!(
                    value == "off" || self.playback_range.is_some(),
                    "set range START END before enabling loop"
                );
                if let Some(p) = &self.playback {
                    self.cursor_seconds = p.state.frames.load(std::sync::atomic::Ordering::Relaxed)
                        as f64
                        / p.rate as f64;
                }
                self.loop_enabled = value == "on";
                if self.playback.is_some() {
                    self.start_transport()?;
                }
            }
            "marker" => {
                let score = language::parse(&self.project.source)?;
                let seconds = self.playback.as_ref().map_or(self.cursor_seconds, |p| {
                    p.state.frames.load(std::sync::atomic::Ordering::Relaxed) as f64 / p.rate as f64
                });
                let beat = score.clock()?.beat(seconds);
                let name = arg(1)?;
                ensure!(!name.contains(['"', '\n', '\r']), "invalid marker name");
                self.project
                    .source
                    .push_str(&format!("\nmarker \"{name}\" at {beat}b\n"));
                self.dirty = true;
            }
            "pause" | "resume" => {
                let playback = self.playback.as_ref().context("start playback first")?;
                if v[0] == "pause" {
                    playback.pause()?;
                } else {
                    playback.resume()?;
                }
            }
            "save" => self.work.send(Job::Save(self.project.clone()))?,
            "open" => {
                ensure!(
                    self.playback.is_none()
                        && self.recording.is_none()
                        && self.finishing.is_none()
                        && self.bank_pending.is_none(),
                    "stop audio and finish takes before opening a project"
                );
                self.work
                    .send(Job::Open(self.project.clone(), PathBuf::from(arg(1)?)))?;
            }
            "save-as" => {
                ensure!(
                    self.playback.is_none()
                        && self.recording.is_none()
                        && self.finishing.is_none()
                        && self.bank_pending.is_none(),
                    "stop audio and finish takes before Save As"
                );
                self.work
                    .send(Job::SaveAs(self.project.clone(), PathBuf::from(arg(1)?)))?;
            }
            "import" => self
                .work
                .send(Job::Import(self.project.clone(), PathBuf::from(arg(1)?)))?,
            "render-region" | "stems" => {
                let name = arg(1)?;
                ensure!(
                    std::path::Path::new(&name).components().count() == 1 && !name.starts_with('.'),
                    "export takes a name inside exports/"
                );
                let path = self.project.root.join("exports").join(name);
                let job = if v[0] == "stems" {
                    Job::Stems(self.project.clone(), path)
                } else {
                    Job::Region(self.project.clone(), path, arg(2)?, arg(3)?)
                };
                self.work.send(job)?;
            }
            "render" => {
                let name = arg(1)?;
                ensure!(
                    std::path::Path::new(&name).components().count() == 1 && !name.starts_with('.'),
                    "render takes a filename in exports/"
                );
                self.work.send(Job::Render(
                    self.project.clone(),
                    self.project.root.join("exports").join(name),
                ))?;
            }
            "plugins" => self.log(crate::plugins::compatibility()),
            "analyze" => self.work.send(Job::Analyze(self.project.clone()))?,
            "devices" => {
                for d in audio::devices()? {
                    self.log(d);
                }
            }
            "input" => {
                ensure!(self.recording.is_none(), "stop recording first");
                let name = arg(1)?;
                self.input = (name != "default").then_some(name);
                self.save_audio_preferences()?;
            }
            "output" => {
                ensure!(
                    self.playback.is_none() && self.recording.is_none(),
                    "stop audio before changing output"
                );
                let name = arg(1)?;
                self.output = (name != "default").then_some(name);
                self.save_audio_preferences()?;
            }
            "channels" => {
                let n = arg(1)?.parse()?;
                ensure!([1, 2].contains(&n), "channels must be 1 or 2");
                ensure!(self.recording.is_none(), "stop recording first");
                self.channels = n;
                self.save_audio_preferences()?;
            }
            "monitor" => {
                let value = arg(1)?;
                ensure!(["on", "off"].contains(&value.as_str()), "monitor on|off");
                ensure!(self.recording.is_none(), "set monitoring before recording");
                self.monitor = value == "on";
            }
            "latency" | "count-in" | "pre-roll" | "metronome" => {
                ensure!(
                    self.recording.is_none(),
                    "finish recording before changing capture settings"
                );
                self.config.recording = crate::recording::preference(&v[0], &arg(1)?)?;
            }
            "record-at" | "record-loop" => {
                ensure!(
                    self.recording.is_none()
                        && self.finishing.is_none()
                        && self.pending_record.is_none(),
                    "finish the current take first"
                );
                let score = language::parse(&self.project.source)?;
                let clock = score.clock()?;
                let loop_capture = v[0] == "record-loop";
                let start = if loop_capture {
                    self.playback_range
                        .context("set range before record-loop")?
                        .0
                } else {
                    clock.parse(&arg(2)?, &score.markers)?
                };
                let end = if loop_capture {
                    self.playback_range.unwrap().1
                } else if let Some(value) = v.get(3) {
                    clock.parse(value, &score.markers)?
                } else {
                    score.duration()
                };
                ensure!(
                    start < end && end <= score.duration(),
                    "record-at NAME START [END] must lie inside arrangement"
                );
                let (bar, _, _, _) = clock.position(clock.beat(start));
                let pre_roll = clock
                    .seconds(
                        clock.bar_beat(
                            bar.saturating_sub(self.config.recording.pre_roll_bars)
                                .max(1),
                        ),
                    )
                    .min(start);
                self.loop_enabled = loop_capture;
                self.pending_record = Some((arg(1)?, "flac".into(), start, end));
                self.work.send(Job::Transport(
                    self.project.clone(),
                    if loop_capture { start } else { pre_roll },
                    Some(end),
                    loop_capture,
                    if loop_capture { Some(start) } else { None },
                    self.config.recording.count_in_bars,
                    self.config.recording.metronome,
                ))?;
            }
            "take-replay" => self
                .work
                .send(Job::TakePreview(self.project.clone(), arg(1)?.parse()?))?,
            "takes" => {
                for take in crate::recording::takes(&self.project.root)? {
                    self.log(format!(
                        "Take {}: {} at {:.3}s, latency {:.1}ms",
                        take.id, take.file, take.at, take.latency_ms
                    ));
                }
            }
            "take-use" => {
                let id: u32 = arg(1)?.parse()?;
                let take = crate::recording::takes(&self.project.root)?
                    .into_iter()
                    .find(|t| t.id == id)
                    .context("unknown take ID")?;
                let score = language::parse(&self.project.source)?;
                let at = if let Some(value) = v.get(2) {
                    score.clock()?.parse(value, &score.markers)?
                } else {
                    take.at
                };
                let trim = if v.len() >= 5 {
                    Some((
                        language::seconds(&arg(3)?, score.tempo)?,
                        language::seconds(&arg(4)?, score.tempo)?,
                    ))
                } else {
                    None
                };
                let name = format!("take-{}-part-{}", id, score.tracks.len() + 1);
                let addition = crate::recording::clip(&take, &name, at, trim)?;
                self.project.source.push_str(&addition);
                self.dirty = true;
            }
            "record" => {
                ensure!(
                    self.recording.is_none()
                        && self.finishing.is_none()
                        && self.pending_record.is_none(),
                    "a take is recording or finalizing"
                );
                let score = language::parse(&self.project.source)?;
                self.recording = Some(Recording::start(
                    &self.project.root,
                    &audio::RecordingConfig {
                        name: &arg(1)?,
                        format: v.get(2).map(String::as_str).unwrap_or(&self.record_format),
                        rate: score.rate,
                        bits: score.bits,
                        channels: self.channels,
                        input_name: self.input.as_deref(),
                        output_name: self.output.as_deref(),
                        monitoring: self.monitor,
                        gate: None,
                    },
                )?);
                self.recording_placement =
                    Some(self.playback.as_ref().map_or(self.cursor_seconds, |p| {
                        p.state.frames.load(std::sync::atomic::Ordering::Relaxed) as f64
                            / p.rate as f64
                    }));
                self.log("Recording real input; record-stop finalizes a unique take");
            }
            "record-stop" => {
                let r = self
                    .recording
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("no recording active"))?;
                self.finishing = Some(r.finish());
                self.log("Capture stopped; finalizing take in background");
            }
            "voice-new" => {
                let path = arg(1)?;
                let rel = std::path::Path::new(&path);
                ensure!(
                    !rel.is_absolute()
                        && rel
                            .components()
                            .all(|c| matches!(c, std::path::Component::Normal(_))),
                    "voice path must be relative"
                );
                crate::codec::directory(
                    &self.project.root,
                    rel.parent()
                        .unwrap_or(std::path::Path::new(""))
                        .to_str()
                        .unwrap_or(""),
                )?;
                crate::voice::write_template(&self.project.root.join(path), &arg(2)?)?;
                self.log("Created voicebank manifest; add authorized phoneme recordings and metadata before use");
            }
            "voice-check" => self
                .work
                .send(Job::Validate(self.project.clone(), arg(1)?))?,
            "recover-draft" => {
                let path = crate::codec::safe_path(&self.project.root, "recovery.sb")?;
                ensure!(
                    std::fs::metadata(&path)?.len() <= 1_048_576,
                    "draft exceeds limit"
                );
                self.history.checkpoint(&self.project.source);
                self.project.source = std::fs::read_to_string(path)?;
                self.dirty = true;
            }
            "undo" => {
                if let Some(s) = self.history.undo(&self.project.source) {
                    self.project.source = s;
                    self.dirty = true;
                }
            }
            "redo" => {
                if let Some(s) = self.history.redo(&self.project.source) {
                    self.project.source = s;
                    self.dirty = true;
                }
            }
            "tracks" => self.show_tracks = !self.show_tracks,
            "console" => self.show_console = !self.show_console,
            "find" => {
                self.search = arg(1)?;
                self.search_open = true;
                self.search_from = 0;
                self.pending_find = true;
            }
            _ => bail!("unknown command {}; :help", v[0]),
        }
        if ["seek", "jump", "range", "loop", "restart", "home", "stop"].contains(&v[0].as_str()) {
            crate::timeline::Transport {
                cursor: self.cursor_seconds,
                range: self.playback_range,
                looping: self.loop_enabled,
            }
            .save(&self.project.root)?;
        }
        Ok(())
    }
}
fn pitch_history(
    ui: &mut egui::Ui,
    history: &std::collections::VecDeque<Option<f64>>,
    reference: Option<f64>,
    color: egui::Color32,
) {
    ui.small("Recent pitch · ±100 cents (out-of-range values reach the edge)");
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 64.), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
    );
    let Some(reference) = reference else {
        return;
    };
    let mut previous = None;
    for (i, hz) in history.iter().enumerate() {
        let point = hz.map(|hz| {
            egui::pos2(
                rect.right() - (history.len() - 1 - i) as f32 / 59. * rect.width(),
                rect.center().y
                    - (crate::tuner::cents(hz, reference) / 100.).clamp(-1., 1.) as f32
                        * rect.height()
                        * 0.45,
            )
        });
        if let (Some(a), Some(b)) = (previous, point) {
            ui.painter()
                .line_segment([a, b], egui::Stroke::new(1.0_f32, color));
        }
        previous = point;
    }
}
fn device_combo(ui: &mut egui::Ui, label: &str, names: &[String], selected: &mut Option<String>) {
    let available = audio::DeviceList::available(names, selected.as_deref());
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!(
            "{label}: {}{}",
            selected.as_deref().unwrap_or("System Default"),
            if available { "" } else { " (unavailable)" }
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "System Default");
            for name in names {
                ui.selectable_value(selected, Some(name.clone()), name);
            }
        });
    if !available {
        ui.small("Device unavailable; reconnect, refresh, or choose another device.");
    }
}
fn compact_peaks(peaks: &[f32], bins: usize) -> Vec<f32> {
    if peaks.is_empty() || bins == 0 {
        return vec![];
    }
    let bins = bins.min(peaks.len());
    (0..bins)
        .map(|i| {
            peaks[i * peaks.len() / bins..(i + 1) * peaks.len() / bins]
                .iter()
                .copied()
                .fold(0f32, |a, b| a.max(b.abs()))
        })
        .collect()
}
fn renderer_peaks(renderer: &Renderer) -> Vec<f32> {
    // Each clip contributes at most 256 bins, with a hard display bound.
    let mut peaks = Vec::new();
    for clip in renderer.tracks.iter().flat_map(|t| &t.clips).take(64) {
        peaks.extend(compact_peaks(&clip.audio.waveform, 256));
    }
    compact_peaks(&peaks, 2048)
}
fn push_sample(
    peaks: &mut std::collections::VecDeque<f32>,
    peak: &mut f32,
    count: &mut usize,
    sample: [f32; 2],
) {
    *peak = (*peak).max(sample[0].abs().max(sample[1].abs()));
    *count += 1;
    if *count == 128 {
        push_peak(peaks, *peak);
        *count = 0;
        *peak = 0.;
    }
}
fn push_peak(peaks: &mut std::collections::VecDeque<f32>, peak: f32) {
    if peaks.len() == 512 {
        peaks.pop_front();
    }
    peaks.push_back(peak);
}
fn paint_peaks(ui: &mut egui::Ui, peaks: &[f32], height: f32, color: egui::Color32) -> egui::Rect {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let peaks = compact_peaks(peaks, (rect.width() as usize).max(1));
    for (i, peak) in peaks.iter().enumerate() {
        let x = rect.left() + (i as f32 + 0.5) * rect.width() / peaks.len() as f32;
        let y = peak.min(1.) * rect.height() * 0.45;
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.center().y - y),
                egui::pos2(x, rect.center().y + y),
            ],
            egui::Stroke::new(1.0_f32, color),
        );
    }
    rect
}
fn highlight(text: &str, palette: Palette, size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for line in text.split_inclusive('\n') {
        let mut quoted = false;
        let mut comment = false;
        for token in line.split_inclusive(|c: char| c.is_whitespace()) {
            if !quoted && token.trim_start().starts_with('#') {
                comment = true;
            }
            let color = if comment {
                palette.text
            } else if quoted || token.contains('"') {
                palette.string
            } else if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                palette.number
            } else {
                palette.keyword
            };
            if token.matches('"').count() % 2 == 1 {
                quoted = !quoted;
            }
            for (index, c) in token.char_indices() {
                let color = if !comment
                    && (quoted || token.contains('"'))
                    && "[]<>{},|*/!@?()%~".contains(c)
                {
                    palette.accent
                } else {
                    color
                };
                job.append(
                    &token[index..index + c.len_utf8()],
                    0.,
                    TextFormat {
                        font_id: egui::FontId::monospace(size),
                        color,
                        ..Default::default()
                    },
                );
            }
        }
    }
    job
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(result) = self.watcher.poll() {
            match result {
                Ok(config) => self.accept_config(ctx, config),
                Err(e) => self.log(format!("CONFIG: {e:#}; keeping current theme")),
            }
        }
        if self.theme_pending {
            self.theme_pending = false;
            match self.config.apply(ctx) {
                Ok(()) => {
                    match crate::locale::Catalog::load(
                        &self.config.language.interface,
                        crate::phonetics::resource_root().as_deref(),
                    ) {
                        Ok(catalog) => self.catalog = catalog,
                        Err(e) => self.log(format!(
                            "Language resources: {e:#}; keeping previous interface"
                        )),
                    }
                    self.font_attempted = false;
                    self.palette = self.config.palette().expect("validated theme");
                    self.show_tracks = self.config.appearance.show_tracks;
                    self.show_console = self.config.appearance.show_console;
                }
                Err(e) => self.log(format!("CONFIG: {e:#}")),
            }
        }

        if let Ok(reply) = self.work.rx.try_recv() {
            self.work.busy = false;
            self.work.replacing_document = false;
            match reply {
                Err(e) => {
                    self.pending_record = None;
                    self.pending_play = false;
                    self.log(format!("ERROR: {e:#}"));
                }
                Ok(Reply::Language(messages)) => {
                    self.font_attempted = false;
                    for message in messages {
                        self.log(message);
                    }
                    match self.watcher.load() {
                        Ok(config) => {
                            self.accept_config(ctx, config);
                        }
                        Err(e) => self.log(format!("CONFIG: {e:#}")),
                    }
                }
                Ok(Reply::Studio(output)) => {
                    for line in &output.view.report {
                        self.log(line.clone());
                    }
                    if let Some(take) = output
                        .view
                        .session
                        .selected
                        .and_then(|i| output.view.session.takes.get(i))
                    {
                        for warning in &take.analysis.warnings {
                            self.log(format!("TAKE: {warning}"));
                        }
                    }
                    self.studio = Some(output.view);
                    self.show_tracks = true;
                    if let Some(renderer) = output.preview {
                        match Playback::start(renderer, self.output.as_deref()) {
                            Ok(p) => {
                                self.playback = Some(p);
                                self.audition = true;
                            }
                            Err(e) => self.log(format!("Preview audio: {e:#}")),
                        }
                    }
                }
                Ok(Reply::Transport(r, source)) => {
                    self.waveform = renderer_peaks(&r);
                    self.last_evaluated = source;
                    self.preview = Some(r.score.clone());
                    let result = self
                        .stop()
                        .and_then(|_| Playback::start(*r, self.output.as_deref()));
                    match result {
                        Ok(p) => {
                            if let Some((name, format, start, end)) = self.pending_record.take() {
                                let gate = crate::recording::Gate {
                                    transport: p.state.clone(),
                                    start: (start * p.rate as f64).round() as u64,
                                    end: (end * p.rate as f64).round() as u64,
                                    looping: p
                                        .state
                                        .loop_end
                                        .load(std::sync::atomic::Ordering::Relaxed)
                                        > 0,
                                    spans: std::sync::Arc::new(Default::default()),
                                };
                                match Recording::start(
                                    &self.project.root,
                                    &audio::RecordingConfig {
                                        name: &name,
                                        format: &format,
                                        rate: p.rate,
                                        bits: self.preview.as_ref().map_or(24, |s| s.bits),
                                        channels: self.channels,
                                        input_name: self.input.as_deref(),
                                        output_name: self.output.as_deref(),
                                        monitoring: self.monitor,
                                        gate: Some(&gate),
                                    },
                                ) {
                                    Ok(recording) => {
                                        self.recording = Some(recording);
                                        self.recording_placement = Some(start);
                                    }
                                    Err(e) => self.log(format!("Recording: {e:#}")),
                                }
                            }
                            self.playback = Some(p);
                        }
                        Err(e) => {
                            self.pending_record = None;
                            self.log(format!("Transport: {e:#}"));
                        }
                    }
                }
                Ok(Reply::Prepared(r, source)) => {
                    let r = *r;
                    self.waveform = renderer_peaks(&r);
                    self.last_evaluated = source;
                    let result = if let Some(p) = &mut self.playback {
                        p.replace(r)
                    } else if self.pending_play {
                        Playback::start(r, self.output.as_deref()).map(|p| self.playback = Some(p))
                    } else {
                        Ok(())
                    };
                    self.pending_play = false;
                    match result {
                        Ok(()) => self.log(
                            "Evaluated; active playback changes at the next available bar boundary",
                        ),
                        Err(e) => self.log(format!("ERROR: {e:#}")),
                    }
                }
                Ok(Reply::Waveform(peaks)) => self.waveform = peaks,
                Ok(Reply::Drafted) => {}
                Ok(Reply::Saved(s)) => {
                    if s == self.project.source {
                        self.dirty = false;
                    }
                    self.log("Saved atomically");
                    self.remember_project();
                }
                Ok(Reply::Message(s)) => self.log(s),
                Ok(Reply::Project(p)) => {
                    self.studio = None;
                    self.project = p;
                    self.remember_project();
                    self.waveform.clear();
                    self.live_waveform.clear();
                    self.scope_source = 0;
                    self.preview = language::parse(&self.project.source).ok();
                    self.preview_source = self.project.source.clone();
                    self.queue_first_asset();
                    let state =
                        crate::timeline::Transport::load(&self.project.root).unwrap_or_default();
                    self.cursor_seconds = state.cursor;
                    self.playback_range = state.range;
                    self.loop_enabled = state.looping;
                    self.dirty = false;
                    self.last_evaluated = self.project.source.clone();
                    self.history = History::default();
                    self.log("Project opened");
                }
                Ok(Reply::Imported(path)) => {
                    self.asset_pending = Some(self.project.root.join(&path));
                    self.history.checkpoint(&self.project.source);
                    self.project.source.push_str(&format!(
                        "\ntrack imported{} {{\n import \"{path}\"\n}}\n",
                        self.commands.len()
                    ));
                    self.dirty = true;
                    self.log(format!("Imported {path}"));
                }
            }
        }
        if self.playback.as_ref().is_some_and(|p| {
            p.state
                .edit_missed
                .swap(false, std::sync::atomic::Ordering::Relaxed)
        }) {
            if let Some(p) = &mut self.playback {
                p.clear_missed_clock();
            }
            self.log("Live edit missed its boundary; original playback retained. Evaluate again.");
        }
        if self.finishing.as_ref().is_some_and(|w| w.is_finished()) {
            let result = self
                .finishing
                .take()
                .unwrap()
                .join()
                .map_err(|_| anyhow::anyhow!("recording worker panicked"))
                .and_then(|r| r);
            match result {
                Ok(path) => {
                    self.asset_pending = Some(path.clone());
                    if let Some((bank, capture)) = self.bank_capture.take() {
                        self.bank_pending = Some((bank, path, capture));
                        self.log("Take finalized; waiting for bank analysis worker");
                    } else {
                        if let Some(at) = self.recording_placement.take() {
                            let result = crate::recording::register(
                                &self.project.root,
                                &path,
                                at,
                                self.config.recording.latency_ms,
                            )
                            .and_then(|take| {
                                crate::recording::clip(
                                    &take,
                                    &format!("take-{}", take.id),
                                    take.at,
                                    None,
                                )
                            });
                            match result {
                                Ok(clip) => {
                                    self.project.source.push_str(&clip);
                                    self.dirty = true;
                                }
                                Err(e) => {
                                    self.log(format!("Take preserved, placement failed: {e:#}"))
                                }
                            }
                        }
                        self.log(format!(
                            "Take preserved: {} (project path {}). Timeline captures are placed automatically.",
                            path.display(),
                            crate::codec::script_path(
                                path.strip_prefix(&self.project.root).unwrap_or(&path)
                            )
                        ));
                    }
                }
                Err(e) => {
                    self.bank_capture = None;
                    self.log(format!("ERROR: {e:#}"));
                }
            }
        }
        if !self.work.busy
            && let Some((bank, path, capture)) = self.bank_pending.take()
            && let Err(e) = self.work.send(Job::Capture(
                self.project.clone(),
                bank,
                path.clone(),
                capture,
            ))
        {
            self.log(format!(
                "Bank import failed: {e:#}; recording preserved at {}",
                path.display()
            ));
        }
        if self.preview_source != self.project.source {
            self.preview = language::parse(&self.project.source).ok();
            self.preview_source = self.project.source.clone();
        }
        if let Some(p) = &self.playback {
            self.output_peak = p.state.peak();
            if p.state
                .device_error
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                self.log("ERROR: playback/device failed; stop and reselect device");
                let _ = self.stop();
            }
        }
        if self
            .playback
            .as_ref()
            .is_some_and(|p| p.state.drained.load(std::sync::atomic::Ordering::Relaxed))
        {
            if let Err(e) = self.stop() {
                self.log(e.to_string());
            } else {
                self.log("Playback complete");
            }
        }
        if let Some(r) = &self.recording {
            self.input_peak = r.state.peak();
        }
        if self
            .recording
            .as_ref()
            .is_some_and(|r| r.state.stopped.load(std::sync::atomic::Ordering::Relaxed))
            && let Err(e) = self.command("record-stop")
        {
            self.log(e.to_string());
        }
        for (key, command) in [
            (
                egui::Key::Space,
                if self
                    .playback
                    .as_ref()
                    .is_some_and(|p| p.state.paused.load(std::sync::atomic::Ordering::Relaxed))
                {
                    "resume"
                } else {
                    "pause"
                },
            ),
            (
                egui::Key::R,
                if self.recording.is_some() {
                    "record-stop"
                } else {
                    "record-at take 0b"
                },
            ),
            (egui::Key::Home, "restart"),
        ] {
            let modifiers = if key == egui::Key::Space {
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::CTRL
            };
            if ctx.input_mut(|i| i.consume_key(modifiers, key))
                && let Err(e) = self.command(command)
            {
                self.log(e.to_string());
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.help.open = true;
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::E)
        }) {
            let name = format!(
                "render mix-{}.flac",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            if let Err(e) = self.command(&name) {
                self.log(e.to_string());
            }
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::L)
        }) {
            let command = if self.loop_enabled {
                "loop off"
            } else {
                "loop on"
            };
            if let Err(e) = self.command(command) {
                self.log(e.to_string());
            }
        }
        for (key, prefix) in [(egui::Key::G, "seek "), (egui::Key::J, "jump ")] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key)) {
                self.command = prefix.into();
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("command-input")));
            }
        }
        let eval = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter));
        let save = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S));
        let play = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P));
        let focus = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L));
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::E)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("composition")));
        }
        let find = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F));
        if find {
            self.search_open = true;
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("find-input")));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3)) {
            self.pending_find = true;
        }
        if eval && let Err(e) = self.evaluate(false) {
            self.log(e.to_string());
        }
        if save && let Err(e) = self.work.send(Job::Save(self.project.clone())) {
            self.log(e.to_string());
        }
        if play {
            let command = if self.playback.is_some() {
                "stop"
            } else {
                "play"
            };
            let r = self.command(command);
            if let Err(e) = r {
                self.log(e.to_string());
            }
        }
        if !self.work.busy
            && let Some(path) = self.asset_pending.take()
            && let Err(e) = self.work.send(Job::Waveform(self.project.clone(), path))
        {
            self.log(format!("Waveform: {e:#}"));
        }
        if let Some(playback) = &mut self.playback {
            let mut count = 0;
            while count < 8192 {
                let Ok(sample) = playback.scope.pop() else {
                    break;
                };
                count += 1;
                if self.scope_source == 0 {
                    push_sample(
                        &mut self.live_waveform,
                        &mut self.scope_peak,
                        &mut self.scope_samples,
                        sample,
                    );
                }
            }
            for _ in 0..64 {
                let Ok(peaks) = playback.track_scope.pop() else {
                    break;
                };
                if self.scope_source > 0 && self.scope_source <= 128 {
                    push_peak(&mut self.live_waveform, peaks[self.scope_source - 1]);
                }
            }
        } else {
            self.live_waveform.clear();
            self.scope_peak = 0.;
            self.scope_samples = 0;
        }
        // Hold interval peaks so readable numbers do not hide short transients.
        self.readout_pending[0] = self.readout_pending[0].max(if self.playback.is_some() {
            self.output_peak
        } else {
            0.
        });
        self.readout_pending[1] = self.readout_pending[1].max(if self.recording.is_some() {
            self.input_peak
        } else {
            0.
        });
        if self.readout_at.elapsed() >= Duration::from_millis(250) {
            self.readout_at = Instant::now();
            self.readout_seconds = self.playback.as_ref().map_or(self.cursor_seconds, |p| {
                p.state.frames.load(std::sync::atomic::Ordering::Relaxed) as f64 / p.rate as f64
            });
            self.readout_levels = self.readout_pending;
            self.readout_pending = [0.; 2];
        }
        if self.playback.is_none() {
            self.readout_seconds = self.cursor_seconds;
        }
        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            self.audio_controls(ui);
            let mut selected_theme = None;
            let builtins = crate::config::builtin_themes();
            let current = builtins.iter().find(|(_, theme)| theme == &self.config.theme)
                .map_or("Custom", |(name, _)| name.as_str());
            egui::ComboBox::from_id_salt("theme")
                .selected_text(format!("Theme: {current}"))
                .show_ui(ui, |ui| {
                    for (name, theme) in builtins {
                        if ui.selectable_label(theme == &self.config.theme, name).clicked() {
                            selected_theme = Some(theme.clone());
                        }
                    }
                    if let Some(path) = &self.watcher.path {
                        match crate::config::user_themes(path) {
                            Ok(themes) => {
                                for (name, theme) in themes {
                                    match theme {
                                        Ok(theme) => {
                                            if ui.selectable_label(theme == self.config.theme, name).clicked() {
                                                selected_theme = Some(theme);
                                            }
                                        }
                                        Err(e) => { ui.label(format!("{name}: {e:#}")); }
                                    }
                                }
                            }
                            Err(e) => { ui.label(format!("Themes: {e:#}")); }
                        }
                        ui.separator();
                        if let Some(parent) = path.parent() {
                            ui.small(format!("Add TOML themes in {}", parent.join("themes").display()));
                        }
                    }
                });
            if let Some(theme) = selected_theme {
                let result = self.watcher.path.as_deref()
                    .context("no configuration path available")
                    .and_then(|path| crate::config::save_theme(path, &theme));
                match result {
                    Ok(()) => {
                        let mut config = self.config.clone();
                        config.theme = theme;
                        self.accept_config(ctx, config);
                    }
                    Err(e) => self.log(format!("Theme: {e:#}")),
                }
            }
            ui.horizontal(|ui| {
                ui.colored_label(self.palette.accent, "SONGBIRD");
                ui.label(format!(
                    "{}{}",
                    self.project.root.display(),
                    if self.dirty { " *" } else { "" }
                ));
                if ui.button(self.catalog.text("Evaluate")).clicked() {
                    let _ = self.command("eval").map_err(|e| self.log(e.to_string()));
                }
                if ui
                    .button(if self.playback.is_some() {
                        "Stop"
                    } else {
                        "Play"
                    })
                    .clicked()
                {
                    let c = if self.playback.is_some() {
                        "stop"
                    } else {
                        "play"
                    };
                    let _ = self.command(c).map_err(|e| self.log(e.to_string()));
                }
                if let Some(playback) = &self.playback {
                    let paused = playback
                        .state
                        .paused
                        .load(std::sync::atomic::Ordering::Relaxed);
                    if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                        let command = if paused { "resume" } else { "pause" };
                        let _ = self.command(command).map_err(|e| self.log(e.to_string()));
                    }
                }
                if ui.button("Start").clicked() { let _ = self.command("home").map_err(|e| self.log(e.to_string())); }
                if ui.button(if self.loop_enabled { "Loop: on" } else { "Loop: off" }).clicked() {
                    let value = if self.loop_enabled { "loop off" } else { "loop on" };
                    let _ = self.command(value).map_err(|e| self.log(e.to_string()));
                }
                if ui.button(self.catalog.text("Help")).clicked() {
                    self.help.open = true;
                }
                if self.work.busy {
                    ui.spinner();
                }
                let mut destination = None;
                if let Some(score) = &self.preview {
                    for name in score.markers.keys().take(8) { if ui.small_button(name).clicked() { destination = Some(format!("jump {name:?}")); } }
                }
                if let Some(command) = destination { let _ = self.command(&command).map_err(|e| self.log(e.to_string())); }
                if self.recording.is_some() {
                    ui.colored_label(self.palette.recording, self.catalog.text("● REC"));
                }
            });
            ui.horizontal_wrapped(|ui| {
                if let Some((a,b)) = self.playback_range { ui.small(format!("Range {a:.3}–{b:.3}s{}",if self.loop_enabled { " (loop)" } else { "" })); }
                if let Some(s) = &self.preview {
                    ui.label(format!(
                        "{} Hz / {} bit",
                        s.rate, s.bits
                    ));
                }
                if let Some(score) = &self.preview
                    && let Ok(clock) = score.clock()
                {
                    let seconds = self.readout_seconds;
                    let clock = self.playback.as_ref().map_or(clock,|p|p.clock().clone());
                    let beat = clock.beat(seconds);
                    let (bar, count, _, sig) = clock.position(beat);
                    ui.monospace(format!(
                        "Bar {bar:>3} | Beat {count} · {:.1} BPM · {}/{} · {:>6.1}/{:.1}s",
                        clock.bpm(beat),
                        sig[0],
                        sig[1],
                        seconds,
                        score.duration()
                    ));
                }
                if let Some(p) = &self.playback {
                    use std::sync::atomic::Ordering::Relaxed;
                    ui.label(format!(
                        "underruns {}",
                        p.state.underruns.load(Relaxed)
                    ));
                }
                if let Some(score) = &self.preview {
                    let duration = score.duration();
                    let seconds = self.playback.as_ref().map_or(self.cursor_seconds, |p| p.state.frames.load(std::sync::atomic::Ordering::Relaxed) as f64 / p.rate as f64);
                    ui.add(egui::ProgressBar::new((seconds/duration.max(0.001)).clamp(0.,1.) as f32).fill(self.palette.selection).desired_width(140.).text(format!("{:.0}s left",(duration-self.readout_seconds).max(0.))));
                }
                ui.monospace(format!(
                    "OUT {:>4.0} dB  IN {:>4.0} dB",
                    20. * self.readout_levels[0].max(1e-6).log10(),
                    20. * self.readout_levels[1].max(1e-6).log10()
                )).on_hover_text("Peak level held over each 250 ms display interval");
                if let Some(r) = &self.recording {
                    use std::sync::atomic::Ordering::Relaxed;
                    ui.label(format!(
                        "clipped {} · dropped {}",
                        r.state.clipped.load(Relaxed),
                        r.state.dropped.load(Relaxed)
                    ));
                }
            });
        });
        let mut studio_action = None;
        if self.show_tracks {
            egui::SidePanel::right("tracks")
                .resizable(true)
                .default_width(220.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.studio.is_some() {
                            let tools_action = self.voicebank_tools(ui);
                            let coach_action = self.pitch_coach(ui);
                            studio_action = tools_action.or(coach_action);
                        }
                        if let Some(view) = &self.studio {
                            ui.heading(self.catalog.text("Voicebank Studio"));
                            ui.label(&view.name);
                            ui.monospace(&view.bank);
                            ui.colored_label(
                                self.palette.accent,
                                format!("{} {}", self.catalog.text("Target:"), view.session.target),
                            );
                            for phone in view.session.target.split('.') {
                                if let Some(p) = view.phones.iter().find(|p| p.id == phone) {
                                    ui.small(format!(
                                        "{} — {} ({})",
                                        p.id, p.description, p.prompt
                                    ));
                                }
                            }
                            ui.small(format!(
                                "{} / {} · {} Hz · {} bit · {} ch · {}",
                                view.session.register,
                                view.session.expression,
                                view.session.settings.rate,
                                view.session.settings.bits,
                                view.session.settings.channels,
                                view.session.settings.format
                            ));
                            ui.label(format!(
                                "{} published samples · {} takes",
                                view.samples,
                                view.session.takes.len()
                            ));
                            if let Some((i, take)) = view
                                .session
                                .selected
                                .and_then(|i| view.session.takes.get(i).map(|t| (i, t)))
                            {
                                ui.monospace(format!("Take {}: {}", i + 1, take.capture.phoneme));
                                ui.small(format!(
                                    "Root: {} · {}",
                                    take.root
                                        .map(|r| format!("{r:.1} Hz"))
                                        .unwrap_or("unknown".into()),
                                    match take.voiced {
                                        Some(true) => "voiced",
                                        Some(false) => "unvoiced",
                                        None => "voicing uncertain",
                                    }
                                ));
                                ui.small(format!(
                                    "onset {:.3} / sustain {:.3} / end {:.3}",
                                    take.onset, take.sustain, take.end
                                ));
                                ui.small(format!(
                                    "peak {:.3} · RMS {:.1} dB · clipped {}",
                                    take.analysis.peak,
                                    take.analysis.rms_db,
                                    take.analysis.clipped_frames
                                ));
                                for warning in &take.analysis.warnings {
                                    ui.colored_label(self.palette.recording, warning);
                                }
                            }
                            ui.small("voicebank help · voicebank guide next");
                            ui.separator();
                        }
                        ui.heading(self.catalog.text("Tracks"));
                        if let Some(s) = &self.preview {
                            for (index, t) in s.tracks.iter().enumerate() {
                                ui.monospace(format!(
                                    "{} {}",
                                    if t.mute {
                                        "×"
                                    } else if t.solo {
                                        "★"
                                    } else {
                                        "›"
                                    },
                                    t.name
                                ));
                                if let Some(p) = &self.playback
                                    && index < 128
                                {
                                    let peak = f32::from_bits(
                                        p.state.track_peaks[index / 32][index % 32]
                                            .swap(0, std::sync::atomic::Ordering::Relaxed),
                                    );
                                    ui.label(format!(
                                        "{:.1} dB{}",
                                        20. * peak.max(1e-6).log10(),
                                        if peak >= 1. { " CLIP" } else { "" }
                                    ));
                                }
                                ui.small(format!(
                                    "{} · {} clips",
                                    t.vocal_preset
                                        .as_deref()
                                        .or(t.voice.as_deref())
                                        .unwrap_or(&t.instrument),
                                    t.clips.len()
                                ));
                                ui.small(format!("Gain {:.3} · Pan {:.2}", t.gain, t.pan));
                                for (i, effect) in t.effects.iter().enumerate() {
                                    ui.small(format!(
                                        "{}. {} {:?}",
                                        i + 1,
                                        effect.kind,
                                        effect.args
                                    ));
                                }
                                ui.separator();
                            }
                        }
                        if let Some(playback) = &self.playback {
                            egui::CollapsingHeader::new("Mixer buses/channels").show(ui, |ui| {
                                use std::sync::atomic::Ordering::Relaxed;
                                for (i, name) in playback.mixer_names.iter().enumerate() {
                                    let peak = f32::from_bits(playback.state.mixer_peaks[i / 32][i % 32].swap(0, Relaxed));
                                    let clipped = playback.state.mixer_clipped[i / 32][i % 32].load(Relaxed);
                                    ui.add(egui::ProgressBar::new(peak.clamp(0., 1.)).fill(self.palette.selection).text(format!("{name} · clipped {clipped}")))
                                        .on_hover_text(format!("Sample peak {:.1} dBFS", 20. * peak.max(1e-6).log10()));
                                }
                            });
                        }
                        egui::CollapsingHeader::new("Live waveform").default_open(true).show(ui, |ui| {
                            let previous = self.scope_source;
                            egui::ComboBox::from_id_salt("scope-source").selected_text(if self.scope_source == 0 { "Master output".into() } else { format!("Track {} (rendered peaks)", self.scope_source) }).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.scope_source, 0, "Master output");
                                if let Some(score) = &self.preview {
                                    for (i, track) in score.tracks.iter().take(128).enumerate() {
                                        ui.selectable_value(&mut self.scope_source, i + 1, &track.name);
                                    }
                                }
                            });
                            if previous != self.scope_source { self.live_waveform.clear(); self.scope_peak = 0.; self.scope_samples = 0; }
                            let peaks: Vec<_> = self.live_waveform.iter().copied().collect();
                            paint_peaks(ui, &peaks, 64., self.palette.waveform);
                            if self.playback.is_none() { ui.small("Play to view actual output samples."); }
                        });
                        let waveform = self.studio.as_ref().map_or(self.waveform.as_slice(), |v| v.waveform.as_slice());
                        let rect = egui::CollapsingHeader::new(if self.studio.is_some() { "Selected take waveform" } else { "Asset waveform peaks" }).default_open(true).show(ui, |ui| {
                            if waveform.is_empty() { ui.small("No audio clip selected or evaluated. Synth audio appears in Live waveform."); }
                            paint_peaks(ui, waveform, 100., self.palette.waveform)
                        }).body_returned.unwrap_or(egui::Rect::NOTHING);
                        if let Some(view) = &self.studio
                            && rect.is_positive()
                            && view.duration > 0.
                            && let Some(take) = view
                                .session
                                .selected
                                .and_then(|i| view.session.takes.get(i))
                        {
                            for (time, color) in [
                                (take.onset, self.palette.accent),
                                (take.sustain, self.palette.number),
                                (take.release.unwrap_or(take.end), self.palette.string),
                                (take.end, self.palette.recording),
                            ] {
                                let x = rect.left()
                                    + rect.width() * (time / view.duration).clamp(0., 1.) as f32;
                                ui.painter().line_segment(
                                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                    egui::Stroke::new(1.0_f32, color),
                                );
                            }
                        }
                    });
                });
        }
        egui::TopBottomPanel::bottom("command").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(":");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .id(egui::Id::new("command-input")),
                );
                if focus {
                    response.request_focus();
                }
                if response.has_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                        let options = [
                            "play",
                            "stop",
                            "pause",
                            "resume",
                            "seek",
                            "jump",
                            "range",
                            "loop",
                            "restart",
                            "marker",
                            "eval",
                            "save",
                            "save-as",
                            "new",
                            "convert",
                            "render",
                            "render-region",
                            "stems",
                            "record",
                            "record-stop",
                            "devices",
                            "input",
                            "output",
                            "import",
                            "open",
                            "voice-new",
                            "voicebank",
                            "language",
                            "voice-check",
                            "undo",
                            "redo",
                            "monitor",
                            "channels",
                            "find",
                        ];
                        if let Some(cmd) =
                            options.iter().find(|c| c.starts_with(self.command.trim()))
                        {
                            self.command = cmd.to_string();
                        }
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !self.commands.is_empty()
                    {
                        self.history_index = self.history_index.saturating_sub(1);
                        self.command = self.commands[self.history_index].clone();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                        && !self.commands.is_empty()
                    {
                        self.history_index = (self.history_index + 1).min(self.commands.len());
                        self.command = self
                            .commands
                            .get(self.history_index)
                            .cloned()
                            .unwrap_or_default();
                    }
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let cmd = std::mem::take(&mut self.command);
                    if !cmd.trim().is_empty() {
                        self.log(format!("> {cmd}"));
                        self.commands.push(cmd.clone());
                        if self.commands.len() > 100 {
                            self.commands.remove(0);
                        }
                        self.history_index = self.commands.len();
                        if let Err(e) = self.command(&cmd) {
                            self.log(format!("ERROR: {e:#}"));
                        }
                    }
                    response.request_focus();
                }
            });
        });
        if let Some(args) = studio_action
            && let Err(e) = self.voicebank_command(&args)
        {
            self.log(format!("Voicebank: {e:#}"));
        }
        if self.show_console {
            egui::TopBottomPanel::bottom("console")
                .resizable(true)
                .default_height(130.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for s in &self.console {
                                ui.monospace(s);
                            }
                        });
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.search_open {
                ui.horizontal(|ui| {
                    ui.label(self.catalog.text("Find:"));
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search)
                                .id(egui::Id::new("find-input")),
                        )
                        .changed()
                    {
                        self.search_from = 0;
                    }
                    if ui.button(self.catalog.text("Next / F3")).clicked() {
                        self.pending_find = true;
                    }
                    if ui.button(self.catalog.text("Close")).clicked() {
                        self.search_open = false;
                    }
                    let n = if self.search.is_empty() {
                        0
                    } else {
                        self.project.source.matches(&self.search).count()
                    };
                    ui.label(format!("{n} matches"));
                });
            }
            if self.pending_find && !self.search.is_empty() {
                self.pending_find = false;
                if self.search_from > self.project.source.len()
                    || !self.project.source.is_char_boundary(self.search_from)
                {
                    self.search_from = 0;
                }
                let found = self.project.source[self.search_from..]
                    .find(&self.search)
                    .map(|i| self.search_from + i)
                    .or_else(|| self.project.source.find(&self.search));
                if let Some(start) = found {
                    let end = start + self.search.len();
                    self.search_from = end;
                    let a = self.project.source[..start].chars().count();
                    let b = a + self.search.chars().count();
                    let id = egui::Id::new("composition");
                    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(a),
                            egui::text::CCursor::new(b),
                        )));
                    state.store(ctx, id);
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
            egui::ScrollArea::both()
                .id_salt("composition-scroll")
                .show(ui, |ui| {
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _: f32| {
                        ui.fonts_mut(|f| {
                            f.layout_job(highlight(
                                text.as_str(),
                                self.palette,
                                self.config.appearance.editor_font_size,
                            ))
                        })
                    };
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.project.source)
                            .id(egui::Id::new("composition"))
                            .code_editor()
                            .interactive(!self.work.replacing_document)
                            .desired_width(f32::INFINITY)
                            .desired_rows(35)
                            .layouter(&mut layouter),
                    );
                    if response.changed() {
                        self.dirty = true;
                    }
                    if response.has_focus()
                        && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Space))
                    {
                        let id = egui::Id::new("composition");
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, id)
                            && let Some(range) = state.cursor.char_range()
                            && range.is_empty()
                        {
                            let cursor = range.primary.index;
                            let byte = self
                                .project
                                .source
                                .char_indices()
                                .nth(cursor)
                                .map_or(self.project.source.len(), |(i, _)| i);
                            let prefix_start = self.project.source[..byte]
                                .char_indices()
                                .rev()
                                .find(|(_, c)| c.is_whitespace() || *c == '"')
                                .map_or(0, |(i, c)| i + c.len_utf8());
                            let prefix = &self.project.source[prefix_start..byte];
                            let options = [
                                "notation mini",
                                "cycle 4",
                                "cycles 4",
                                "seed 42",
                                "kick(3,8)",
                                "hat(5,8,2)",
                            ];
                            if !prefix.is_empty()
                                && let Some(value) =
                                    options.iter().find(|value| value.starts_with(prefix))
                            {
                                let position = self.project.source[..prefix_start].chars().count()
                                    + value.chars().count();
                                self.project.source.replace_range(prefix_start..byte, value);
                                state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(
                                        egui::text::CCursor::new(position),
                                    )));
                                state.store(ctx, id);
                                self.dirty = true;
                            }
                        }
                    }
                });
        });
        if self.dirty && !self.work.busy && self.draft_at.elapsed() > Duration::from_secs(5) {
            self.draft_at = Instant::now();
            if let Err(e) = self.work.send(Job::Draft(self.project.clone())) {
                self.log(format!("Draft save failed: {e}"));
            }
        }
        if let Some(frames) = &mut self.smoke_frames {
            *frames += 1;
            if *frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
        let needs_font = self.catalog.language.starts_with("ja")
            || self
                .project
                .source
                .chars()
                .any(|c| ('\u{2e80}'..='\u{9fff}').contains(&c))
            || self
                .studio
                .as_ref()
                .is_some_and(|s| s.language.starts_with("ja"));
        if !self.font_attempted && (needs_font || self.config.language.font_file.is_some()) {
            self.font_attempted = true;
            match crate::locale::font(ctx,&self.config,needs_font){Ok(true)=>{},Ok(false)=>self.log("CJK text needs an installed CJK font; set language.font_file or install Noto Sans CJK, then language reload"),Err(e)=>self.log(format!("Language font: {e:#}"))}
        }
        self.help.show(ctx, &self.catalog);
        ctx.request_repaint_after(
            if self.playback.is_some() || self.recording.is_some() || self.work.busy {
                Duration::from_millis(33)
            } else {
                Duration::from_millis(500)
            },
        );
    }
}
pub fn run(project: Project) -> Result<()> {
    let mut app = App::new(project);
    app.remember_project();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([
            app.config.appearance.window_width,
            app.config.appearance.window_height,
        ]),
        ..Default::default()
    };
    eframe::run_native(
        "Songbird",
        options,
        Box::new(move |cc| {
            app.config.apply(&cc.egui_ctx)?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI: {e}"))
}

impl Drop for App {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.project.save_draft();
        }
        if let Some(worker) = self.finishing.take() {
            let _ = worker.join();
        }
    }
}
pub fn smoke(project: Project, bank: Option<String>) -> Result<()> {
    let view = bank
        .map(|b| studio::Studio::open(&project.root, &b)?.view(vec![]))
        .transpose()?;
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Songbird GUI smoke",
        options,
        Box::new(move |_| {
            let mut app = App::new(project);
            app.studio = view;
            app.smoke_frames = Some(0);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI smoke: {e}"))
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn peaks_preserve_transients_and_stay_bounded() {
        assert_eq!(compact_peaks(&[0., -0.9, 0.2, 0.4], 2), vec![0.9, 0.4]);
        assert!(compact_peaks(&[], 100).is_empty());
        assert_eq!(compact_peaks(&[0.5], 500), vec![0.5]);
        let mut peaks = Default::default();
        let (mut peak, mut count) = (0., 0);
        for i in 0..128 * 1024 {
            push_sample(
                &mut peaks,
                &mut peak,
                &mut count,
                [if i % 128 == 0 { -0.75 } else { 0. }, 0.25],
            );
        }
        assert_eq!(peaks.len(), 512);
        assert!(peaks.iter().all(|&v| v == 0.75));
    }
    #[test]
    fn repeated_theme_deltas_preserve_editor_and_document() {
        let ctx = egui::Context::default();
        // No disk creation, microphone activation, or audio playback.
        let mut app = App::new(Project {
            root: PathBuf::from("/nonexistent-songbird-regression"),
            source: DEFAULT.into(),
        });
        app.dirty = false;
        app.theme_pending = false;
        app.project.source.push_str("\n# unsaved typing");
        let source = app.project.source.clone();
        let preview_source = app.preview_source.clone();
        let id = egui::Id::new("composition");
        let range =
            egui::text::CCursorRange::two(egui::text::CCursor::new(2), egui::text::CCursor::new(8));
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(range));
        let mut undoer = state.undoer();
        undoer.feed_state(0., &(range, "before".into()));
        undoer.feed_state(2., &(range, source.clone()));
        state.set_undoer(undoer);
        state.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
        let scroll_id = egui::Id::new("composition-scroll-test");
        let mut scroll = egui::scroll_area::State::load(&ctx, scroll_id).unwrap_or_default();
        scroll.offset = egui::vec2(12., 80.);
        scroll.store(&ctx, scroll_id);
        app.font_attempted = true;
        app.work.busy = true; // e.g. automatic draft save
        for color in ["#112233", "#223344", "#112233"] {
            let mut config = app.config.clone();
            config.theme.accent = Some(color.into());
            app.accept_config(&ctx, config.clone());
            app.accept_config(&ctx, config);
            assert_eq!(app.project.source, source);
            assert_eq!(app.preview_source, preview_source);
            assert!(app.font_attempted);
            assert!(!app.work.replacing_document);
            assert_eq!(ctx.memory(|m| m.focused()), Some(id));
            let state = egui::TextEdit::load_state(&ctx, id).unwrap();
            assert_eq!(state.cursor.char_range(), Some(range));
            assert!(state.undoer().has_undo(&(range, source.clone())));
            assert_eq!(
                egui::scroll_area::State::load(&ctx, scroll_id)
                    .unwrap()
                    .offset,
                scroll.offset
            );
        }
    }
    #[test]
    fn recording_controls_require_explicit_start_and_preserve_idle_state() {
        let mut app = App::new(Project {
            root: PathBuf::from("/nonexistent-songbird-regression"),
            source: DEFAULT.into(),
        });
        assert!(!app.monitor);
        assert!(app.recording.is_none() && app.finishing.is_none());
        assert!(app.command("record-stop").is_err());
        app.command("monitor on").unwrap();
        assert!(app.monitor && app.recording.is_none());
        app.command("monitor off").unwrap();
        app.pending_record = Some(("take".into(), "wav".into(), 0., 1.));
        assert!(app.command("record take wav").is_err());
        assert!(app.recording.is_none());
    }
}

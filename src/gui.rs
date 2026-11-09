use crate::{
    audio::{self, Playback, Recording},
    engine::Renderer,
    language,
    project::{History, Project},
};
use anyhow::{Result, bail, ensure};
use eframe::egui::{self, Color32, TextFormat};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};
const HELP: &str = "play | stop | eval | save | open DIR | save-as DIR | import FILE | render FILE | devices | input NAME | output NAME | record NAME flac|wav | record-stop | channels 1|2 | monitor on|off | voice-new PATH NAME | voice-check PATH | undo | redo | tracks | console | find TEXT | recover-draft";
pub const DEFAULT: &str = include_str!("../examples/garden/composition.sb");
enum Job {
    New(Project, PathBuf),
    Convert(Project, String, String, u32, u16),
    Draft(Project),
    Prepare(Project, Option<audio::LiveContext>),
    Render(Project, PathBuf),
    Save(Project),
    SaveAs(Project, PathBuf),
    Open(Project, PathBuf),
    Import(Project, PathBuf),
    Validate(Project, String),
}
enum Reply {
    Drafted,
    Saved(String),
    Prepared(Renderer, String),
    Message(String),
    Project(Project),
    Imported(String),
}
struct Work {
    tx: SyncSender<Job>,
    rx: Receiver<Result<Reply>>,
    busy: bool,
}
impl Work {
    fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(2);
        let (ot, or) = mpsc::sync_channel(2);
        thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                let r = (|| -> Result<Reply> {
                    Ok(match job {
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
                        Job::Prepare(p, live) => {
                            let mut r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
                            if let Some(live) = live {
                                live.preroll(&mut r)?;
                            }
                            Reply::Prepared(r, p.source)
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
        }
    }
    fn send(&mut self, job: Job) -> Result<()> {
        ensure!(!self.busy, "background operation in progress");
        self.tx
            .try_send(job)
            .map_err(|_| anyhow::anyhow!("background queue unavailable"))?;
        self.busy = true;
        Ok(())
    }
}
pub struct App {
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
    pending_play: bool,
    dirty: bool,
    draft_at: Instant,
    smoke_frames: Option<u32>,
    pub output_peak: f32,
    pub input_peak: f32,
}
impl App {
    pub fn new(project: Project) -> Self {
        let last_evaluated = project.source.clone();
        let preview = language::parse(&project.source).ok();
        let preview_source = project.source.clone();
        Self {
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
            input: None,
            output: None,
            monitor: false,
            channels: 1,
            show_tracks: true,
            show_console: true,
            search: String::new(),
            search_open: false,
            search_from: 0,
            pending_find: false,
            waveform: vec![],
            pending_play: false,
            dirty: false,
            draft_at: Instant::now(),
            smoke_frames: None,
            output_peak: 0.,
            input_peak: 0.,
        }
    }
    fn log(&mut self, s: impl Into<String>) {
        self.console.push(s.into());
        if self.console.len() > 200 {
            self.console.remove(0);
        }
    }
    fn evaluate(&mut self, play: bool) -> Result<()> {
        self.history.checkpoint(&self.last_evaluated);
        self.pending_play = play;
        self.work.send(Job::Prepare(
            self.project.clone(),
            self.playback.as_ref().map(Playback::live_context),
        ))
    }
    fn stop(&mut self) -> Result<()> {
        if let Some(mut p) = self.playback.take() {
            p.stop()?;
        }
        Ok(())
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
            "help" => self.log(HELP),
            "new" => {
                ensure!(
                    self.playback.is_none() && self.recording.is_none() && self.finishing.is_none(),
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
            "stop" => self.stop()?,
            "save" => self.work.send(Job::Save(self.project.clone()))?,
            "open" => {
                ensure!(
                    self.playback.is_none() && self.recording.is_none() && self.finishing.is_none(),
                    "stop audio and finish takes before opening a project"
                );
                self.work
                    .send(Job::Open(self.project.clone(), PathBuf::from(arg(1)?)))?;
            }
            "save-as" => {
                ensure!(
                    self.playback.is_none() && self.recording.is_none() && self.finishing.is_none(),
                    "stop audio and finish takes before Save As"
                );
                self.work
                    .send(Job::SaveAs(self.project.clone(), PathBuf::from(arg(1)?)))?;
            }
            "import" => self
                .work
                .send(Job::Import(self.project.clone(), PathBuf::from(arg(1)?)))?,
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
            "devices" => {
                for d in audio::devices()? {
                    self.log(d);
                }
            }
            "input" => {
                ensure!(self.recording.is_none(), "stop recording first");
                self.input = Some(arg(1)?);
            }
            "output" => {
                ensure!(
                    self.playback.is_none() && self.recording.is_none(),
                    "stop audio before changing output"
                );
                self.output = Some(arg(1)?);
            }
            "channels" => {
                let n = arg(1)?.parse()?;
                ensure!([1, 2].contains(&n), "channels must be 1 or 2");
                self.channels = n;
            }
            "monitor" => {
                let value = arg(1)?;
                ensure!(["on", "off"].contains(&value.as_str()), "monitor on|off");
                ensure!(self.recording.is_none(), "set monitoring before recording");
                self.monitor = value == "on";
            }
            "record" => {
                ensure!(
                    self.recording.is_none() && self.finishing.is_none(),
                    "a take is recording or finalizing"
                );
                let score = language::parse(&self.project.source)?;
                self.recording = Some(Recording::start(
                    &self.project.root,
                    &audio::RecordingConfig {
                        name: &arg(1)?,
                        format: v.get(2).map(String::as_str).unwrap_or("flac"),
                        rate: score.rate,
                        bits: score.bits,
                        channels: self.channels,
                        input_name: self.input.as_deref(),
                        output_name: self.output.as_deref(),
                        monitoring: self.monitor,
                    },
                )?);
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
        Ok(())
    }
}
fn highlight(text: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for line in text.split_inclusive('\n') {
        let mut quoted = false;
        for token in line.split_inclusive(|c: char| c.is_whitespace()) {
            let color = if token.trim_start().starts_with('#') {
                Color32::GRAY
            } else if quoted || token.contains('"') {
                Color32::from_rgb(209, 184, 120)
            } else if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                Color32::from_rgb(182, 155, 230)
            } else {
                Color32::from_rgb(112, 205, 181)
            };
            if token.matches('"').count() % 2 == 1 {
                quoted = !quoted;
            }
            job.append(
                token,
                0.,
                TextFormat {
                    font_id: egui::FontId::monospace(14.),
                    color,
                    ..Default::default()
                },
            );
        }
    }
    job
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Ok(reply) = self.work.rx.try_recv() {
            self.work.busy = false;
            match reply {
                Err(e) => {
                    self.pending_play = false;
                    self.log(format!("ERROR: {e:#}"));
                }
                Ok(Reply::Prepared(r, source)) => {
                    self.waveform = r
                        .tracks
                        .iter()
                        .flat_map(|t| {
                            t.clips
                                .iter()
                                .flat_map(|c| c.audio.waveform.iter().copied())
                        })
                        .collect();
                    self.last_evaluated = source;
                    let result = if let Some(p) = &self.playback {
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
                Ok(Reply::Drafted) => {}
                Ok(Reply::Saved(s)) => {
                    if s == self.project.source {
                        self.dirty = false;
                    }
                    self.log("Saved atomically");
                }
                Ok(Reply::Message(s)) => self.log(s),
                Ok(Reply::Project(p)) => {
                    self.project = p;
                    self.dirty = false;
                    self.last_evaluated = self.project.source.clone();
                    self.history = History::default();
                    self.log("Project opened");
                }
                Ok(Reply::Imported(path)) => {
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
                Ok(path) => self.log(format!(
                    "Take saved: {}. Add import \"{}\" to a track.",
                    path.display(),
                    path.strip_prefix(&self.project.root)
                        .unwrap_or(&path)
                        .display()
                )),
                Err(e) => self.log(format!("ERROR: {e:#}")),
            }
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
            let r = if self.playback.is_some() {
                self.stop()
            } else {
                self.evaluate(true)
            };
            if let Err(e) = r {
                self.log(e.to_string());
            }
        }
        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::from_rgb(125, 226, 186), "SONGBIRD");
                ui.label(format!(
                    "{}{}",
                    self.project.root.display(),
                    if self.dirty { " *" } else { "" }
                ));
                if ui.button("Evaluate").clicked() {
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
                if self.work.busy {
                    ui.spinner();
                }
                if self.recording.is_some() {
                    ui.colored_label(Color32::LIGHT_RED, "● REC");
                }
            });
            ui.horizontal(|ui| {
                if let Some(s) = &self.preview {
                    ui.label(format!(
                        "{} BPM  {}/{}  {} Hz / {} bit",
                        s.tempo, s.signature[0], s.signature[1], s.rate, s.bits
                    ));
                }
                if let Some(p) = &self.playback {
                    use std::sync::atomic::Ordering::Relaxed;
                    ui.label(format!(
                        "{:.2}s · underruns {}",
                        p.state.frames.load(Relaxed) as f64 / p.rate as f64,
                        p.state.underruns.load(Relaxed)
                    ));
                }
                ui.label(format!(
                    "OUT {:.1} dB  IN {:.1} dB",
                    20. * self.output_peak.max(1e-6).log10(),
                    20. * self.input_peak.max(1e-6).log10()
                ));
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
        if self.show_tracks {
            egui::SidePanel::right("tracks")
                .resizable(true)
                .default_width(220.)
                .show(ctx, |ui| {
                    ui.heading("Tracks");
                    if let Some(s) = &self.preview {
                        for t in &s.tracks {
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
                            ui.small(format!(
                                "{} · {} clips",
                                t.voice.as_deref().unwrap_or(&t.instrument),
                                t.clips.len()
                            ));
                            ui.separator();
                        }
                    }
                    ui.label("Asset waveform peaks");
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 100.),
                        egui::Sense::hover(),
                    );
                    if !self.waveform.is_empty() {
                        let p = ui.painter();
                        let bins = (rect.width() as usize).max(1);
                        for i in 0..bins {
                            let start = i * self.waveform.len() / bins;
                            let end = ((i + 1) * self.waveform.len() / bins)
                                .max(start + 1)
                                .min(self.waveform.len());
                            let y = self.waveform[start..end]
                                .iter()
                                .copied()
                                .fold(0f32, f32::max)
                                .min(1.)
                                * rect.height()
                                * 0.45;
                            let x = rect.left() + i as f32;
                            p.line_segment(
                                [
                                    egui::pos2(x, rect.center().y - y),
                                    egui::pos2(x, rect.center().y + y),
                                ],
                                egui::Stroke::new(1.0_f32, Color32::from_rgb(95, 185, 160)),
                            );
                        }
                    }
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
                            "eval",
                            "save",
                            "save-as",
                            "new",
                            "convert",
                            "render",
                            "record",
                            "record-stop",
                            "devices",
                            "input",
                            "output",
                            "import",
                            "open",
                            "voice-new",
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
                    ui.label("Find:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search)
                                .id(egui::Id::new("find-input")),
                        )
                        .changed()
                    {
                        self.search_from = 0;
                    }
                    if ui.button("Next / F3").clicked() {
                        self.pending_find = true;
                    }
                    if ui.button("Close").clicked() {
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
            egui::ScrollArea::both().show(ui, |ui| {
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _: f32| {
                    ui.fonts_mut(|f| f.layout_job(highlight(text.as_str())))
                };
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.project.source)
                        .id(egui::Id::new("composition"))
                        .code_editor()
                        .interactive(!self.work.busy)
                        .desired_width(f32::INFINITY)
                        .desired_rows(35)
                        .layouter(&mut layouter),
                );
                if response.changed() {
                    self.dirty = true;
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1120., 780.]),
        ..Default::default()
    };
    eframe::run_native(
        "Songbird",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(App::new(project)))
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
pub fn smoke(project: Project) -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Songbird GUI smoke",
        options,
        Box::new(move |_| {
            let mut app = App::new(project);
            app.smoke_frames = Some(0);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI smoke: {e}"))
}

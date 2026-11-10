use crate::{codec, engine::Renderer};
use anyhow::{Context, Result, bail, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[derive(Default)]
pub struct Meters {
    pub peak: AtomicU32,
    pub frames: AtomicU64,
    pub underruns: AtomicU64,
    pub dropped: AtomicU64,
    pub clipped: AtomicU64,
    pub device_error: AtomicBool,
    pub stopped: AtomicBool,
    pub paused: AtomicBool,
    pub loop_start: AtomicU64,
    pub loop_end: AtomicU64,
    pub count_in: AtomicU64,
    pub loop_count: AtomicU64,
    pub loop_missed: AtomicBool,
    pub track_peaks: [[AtomicU32; 32]; 4],
    pub drained: AtomicBool,
    pub edit_missed: AtomicBool,
    pub edit_frame: AtomicU64,
    pub finished: AtomicBool,
}
impl Meters {
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
    fn update(&self, x: [f32; 2]) {
        let p = x[0].abs().max(x[1].abs());
        self.peak.fetch_max(p.to_bits(), Ordering::Relaxed);
        if p >= 1. {
            self.clipped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub fn devices() -> Result<Vec<String>> {
    let h = cpal::default_host();
    let mut out = vec![];
    for d in h.input_devices()? {
        out.push(format!("input: {}", d.name()?));
    }
    for d in h.output_devices()? {
        out.push(format!("output: {}", d.name()?));
    }
    Ok(out)
}
#[derive(Default)]
pub struct DeviceList {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}
impl DeviceList {
    pub fn refresh() -> Result<Self> {
        let host = cpal::default_host();
        let mut inputs = host
            .input_devices()?
            .map(|d| d.name())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut outputs = host
            .output_devices()?
            .map(|d| d.name())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        inputs.sort();
        inputs.dedup();
        outputs.sort();
        outputs.dedup();
        Ok(Self { inputs, outputs })
    }
    pub fn available(names: &[String], selected: Option<&str>) -> bool {
        selected.is_none_or(|name| names.iter().any(|n| n == name))
    }
}
fn device(input: bool, name: Option<&str>) -> Result<cpal::Device> {
    let h = cpal::default_host();
    if let Some(name) = name {
        let list = if input {
            h.input_devices()?
        } else {
            h.output_devices()?
        };
        return list
            .into_iter()
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| anyhow::anyhow!("device unavailable: {name}"));
    }
    if input {
        h.default_input_device()
    } else {
        h.default_output_device()
    }
    .context("no audio device available")
}
fn config(
    d: &cpal::Device,
    input: bool,
    rate: u32,
    channels: Option<u16>,
) -> Result<(cpal::StreamConfig, cpal::SampleFormat)> {
    let configs: Vec<_> = if input {
        d.supported_input_configs()?.collect()
    } else {
        d.supported_output_configs()?.collect()
    };
    let mut configs = configs
        .into_iter()
        .filter(|c| {
            [1, 2].contains(&c.channels())
                && channels.is_none_or(|n| n == c.channels())
                && c.min_sample_rate().0 <= rate
                && c.max_sample_rate().0 >= rate
        })
        .collect::<Vec<_>>();
    configs.sort_by_key(|c| {
        (
            c.sample_format() != cpal::SampleFormat::F32,
            c.channels() != 2,
        )
    });
    let c = (*configs.first().context(
        "device does not support requested rate/channel count; change project rate or device",
    )?)
    .with_sample_rate(cpal::SampleRate(rate));
    Ok((c.config(), c.sample_format()))
}
fn queued_frame(rx: &mut rtrb::Consumer<[f32; 2]>, state: &Meters, missed: &mut bool) -> [f32; 2] {
    if state.stopped.load(Ordering::Relaxed) || state.paused.load(Ordering::Relaxed) {
        return [0.; 2];
    }
    match rx.pop() {
        Ok(x) => {
            if state.count_in.load(Ordering::Relaxed) > 0 {
                state.count_in.fetch_sub(1, Ordering::Relaxed);
            } else {
                let next = state.frames.fetch_add(1, Ordering::Relaxed) + 1;
                let end = state.loop_end.load(Ordering::Relaxed);
                if end > 0 && next >= end {
                    state.loop_count.fetch_add(1, Ordering::Relaxed);
                    state
                        .frames
                        .store(state.loop_start.load(Ordering::Relaxed), Ordering::Relaxed);
                }
            }
            x
        }
        Err(_) => {
            if state.finished.load(Ordering::Relaxed) {
                state.drained.store(true, Ordering::Relaxed);
            } else {
                *missed = true;
            }
            [0.; 2]
        }
    }
}
fn device_frame(sample: [f32; 2], channels: usize) -> [f32; 2] {
    let sample = if channels == 1 {
        [(sample[0] + sample[1]) * 0.5; 2]
    } else {
        sample
    };
    sample.map(|v| v.clamp(-1., 1.))
}
fn output<T: cpal::SizedSample + cpal::FromSample<f32>>(
    d: &cpal::Device,
    c: &cpal::StreamConfig,
    mut rx: rtrb::Consumer<[f32; 2]>,
    state: Arc<Meters>,
    mut scope: Option<rtrb::Producer<[f32; 2]>>,
) -> Result<cpal::Stream> {
    let channels = c.channels as usize;
    let err = state.clone();
    Ok(d.build_output_stream(
        c,
        move |data: &mut [T], _| {
            let mut missed = false;
            for out in data.chunks_mut(channels) {
                let x = queued_frame(&mut rx, &state, &mut missed);
                state.update(x);
                let audible = device_frame(x, channels);
                if let Some(scope) = &mut scope {
                    // Sample handoff only: GUI performs all visualization reduction/rendering.
                    // A full queue drops visualization samples, never stalls audio.
                    let _ = scope.push(audible);
                }
                for (ch, v) in out.iter_mut().enumerate() {
                    *v = T::from_sample(audible[ch]);
                }
            }
            if missed {
                state.underruns.fetch_add(1, Ordering::Relaxed);
            }
        },
        move |_| {
            err.device_error.store(true, Ordering::Relaxed);
            err.stopped.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}
fn output_stream(
    d: &cpal::Device,
    c: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    rx: rtrb::Consumer<[f32; 2]>,
    state: Arc<Meters>,
    scope: Option<rtrb::Producer<[f32; 2]>>,
) -> Result<cpal::Stream> {
    match format {
        cpal::SampleFormat::F32 => output::<f32>(d, c, rx, state, scope),
        cpal::SampleFormat::I16 => output::<i16>(d, c, rx, state, scope),
        cpal::SampleFormat::U16 => output::<u16>(d, c, rx, state, scope),
        cpal::SampleFormat::I32 => output::<i32>(d, c, rx, state, scope),
        cpal::SampleFormat::F64 => output::<f64>(d, c, rx, state, scope),
        _ => bail!("unsupported device sample format {format}"),
    }
}
#[derive(Clone)]
pub struct LiveContext {
    pub state: Arc<Meters>,
    pub bar: u64,
    pub rate: u32,
    pub clock: Option<crate::timeline::Clock>,
}
impl LiveContext {
    pub fn preroll(&self, next: &mut Renderer) -> Result<()> {
        ensure!(
            next.score.rate == self.rate,
            "stop playback before changing sample rate"
        );
        // Complete effect-history preroll on the preparation worker, never the playback feeder.
        for _ in 0..8 {
            ensure!(
                !self.state.stopped.load(Ordering::Relaxed),
                "playback stopped during live evaluation"
            );
            let ahead = self.state.frames.load(Ordering::Relaxed) + 16384 + self.bar;
            let target = self.clock.as_ref().map_or_else(
                || ahead.div_ceil(self.bar) * self.bar,
                |clock| clock.next_bar_frame(ahead, self.rate),
            );
            ensure!(
                target < next.length,
                "new arrangement ends before live boundary; stop and play it"
            );
            next.seek(target)?;
            if self.state.frames.load(Ordering::Relaxed) + 16384 < target {
                return Ok(());
            }
        }
        bail!("live preparation cannot keep up; stop playback before applying this edit")
    }
}
pub struct Playback {
    pub scope: rtrb::Consumer<[f32; 2]>,
    pub track_scope: rtrb::Consumer<[f32; 128]>,
    stream: Option<cpal::Stream>,
    worker: Option<JoinHandle<Result<()>>>,
    pub state: Arc<Meters>,
    replace: SyncSender<Renderer>,
    pub rate: u32,
    pub bar: u64,
    clock: crate::timeline::Clock,
    pending_clock: Option<(u64, crate::timeline::Clock)>,
}
impl Playback {
    pub fn start(mut renderer: Renderer, name: Option<&str>) -> Result<Self> {
        let rate = renderer.score.rate;
        let bar = renderer.bar_frames().max(1);
        let d = device(false, name)?;
        let (c, format) = config(&d, false, rate, None)?;
        let (mut tx, rx) = rtrb::RingBuffer::new(16384);
        let state = Arc::new(Meters::default());
        state.frames.store(renderer.position, Ordering::Relaxed);
        state
            .count_in
            .store(renderer.count_in_frames, Ordering::Relaxed);
        state.loop_start.store(
            renderer.loop_start.unwrap_or(renderer.position),
            Ordering::Relaxed,
        );
        state
            .loop_end
            .store(renderer.loop_end.unwrap_or(0), Ordering::Relaxed);
        let clock = renderer.clock.clone();
        let mut block = [[0.; 2]; 1024];
        let mut last_sent = [0.; 2];
        for _ in 0..4 {
            let n = renderer.block(&mut block)?;
            for x in &block[..n] {
                tx.push(*x)
                    .map_err(|_| anyhow::anyhow!("prefill overflow"))?;
            }
        }
        let (scope_tx, scope) = rtrb::RingBuffer::new(8192);
        let (mut track_tx, track_scope) = rtrb::RingBuffer::new(64);
        let stream = output_stream(&d, &c, format, rx, state.clone(), Some(scope_tx))?;
        let (replace, changes) = mpsc::sync_channel::<Renderer>(1);
        let s = state.clone();
        let worker = thread::spawn(move || -> Result<()> {
            let result = (|| -> Result<()> {
                let mut pending = None;
                loop {
                    if s.stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(new) = changes.try_recv() {
                        pending = Some(new);
                    }
                    if pending
                        .as_ref()
                        .is_some_and(|next: &Renderer| next.position < renderer.position)
                    {
                        pending = None;
                        s.edit_missed.store(true, Ordering::Relaxed);
                    }
                    if pending
                        .as_ref()
                        .is_some_and(|next: &Renderer| next.position == renderer.position)
                    {
                        renderer = pending.take().unwrap();
                        s.edit_frame.store(renderer.position, Ordering::Release);
                        renderer.fade_from(last_sent);
                    }
                    let boundary = renderer.bar_frames().max(1);
                    let mut n = ((boundary - renderer.position % boundary).min(1024)) as usize;
                    if let Some(next) = &pending {
                        n = n.min(next.position.saturating_sub(renderer.position) as usize);
                    }
                    let n = renderer.block(&mut block[..n])?;
                    if n == 0 {
                        break;
                    }
                    let mut peaks = [0.; 128];
                    for (dst, src) in peaks.iter_mut().zip(&renderer.channel_peaks) {
                        *dst = *src;
                    }
                    let _ = track_tx.push(peaks);
                    for (i, peak) in renderer.channel_peaks.iter().enumerate().take(128) {
                        s.track_peaks[i / 32][i % 32].fetch_max(peak.to_bits(), Ordering::Relaxed);
                    }
                    for x in &block[..n] {
                        last_sent = *x;
                        while tx.is_full() {
                            if s.stopped.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        tx.push(*x)
                            .map_err(|_| anyhow::anyhow!("audio queue disconnected"))?;
                    }
                }
                Ok(())
            })();
            s.finished.store(true, Ordering::Relaxed);
            if result.is_err() {
                s.device_error.store(true, Ordering::Relaxed);
            }
            result
        });
        if let Err(e) = stream.play() {
            state.stopped.store(true, Ordering::Relaxed);
            let _ = worker.join();
            return Err(e.into());
        }
        Ok(Self {
            scope,
            track_scope,
            stream: Some(stream),
            worker: Some(worker),
            state,
            replace,
            rate,
            bar,
            clock,
            pending_clock: None,
        })
    }
    pub fn live_context(&self) -> LiveContext {
        LiveContext {
            state: self.state.clone(),
            bar: self.bar,
            rate: self.rate,
            clock: Some(self.clock().clone()),
        }
    }
    pub fn clock(&self) -> &crate::timeline::Clock {
        if let Some((frame, clock)) = &self.pending_clock
            && self.state.frames.load(Ordering::Relaxed) >= *frame
            && self.state.edit_frame.load(Ordering::Acquire) == *frame
        {
            clock
        } else {
            &self.clock
        }
    }
    pub fn clear_missed_clock(&mut self) {
        if self
            .pending_clock
            .as_ref()
            .is_some_and(|(frame, _)| self.state.edit_frame.load(Ordering::Acquire) != *frame)
        {
            self.pending_clock = None;
        }
    }
    pub fn replace(&mut self, renderer: Renderer) -> Result<()> {
        if let Some((frame, _)) = &self.pending_clock {
            ensure!(
                self.state.frames.load(Ordering::Relaxed) >= *frame
                    && self.state.edit_frame.load(Ordering::Acquire) == *frame,
                "wait for the queued live edit to reach its audible boundary"
            );
            self.clock = self.pending_clock.take().unwrap().1;
        }
        ensure!(
            renderer.score.rate == self.rate,
            "stop playback before changing sample rate"
        );
        ensure!(
            self.state.loop_end.load(Ordering::Relaxed) == 0,
            "pause/stop transport looping before applying a live edit"
        );
        let metadata = (renderer.position, renderer.clock.clone());
        self.replace
            .try_send(renderer)
            .map_err(|_| anyhow::anyhow!("a live edit is already queued"))?;
        self.pending_clock = Some(metadata);
        Ok(())
    }
    /// Freeze queue consumption and the audible frame clock; keep prepared DSP state.
    pub fn pause(&self) -> Result<()> {
        ensure!(
            !self.state.stopped.load(Ordering::Relaxed),
            "playback is stopped"
        );
        self.state.paused.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn resume(&self) -> Result<()> {
        ensure!(
            !self.state.stopped.load(Ordering::Relaxed),
            "playback is stopped"
        );
        self.state.paused.store(false, Ordering::Relaxed);
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.state.stopped.store(true, Ordering::Relaxed);
        self.stream.take();
        if let Some(w) = self.worker.take() {
            w.join()
                .map_err(|_| anyhow::anyhow!("playback worker panicked"))??;
        }
        Ok(())
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn input<T: cpal::SizedSample + cpal::Sample>(
    d: &cpal::Device,
    c: &cpal::StreamConfig,
    mut tx: rtrb::Producer<[f32; 2]>,
    mut monitor: Option<rtrb::Producer<[f32; 2]>>,
    s: Arc<Meters>,
    gate: Option<crate::recording::Gate>,
) -> Result<cpal::Stream>
where
    f32: cpal::FromSample<T>,
{
    let ch = c.channels as usize;
    let err = s.clone();
    let mut last_loop = gate
        .as_ref()
        .map_or(0, |g| g.transport.loop_count.load(Ordering::Relaxed));
    Ok(d.build_input_stream(
        c,
        move |data: &[T], _| {
            if s.stopped.load(Ordering::Relaxed) {
                return;
            }
            for frame in data.chunks_exact(ch) {
                let x = [
                    frame[0].to_sample::<f32>(),
                    frame[ch - 1].to_sample::<f32>(),
                ];
                s.update(x);
                if let Some(m) = &mut monitor {
                    let _ = m.push(x);
                }
                if let Some(gate) = &gate {
                    if gate.looping {
                        let lap = gate.transport.loop_count.load(Ordering::Relaxed);
                        if lap != last_loop {
                            if lap > last_loop + 1 {
                                s.loop_missed.store(true, Ordering::Relaxed);
                                s.stopped.store(true, Ordering::Relaxed);
                                break;
                            }
                            let index = gate.spans.count.load(Ordering::Relaxed) as usize + 1;
                            if let Some(slot) = gate.spans.frames.get(index) {
                                slot.store(s.frames.load(Ordering::Relaxed), Ordering::Relaxed);
                                gate.spans.count.store(index as u64, Ordering::Release);
                            } else {
                                s.dropped.fetch_add(1, Ordering::Relaxed);
                                s.stopped.store(true, Ordering::Relaxed);
                                break;
                            }
                            last_loop = lap;
                        }
                    }
                    match gate.status() {
                        -1 => continue,
                        1 => {
                            s.stopped.store(true, Ordering::Relaxed);
                            break;
                        }
                        _ => {}
                    }
                }
                s.frames.fetch_add(1, Ordering::Relaxed);
                if tx.push(x).is_err() {
                    s.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        },
        move |_| {
            err.device_error.store(true, Ordering::Relaxed);
            err.stopped.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}
struct RecordingDone(Arc<Meters>);
impl Drop for RecordingDone {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Relaxed);
        self.0.finished.store(true, Ordering::Relaxed);
    }
}
pub struct Recording {
    stream: Option<cpal::Stream>,
    monitor: Option<cpal::Stream>,
    worker: Option<JoinHandle<Result<PathBuf>>>,
    pub state: Arc<Meters>,
    pub take: PathBuf,
}
#[derive(Clone, Copy)]
pub struct RecordingConfig<'a> {
    pub name: &'a str,
    pub format: &'a str,
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
    pub input_name: Option<&'a str>,
    pub output_name: Option<&'a str>,
    pub monitoring: bool,
    pub gate: Option<&'a crate::recording::Gate>,
}
impl Recording {
    pub fn start(root: &Path, settings: &RecordingConfig<'_>) -> Result<Self> {
        let RecordingConfig {
            name,
            format,
            rate,
            bits,
            channels,
            input_name,
            output_name,
            monitoring,
            gate,
        } = *settings;
        ensure!(
            ["wav", "flac"].contains(&format),
            "record format must be wav or flac"
        );
        crate::storage::preflight(
            &root.join("recordings"),
            rate as u64 * channels as u64 * 3 * 5,
            crate::phonetics::app_config().storage.minimum_free_bytes,
        )?;
        let d = device(true, input_name)?;
        let (c, f) = config(&d, true, rate, Some(channels))?;
        let take = codec::unique(&root.join("recordings"), name, "wav")?;
        let encoder = codec::Encoder::create(&take, rate, channels, bits)?;
        let (tx, mut rx) = rtrb::RingBuffer::new(32768);
        let state = Arc::new(Meters::default());
        let (monitor_tx, monitor) = if monitoring {
            let (mt, mr) = rtrb::RingBuffer::new(8192);
            let d = device(false, output_name)?;
            let (c, f) = config(&d, false, rate, None)?;
            let stream = output_stream(&d, &c, f, mr, Arc::new(Meters::default()), None)?;
            (Some(mt), Some(stream))
        } else {
            (None, None)
        };
        let stream = match f {
            cpal::SampleFormat::F32 => {
                input::<f32>(&d, &c, tx, monitor_tx, state.clone(), gate.cloned())?
            }
            cpal::SampleFormat::I16 => {
                input::<i16>(&d, &c, tx, monitor_tx, state.clone(), gate.cloned())?
            }
            cpal::SampleFormat::U16 => {
                input::<u16>(&d, &c, tx, monitor_tx, state.clone(), gate.cloned())?
            }
            cpal::SampleFormat::I32 => {
                input::<i32>(&d, &c, tx, monitor_tx, state.clone(), gate.cloned())?
            }
            cpal::SampleFormat::F64 => {
                input::<f64>(&d, &c, tx, monitor_tx, state.clone(), gate.cloned())?
            }
            _ => bail!("unsupported input sample format {f}"),
        };
        let s = state.clone();
        let path = take.clone();
        let final_flac = format == "flac";
        let root = root.to_path_buf();
        let capture_gate = gate.cloned();
        let worker = thread::spawn(move || -> Result<PathBuf> {
            let _done = RecordingDone(s.clone());
            let mut encoder = encoder;
            let result = (|| -> Result<()> {
                let mut block = Vec::with_capacity(1024);
                let mut disk_check = rate as u64 * 5;
                loop {
                    let frames = s.frames.load(Ordering::Relaxed);
                    ensure!(
                        frames <= rate as u64 * 3600,
                        "recording exceeds one-hour limit; original WAV preserved"
                    );
                    if frames >= disk_check {
                        crate::storage::preflight(
                            &root,
                            rate as u64 * channels as u64 * 3 * 5,
                            crate::phonetics::app_config().storage.minimum_free_bytes,
                        )?;
                        disk_check = frames + rate as u64 * 5;
                    }
                    while let Ok(frame) = rx.pop() {
                        block.push(frame);
                        if block.len() == 1024 {
                            encoder.write(&block)?;
                            block.clear();
                        }
                    }
                    if s.stopped.load(Ordering::Relaxed) && rx.is_empty() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                encoder.write(&block)?;
                Ok(())
            })();
            let finalized = encoder.finish();
            result?;
            finalized?;
            ensure!(
                !s.loop_missed.load(Ordering::Relaxed),
                "input callback missed loop boundaries; original WAV preserved; use a longer recording loop or smaller hardware buffer"
            );
            ensure!(
                s.dropped.load(Ordering::Relaxed) == 0,
                "recording buffer overrun; incomplete take preserved at {}",
                path.display()
            );
            ensure!(
                !s.device_error.load(Ordering::Relaxed),
                "device lost; partial take preserved at {}",
                path.display()
            );
            ensure!(
                s.frames.load(Ordering::Relaxed) > 0,
                "no input received; empty take preserved"
            );
            if let Some(gate) = &capture_gate
                && gate.looping
            {
                gate.save_spans(&path, s.frames.load(Ordering::Relaxed), rate)?;
            }
            if final_flac {
                let output = path.with_extension("flac");
                codec::convert(&path, &output, &root.join("cache"), rate, bits)?;
                Ok(output)
            } else {
                Ok(path)
            }
        });
        let recording = Self {
            stream: Some(stream),
            monitor,
            worker: Some(worker),
            state,
            take,
        };
        if let Some(m) = &recording.monitor {
            m.play()?;
        }
        if let Some(gate) = gate {
            ensure!(
                gate.transport.count_in.load(Ordering::Relaxed) > 0
                    || gate.transport.frames.load(Ordering::Relaxed) <= gate.start,
                "input device armed after punch-in; increase count-in or pre-roll and retry (take file preserved)"
            );
        }
        if let Err(e) = recording.stream.as_ref().unwrap().play() {
            recording.state.stopped.store(true, Ordering::Relaxed);
            return Err(e.into());
        }
        Ok(recording)
    }
    pub fn finish(mut self) -> JoinHandle<Result<PathBuf>> {
        self.state.stopped.store(true, Ordering::Relaxed);
        self.stream.take();
        self.monitor.take();
        self.worker.take().unwrap()
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.state.stopped.store(true, Ordering::Relaxed);
        self.stream.take();
        self.monitor.take();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    #[test]
    fn output_handoff_matches_mono_and_clipped_stereo_and_never_blocks() {
        assert_eq!(device_frame([2., -2.], 2), [1., -1.]);
        assert_eq!(device_frame([0.8, -0.2], 1), [0.3, 0.3]);
        let (mut tx, mut rx) = rtrb::RingBuffer::new(4);
        for _ in 0..1000 {
            // Simulate a GUI that sometimes stops draining the bounded handoff.
            for _ in 0..8 {
                let _ = tx.push(device_frame([0.25, -0.5], 2));
            }
            for _ in 0..4 {
                assert_eq!(rx.pop().unwrap(), [0.25, -0.5]);
            }
        }
    }
    #[test]
    fn recording_finish_stops_capture_and_retains_worker_result() {
        let state = Arc::new(Meters::default());
        let worker_state = state.clone();
        let recording = Recording {
            stream: None,
            monitor: None,
            state: state.clone(),
            take: "take.wav".into(),
            worker: Some(thread::spawn(move || {
                while !worker_state.stopped.load(Ordering::Relaxed) {
                    thread::yield_now();
                }
                Ok(PathBuf::from("take.wav"))
            })),
        };
        let worker = recording.finish();
        assert!(state.stopped.load(Ordering::Relaxed));
        assert_eq!(worker.join().unwrap().unwrap(), PathBuf::from("take.wav"));
    }
    #[test]
    fn pause_preserves_queued_pcm_and_clock_until_resume() {
        let (mut tx, mut rx) = rtrb::RingBuffer::new(4);
        tx.push([0.1, 0.2]).unwrap();
        tx.push([0.3, 0.4]).unwrap();
        let state = Meters::default();
        let mut missed = false;
        state.paused.store(true, Ordering::Relaxed);
        state.finished.store(true, Ordering::Relaxed);
        for _ in 0..20 {
            assert_eq!(queued_frame(&mut rx, &state, &mut missed), [0.; 2]);
        }
        assert_eq!(state.frames.load(Ordering::Relaxed), 0);
        assert!(!missed && !state.drained.load(Ordering::Relaxed));
        state.paused.store(false, Ordering::Relaxed);
        assert_eq!(queued_frame(&mut rx, &state, &mut missed), [0.1, 0.2]);
        assert_eq!(queued_frame(&mut rx, &state, &mut missed), [0.3, 0.4]);
        assert_eq!(state.frames.load(Ordering::Relaxed), 2);
        queued_frame(&mut rx, &state, &mut missed);
        assert!(state.drained.load(Ordering::Relaxed));
    }
}

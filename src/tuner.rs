//! Optional capture-side analysis. The input callback only copies PCM into a bounded queue.
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub fn note_name(midi: i32) -> String {
    const NOTES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!(
        "{}{}",
        NOTES[midi.rem_euclid(12) as usize],
        midi.div_euclid(12) - 1
    )
}
pub fn cents(hz: f64, target: f64) -> f64 {
    1200. * (hz / target).log2()
}
pub fn nearest_note(hz: f64) -> i32 {
    (69. + 12. * (hz / 440.).log2()).round() as i32
}

#[derive(Clone)]
pub struct Reading {
    pub hz: Option<f64>,
    pub confidence: f64,
    pub rms_db: f32,
    pub peak: f32,
    pub clipped: bool,
    pub stability_cents: Option<f64>,
    pub history: VecDeque<Option<f64>>,
    pub measured: Instant,
    pub dropped: u64,
}
struct Analyzer {
    rate: u32,
    analysis_rate: u32,
    source_frames: u64,
    bucket: u64,
    sum: f64,
    count: usize,
    window: VecDeque<f32>,
    since: usize,
    level_sum: f64,
    level_frames: usize,
    peak: f32,
    clipped: bool,
    stable: VecDeque<f64>,
    history: VecDeque<Option<f64>>,
}
impl Analyzer {
    fn new(rate: u32) -> Self {
        Self {
            rate,
            analysis_rate: rate.min(8000),
            source_frames: 0,
            bucket: 0,
            sum: 0.,
            count: 0,
            window: VecDeque::with_capacity(1024),
            since: 0,
            level_sum: 0.,
            level_frames: 0,
            peak: 0.,
            clipped: false,
            stable: VecDeque::with_capacity(10),
            history: VecDeque::with_capacity(60),
        }
    }
    fn feed(&mut self, frame: [f32; 2]) {
        if frame.iter().any(|v| !v.is_finite()) {
            self.reset_gap();
            return;
        }
        self.peak = self.peak.max(frame[0].abs().max(frame[1].abs()));
        self.clipped |= frame.iter().any(|v| v.abs() >= 0.999);
        let mono = (frame[0] as f64 + frame[1] as f64) * 0.5;
        self.level_sum += mono * mono;
        self.level_frames += 1;
        self.sum += mono;
        self.count += 1;
        self.source_frames += 1;
        let bucket = self.source_frames * self.analysis_rate as u64 / self.rate as u64;
        if bucket > self.bucket {
            if self.window.len() == 1024 {
                self.window.pop_front();
            }
            self.window.push_back((self.sum / self.count as f64) as f32);
            self.since += 1;
            self.sum = 0.;
            self.count = 0;
            self.bucket = bucket;
        }
    }
    fn reset_gap(&mut self) {
        let history = std::mem::take(&mut self.history);
        *self = Self::new(self.rate);
        self.history = history;
        if self.history.len() == 60 {
            self.history.pop_front();
        }
        self.history.push_back(None);
    }
    fn reading(&mut self) -> Option<Reading> {
        if self.window.len() < 1024 || self.since < (self.analysis_rate / 5) as usize {
            return None;
        }
        self.since = 0;
        let mut samples: Vec<_> = self.window.iter().copied().collect();
        let mean = samples.iter().map(|&v| v as f64).sum::<f64>() / samples.len() as f64;
        for v in &mut samples {
            *v -= mean as f32;
        }
        let energy =
            samples.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / samples.len() as f64;
        let estimate = if energy.sqrt() >= 0.003 && !self.clipped {
            crate::voice_analysis::pitch(&samples, self.analysis_rate as f64)
        } else {
            None
        };
        let confidence = estimate.map_or(0., |(_, confidence)| confidence);
        let hz = estimate
            .filter(|&(_, confidence)| confidence >= 0.85)
            .map(|(hz, _)| hz);
        if let Some(hz) = hz {
            if self.stable.len() == 10 {
                self.stable.pop_front();
            }
            self.stable.push_back(hz);
        } else {
            self.stable.clear();
        }
        let stability_cents = if self.stable.len() >= 4 {
            let center =
                self.stable.iter().map(|v| v.log2()).sum::<f64>() / self.stable.len() as f64;
            Some(
                (self
                    .stable
                    .iter()
                    .map(|v| (1200. * (v.log2() - center)).powi(2))
                    .sum::<f64>()
                    / self.stable.len() as f64)
                    .sqrt(),
            )
        } else {
            None
        };
        if self.history.len() == 60 {
            self.history.pop_front();
        }
        self.history.push_back(hz);
        let reading = Reading {
            hz,
            confidence,
            rms_db: (20.
                * (self.level_sum / self.level_frames.max(1) as f64)
                    .sqrt()
                    .max(1e-6)
                    .log10()) as f32,
            peak: self.peak,
            clipped: self.clipped,
            stability_cents,
            history: self.history.clone(),
            measured: Instant::now(),
            dropped: 0,
        };
        self.level_sum = 0.;
        self.level_frames = 0;
        self.peak = 0.;
        self.clipped = false;
        Some(reading)
    }
}

pub struct CaptureTap {
    producer: rtrb::Producer<[f32; 2]>,
    dropped: Arc<AtomicU64>,
}
impl CaptureTap {
    pub fn push(&mut self, frame: [f32; 2]) {
        if self.producer.push(frame).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub struct LiveTuner {
    latest: Arc<Mutex<Option<Reading>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl LiveTuner {
    pub fn start(rate: u32) -> anyhow::Result<(CaptureTap, Self)> {
        anyhow::ensure!(
            (8000..=192000).contains(&rate),
            "live tuner supports 8000–192000 Hz capture"
        );
        let capacity = (rate as usize / 4).clamp(2048, 32768);
        let (producer, mut consumer) = rtrb::RingBuffer::new(capacity);
        let dropped = Arc::new(AtomicU64::new(0));
        let lost = dropped.clone();
        let latest = Arc::new(Mutex::new(None));
        let output = latest.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let worker = thread::Builder::new()
            .name("songbird-pitch".into())
            .spawn(move || {
                let mut analyzer = Analyzer::new(rate);
                let mut previous_lost = 0;
                while !done.load(Ordering::Relaxed) {
                    // A full side queue cannot stall capture. Discard stale backlog, reset
                    // continuity, and show a gap instead of measuring across missing PCM.
                    let lost_now = lost.load(Ordering::Relaxed);
                    if lost_now != previous_lost {
                        previous_lost = lost_now;
                        let stale = consumer.slots();
                        for _ in 0..stale {
                            let _ = consumer.pop();
                        }
                        analyzer.reset_gap();
                        if let Ok(mut latest) = output.lock() {
                            *latest = None;
                        }
                    }
                    for _ in 0..capacity {
                        let Ok(frame) = consumer.pop() else {
                            break;
                        };
                        analyzer.feed(frame);
                    }
                    if lost.load(Ordering::Relaxed) != previous_lost {
                        continue;
                    }
                    if let Some(mut reading) = analyzer.reading() {
                        if lost.load(Ordering::Relaxed) != previous_lost {
                            // Do not retain a history point measured across a dropped-input gap.
                            analyzer.history.pop_back();
                            continue;
                        }
                        reading.dropped = previous_lost;
                        if let Ok(mut latest) = output.lock() {
                            *latest = Some(reading);
                        }
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            })?;
        Ok((
            CaptureTap { producer, dropped },
            Self {
                latest,
                stop,
                worker: Some(worker),
            },
        ))
    }
    pub fn reading(&self) -> Option<Reading> {
        self.latest
            .try_lock()
            .ok()?
            .as_ref()
            .filter(|r| r.measured.elapsed() < Duration::from_millis(750))
            .cloned()
    }
}
impl Drop for LiveTuner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(analyzer: &mut Analyzer, hz: f64, seconds: f64, amplitude: f32) -> Reading {
        let mut latest = None;
        for i in 0..(analyzer.rate as f64 * seconds) as usize {
            let v = (std::f64::consts::TAU * hz * i as f64 / analyzer.rate as f64).sin() as f32
                * amplitude;
            analyzer.feed([v; 2]);
            if i % (analyzer.rate as usize / 5) == 0 {
                latest = analyzer.reading().or(latest);
            }
        }
        analyzer.reading().or(latest).unwrap()
    }
    #[test]
    fn pitch_cents_stability_and_sample_rates() {
        for rate in [8000, 44100, 48000, 96000, 192000] {
            let mut analyzer = Analyzer::new(rate);
            let reading = tone(&mut analyzer, 220., 1.2, 0.2);
            assert!((reading.hz.unwrap() - 220.).abs() < 2.);
            assert!(reading.stability_cents.unwrap() < 5.);
            assert!(analyzer.window.len() <= 1024 && reading.history.len() <= 60);
        }
        assert_eq!(note_name(nearest_note(220.)), "A3");
        assert!(cents(440., 440.).abs() < 1e-9);
        assert!((cents(440. * 2f64.powf(25. / 1200.), 440.) - 25.).abs() < 1e-6);
    }
    #[test]
    fn bounded_tap_overflow_stale_results_and_worker_shutdown() {
        let (producer, _consumer) = rtrb::RingBuffer::new(4);
        let dropped = Arc::new(AtomicU64::new(0));
        let mut tap = CaptureTap {
            producer,
            dropped: dropped.clone(),
        };
        for _ in 0..10 {
            tap.push([0.; 2]);
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 6);
        let mut analyzer = Analyzer::new(8000);
        let mut reading = tone(&mut analyzer, 220., 0.5, 0.2);
        reading.measured = Instant::now() - Duration::from_secs(2);
        let tuner = LiveTuner {
            latest: Arc::new(Mutex::new(Some(reading))),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        };
        assert!(tuner.reading().is_none());
        let (_tap, worker) = LiveTuner::start(48000).unwrap();
        let stop = worker.stop.clone();
        drop(worker);
        assert!(stop.load(Ordering::Relaxed));
    }
    #[test]
    fn silence_clipping_noise_and_gaps_are_not_valid_notes() {
        let mut analyzer = Analyzer::new(48000);
        assert!(tone(&mut analyzer, 220., 0.5, 0.).hz.is_none());
        assert!(tone(&mut analyzer, 220., 0.5, 1.5).hz.is_none());
        for i in 0..12000 {
            let v = crate::dsp::noise(i, 17) * 0.2;
            analyzer.feed([v; 2]);
        }
        assert!(analyzer.reading().unwrap().hz.is_none());
        analyzer.reset_gap();
        assert!(analyzer.stable.is_empty() && analyzer.reading().is_none());
        for _ in 0..100 {
            analyzer.reset_gap();
        }
        assert_eq!(analyzer.history.len(), 60);
        assert!(LiveTuner::start(0).is_err());
    }
}

use crate::language::{Effect, Track};
use std::f32::consts::{PI, TAU};
pub fn hz(midi: f64) -> f64 {
    440. * 2f64.powf((midi - 69.) / 12.)
}
pub fn noise(index: u64, seed: u64) -> f32 {
    let mut x = index.wrapping_add(seed).wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((x ^ (x >> 31)) as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32
}
fn blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.
    } else if t > 1. - dt {
        let t = (t - 1.) / dt;
        t * t + t + t + 1.
    } else {
        0.
    }
}
pub fn oscillator(kind: &str, phase: f64, freq: f64, rate: u32, index: u64) -> f32 {
    let t = phase.fract();
    let dt = (freq / rate as f64).min(0.49);
    let x = match kind {
        "sine" | "soft-lead" => (t * std::f64::consts::TAU).sin(),
        "saw" | "analog" => 2. * t - 1. - blep(t, dt),
        "square" => (if t < 0.5 { 1. } else { -1. }) + blep(t, dt) - blep((t + 0.5) % 1., dt),
        "triangle" => {
            let mut x = 0.;
            for n in (1..=15).step_by(2) {
                if freq * n as f64 >= rate as f64 * 0.5 {
                    break;
                }
                x += if n % 4 == 1 { 1. } else { -1. }
                    * (t * std::f64::consts::TAU * n as f64).sin()
                    / (n * n) as f64;
            }
            x * 8. / std::f64::consts::PI.powi(2)
        }
        "noise" => noise(index, 71) as f64,
        _ => 0.,
    };
    x as f32
}
/// Shared voice path for live playback, preview, and offline export.
pub fn instrument(kind: &str, phase: f64, freq: f64, rate: u32, index: u64, elapsed: f64) -> f32 {
    if crate::instruments::NAMES.contains(&kind) {
        crate::instruments::sample(kind, phase, freq, rate, elapsed)
    } else {
        oscillator(kind, phase, freq, rate, index)
    }
}
pub fn envelope(t: f64, duration: f64, track: &Track) -> f32 {
    if t < 0. || t >= duration {
        return 0.;
    }
    let a = track.attack as f64;
    let d = track.decay as f64;
    let r = (track.release as f64).min(duration / 2.);
    let base = if t < a {
        t / a.max(1e-9)
    } else if t < a + d {
        1. - (1. - track.sustain as f64) * (t - a) / d.max(1e-9)
    } else {
        track.sustain as f64
    };
    (base * ((duration - t) / r.max(1e-9)).min(1.)) as f32
}
pub const DRUM_NAMES: &[&str] = &[
    "kick",
    "snare",
    "hat",
    "open-hat",
    "clap",
    "rimshot",
    "crash",
    "ride",
    "high-tom",
    "mid-tom",
    "low-tom",
    "shaker",
    "cowbell",
    "tambourine",
];
pub fn drum(kind: &str, t: f64, index: u64) -> f32 {
    if t < 0. {
        return 0.;
    }
    match kind {
        "kick" => {
            let phase = 45. * t + 100. * 0.025 * (1. - (-t / 0.025).exp());
            (phase as f32 * TAU).sin() * (-t as f32 * 15.).exp() * 0.8
        }
        "snare" => {
            noise(index, 123) * (-t as f32 * 24.).exp() * 0.5
                + (t as f32 * 180. * TAU).sin() * (-t as f32 * 32.).exp() * 0.3
        }
        "hat" => noise(index, 56) * (-t as f32 * 90.).exp() * 0.3,
        "open-hat" => noise(index, 56) * (-t as f32 * 6.).exp() * 0.3,
        "clap" => noise(index, 971) * (-t as f32 * 22.).exp() * 0.4,
        "rimshot" => {
            ((t as f32 * 1700. * TAU).sin() * 0.35 + noise(index, 91) * 0.2)
                * (-t as f32 * 75.).exp()
        }
        "high-tom" | "mid-tom" | "low-tom" => {
            let frequency = match kind {
                "high-tom" => 180.,
                "mid-tom" => 130.,
                _ => 90.,
            };
            let phase = frequency * (t + 0.015 * (1. - (-t / 0.03).exp()));
            (phase as f32 * TAU).sin() * (-t as f32 * 10.).exp() * 0.6
                + noise(index, 143) * (-t as f32 * 80.).exp() * 0.08
        }
        "crash" | "ride" | "tambourine" => {
            let decay = match kind {
                "crash" => 3.2,
                "ride" => 4.,
                _ => 18.,
            };
            let metallic = [1379., 2027., 2723., 3571.]
                .iter()
                .map(|frequency| (t as f32 * frequency * TAU).sin())
                .sum::<f32>()
                * 0.05;
            let noise_level = if kind == "ride" { 0.08 } else { 0.25 };
            (metallic + noise(index, 173) * noise_level) * (-t as f32 * decay).exp()
        }
        "shaker" => noise(index, 181) * (-t as f32 * 35.).exp() * 0.4,
        "cowbell" => {
            let a = (t as f32 * 560. * TAU).sin() + 0.3 * (t as f32 * 1680. * TAU).sin();
            let b = (t as f32 * 845. * TAU).sin() + 0.3 * (t as f32 * 2535. * TAU).sin();
            (a + b) * 0.13 * (-t as f32 * 14.).exp()
        }
        _ => 0.,
    }
}
pub fn pan(frame: [f32; 2], p: f32) -> [f32; 2] {
    let angle = (p.clamp(-1., 1.) + 1.) * PI / 4.;
    [frame[0] * angle.cos(), frame[1] * angle.sin()]
}
pub fn automation(t: f64, points: &[[f64; 2]], fallback: f32) -> f32 {
    if points.is_empty() {
        return fallback;
    }
    if t <= points[0][0] {
        return points[0][1] as f32;
    }
    for p in points.windows(2) {
        if t <= p[1][0] {
            let f = (t - p[0][0]) / (p[1][0] - p[0][0]);
            return (p[0][1] + f * (p[1][1] - p[0][1])) as f32;
        }
    }
    points.last().unwrap()[1] as f32
}
pub fn automation_step(t: f64, points: &[[f64; 2]], fallback: f32) -> f32 {
    if points.is_empty() {
        return fallback;
    }
    let index = points.partition_point(|p| p[0] <= t).saturating_sub(1);
    points[index][1] as f32
}
#[derive(Clone)]
pub struct Processor {
    bypassed: bool,
    biquad_state: [[f64; 2]; 2],
    biquad_coefficients: [f64; 5],
    biquad_target: [f64; 5],
    biquad_key: [f32; 3],
    gate_hold: u32,
    pub reduction_db: f32,
    spec: Effect,
    rate: f32,
    low: [f32; 2],
    high: [f32; 2],
    env: f32,
    delay: Vec<[f32; 2]>,
    position: usize,
    combs: Vec<Vec<[f32; 2]>>,
    positions: Vec<usize>,
}
impl Processor {
    pub fn new(spec: &Effect, rate: u32) -> Self {
        let delay = if spec.kind == "delay" {
            vec![[0.; 2]; (spec.args[0] * rate as f32).ceil() as usize]
        } else {
            vec![]
        };
        let combs = if spec.kind == "reverb" {
            [0.0297, 0.0371, 0.0411, 0.0437]
                .iter()
                .map(|t| vec![[0.; 2]; (t * rate as f32) as usize])
                .collect()
        } else {
            vec![]
        };
        Self {
            bypassed: false,
            biquad_state: [[0.; 2]; 2],
            biquad_coefficients: [0.; 5],
            biquad_target: [0.; 5],
            biquad_key: [f32::NAN; 3],
            gate_hold: 0,
            reduction_db: 0.,
            spec: spec.clone(),
            rate: rate as f32,
            low: [0.; 2],
            high: [0.; 2],
            env: 0.,
            delay,
            position: 0,
            combs,
            positions: vec![0; 4],
        }
    }
    pub fn automate_argument(&mut self, parameter: usize, value: f32) {
        if let Some(argument) = self.spec.args.get_mut(parameter) {
            *argument = value;
        }
    }
    pub fn state_bytes(&self) -> usize {
        (self.delay.len() + self.combs.iter().map(Vec::len).sum::<usize>())
            * std::mem::size_of::<[f32; 2]>()
    }
    pub fn automate_filter(&mut self, cutoff: f32) {
        if self.spec.kind == "filter" {
            self.spec.args[0] = cutoff;
        }
    }
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.process_with_detector(x, None)
    }
    pub fn process_with_detector(
        &mut self,
        mut x: [f32; 2],
        detector: Option<[f32; 2]>,
    ) -> [f32; 2] {
        self.reduction_db = 0.;
        if self.bypassed {
            return x;
        }
        let a = &self.spec.args;
        match self.spec.kind.as_str() {
            "highpass" | "lowpass" | "bell" | "lowshelf" | "highshelf" => {
                let key = [
                    a[0],
                    if a.len() == 3 { a[1] } else { 0. },
                    *a.last().unwrap(),
                ];
                if key != self.biquad_key {
                    let first = self.biquad_key[0].is_nan();
                    self.biquad_target = biquad(
                        &self.spec.kind,
                        self.rate as f64,
                        key[0] as f64,
                        key[1] as f64,
                        key[2] as f64,
                    );
                    if first {
                        self.biquad_coefficients = self.biquad_target;
                    }
                    self.biquad_key = key;
                }
                let smooth = 1. - (-1. / (self.rate as f64 * 0.005)).exp();
                for (coefficient, target) in
                    self.biquad_coefficients.iter_mut().zip(self.biquad_target)
                {
                    *coefficient += smooth * (target - *coefficient);
                }
                let [b0, b1, b2, a1, a2] = self.biquad_coefficients;
                for (ch, sample) in x.iter_mut().enumerate() {
                    let input = *sample as f64;
                    let output = b0 * input + self.biquad_state[ch][0];
                    self.biquad_state[ch][0] = b1 * input - a1 * output + self.biquad_state[ch][1];
                    self.biquad_state[ch][1] = b2 * input - a2 * output;
                    for state in &mut self.biquad_state[ch] {
                        if state.abs() < 1e-30 {
                            *state = 0.;
                        }
                    }
                    *sample = output as f32;
                }
            }
            "compressor" => {
                let side = detector.unwrap_or(x);
                let level = side[0].abs().max(side[1].abs());
                let attack = (-1. / (a[2] * 0.001 * self.rate)).exp();
                let release = (-1. / (a[3] * 0.001 * self.rate)).exp();
                let coefficient = if level > self.env { attack } else { release };
                self.env = coefficient * self.env + (1. - coefficient) * level;
                if self.env.abs() < 1e-20 {
                    self.env = 0.;
                }
                let over = 20. * self.env.max(1e-12).log10() - a[0];
                let slope = 1. - 1. / a[1];
                self.reduction_db = if a[4] > 0. && (-a[4] / 2. ..a[4] / 2.).contains(&over) {
                    slope * (over + a[4] / 2.).powi(2) / (2. * a[4])
                } else if over > 0. {
                    slope * over
                } else {
                    0.
                };
                let gain = 10f32.powf((a[5] - self.reduction_db) / 20.);
                x = x.map(|v| v * gain);
            }
            "noisegate" => {
                let side = detector.unwrap_or(x);
                let open = side[0].abs().max(side[1].abs()) >= 10f32.powf(a[0] / 20.);
                if open {
                    self.gate_hold = (a[3] * 0.001 * self.rate) as u32;
                } else {
                    self.gate_hold = self.gate_hold.saturating_sub(1);
                }
                let target = if open || self.gate_hold > 0 { 1. } else { 0. };
                let ms = if target > self.env { a[1] } else { a[2] };
                let coefficient = (-1. / (ms * 0.001 * self.rate)).exp();
                self.env = coefficient * self.env + (1. - coefficient) * target;
                if self.env.abs() < 1e-20 {
                    self.env = 0.;
                }
                x = x.map(|v| v * self.env);
            }
            "width" => {
                let mid = (x[0] + x[1]) * 0.5;
                let side = (x[0] - x[1]) * 0.5 * a[0];
                x = [mid + side, mid - side];
            }
            "filter" => {
                let k = 1. - (-TAU * a[0] / self.rate).exp();
                for (ch, v) in x.iter_mut().enumerate() {
                    self.low[ch] += k * (*v - self.low[ch]);
                    *v = self.low[ch];
                }
            }
            "eq" => {
                let kl = 1. - (-TAU * 250. / self.rate).exp();
                let kh = 1. - (-TAU * 4000. / self.rate).exp();
                for (ch, v) in x.iter_mut().enumerate() {
                    self.low[ch] += kl * (*v - self.low[ch]);
                    self.high[ch] += kh * (*v - self.high[ch]);
                    *v = self.low[ch] * 10f32.powf(a[0] / 20.)
                        + (self.high[ch] - self.low[ch]) * 10f32.powf(a[1] / 20.)
                        + (*v - self.high[ch]) * 10f32.powf(a[2] / 20.);
                }
            }
            "compress" => {
                let peak = x[0].abs().max(x[1].abs());
                let k = if peak > self.env { 0.01 } else { 0.0005 };
                self.env += k * (peak - self.env);
                let db = 20. * self.env.max(1e-9).log10();
                if db > a[0] {
                    let g = 10f32.powf((a[0] + (db - a[0]) / a[1] - db) / 20.);
                    self.reduction_db = -20. * g.max(1e-12).log10();
                    x[0] *= g;
                    x[1] *= g;
                }
            }
            "limit" => {
                let ceiling = 10f32.powf(a[0] / 20.);
                let peak = x[0].abs().max(x[1].abs());
                if peak > ceiling {
                    x[0] *= ceiling / peak;
                    x[1] *= ceiling / peak;
                }
            }
            "gate" | "denoise" => {
                let peak = x[0].abs().max(x[1].abs());
                let threshold = 10f32.powf(a[0] / 20.);
                let target = if self.spec.kind == "gate" {
                    if peak < threshold { 0. } else { 1. }
                } else {
                    (1. - threshold / peak.max(1e-9)).max(0.)
                };
                self.env += 0.01 * (target - self.env);
                x[0] *= self.env;
                x[1] *= self.env;
            }
            "delay" => {
                let d = self.delay[self.position];
                self.delay[self.position] = [x[0] + d[0] * a[1], x[1] + d[1] * a[1]];
                self.position = (self.position + 1) % self.delay.len();
                x = [
                    x[0] * (1. - a[2]) + d[0] * a[2],
                    x[1] * (1. - a[2]) + d[1] * a[2],
                ];
            }
            "reverb" => {
                let mut wet = [0.; 2];
                for (j, comb) in self.combs.iter_mut().enumerate() {
                    let p = self.positions[j];
                    let d = comb[p];
                    comb[p] = [x[0] + d[1] * 0.72, x[1] + d[0] * 0.72];
                    wet[0] += d[0] * 0.25;
                    wet[1] += d[1] * 0.25;
                    self.positions[j] = (p + 1) % comb.len();
                }
                x = [
                    x[0] * (1. - a[0]) + wet[0] * a[0],
                    x[1] * (1. - a[0]) + wet[1] * a[0],
                ];
            }
            _ => {}
        }
        x
    }
}
pub fn effects(effects: &[Effect], rate: u32) -> Vec<Processor> {
    effects.iter().map(|e| Processor::new(e, rate)).collect()
}
pub fn apply(x: &mut [f32; 2], effects: &mut [Processor]) {
    for effect in effects {
        *x = effect.process(*x);
    }
}

pub fn effects_bypassed(specs: &[Effect], bypassed: &[usize], rate: u32) -> Vec<Processor> {
    let mut prepared = effects(specs, rate);
    for &index in bypassed {
        if let Some(effect) = prepared.get_mut(index.saturating_sub(1)) {
            effect.bypassed = true;
        }
    }
    prepared
}

/// RBJ biquads with frequency clamped below Nyquist. Double precision state.
fn biquad(kind: &str, rate: f64, hz: f64, gain: f64, q: f64) -> [f64; 5] {
    let w = std::f64::consts::TAU * hz.clamp(1., rate * 0.49) / rate;
    let c = w.cos();
    let alpha = w.sin() / (2. * q);
    let a = 10f64.powf(gain / 40.);
    let beta = 2. * a.sqrt() * alpha;
    let (b0, b1, b2, a0, a1, a2) = match kind {
        "highpass" => (
            (1. + c) / 2.,
            -(1. + c),
            (1. + c) / 2.,
            1. + alpha,
            -2. * c,
            1. - alpha,
        ),
        "lowpass" => (
            (1. - c) / 2.,
            1. - c,
            (1. - c) / 2.,
            1. + alpha,
            -2. * c,
            1. - alpha,
        ),
        "bell" => (
            1. + alpha * a,
            -2. * c,
            1. - alpha * a,
            1. + alpha / a,
            -2. * c,
            1. - alpha / a,
        ),
        "lowshelf" => (
            a * ((a + 1.) - (a - 1.) * c + beta),
            2. * a * ((a - 1.) - (a + 1.) * c),
            a * ((a + 1.) - (a - 1.) * c - beta),
            (a + 1.) + (a - 1.) * c + beta,
            -2. * ((a - 1.) + (a + 1.) * c),
            (a + 1.) + (a - 1.) * c - beta,
        ),
        _ => (
            a * ((a + 1.) + (a - 1.) * c + beta),
            -2. * a * ((a - 1.) + (a + 1.) * c),
            a * ((a + 1.) + (a - 1.) * c - beta),
            (a + 1.) - (a - 1.) * c + beta,
            2. * ((a - 1.) - (a + 1.) * c),
            (a + 1.) - (a - 1.) * c - beta,
        ),
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

pub fn estimated_state_bytes(spec: &Effect, rate: u32) -> usize {
    match spec.kind.as_str() {
        "delay" => (spec.args[0] as f64 * rate as f64).ceil() as usize * 8,
        "reverb" => [0.0297, 0.0371, 0.0411, 0.0437]
            .iter()
            .map(|t| (t * rate as f64) as usize * 8)
            .sum(),
        _ => 0,
    }
}

/// Finite native percussion support ending at its slowest envelope's -80 dB point.
/// Used only by opt-in sound constructors; legacy pattern gating is unchanged.
pub fn drum_duration(name: &str) -> f64 {
    let rate = match name {
        "kick" => 15.,
        "snare" => 24.,
        "hat" => 90.,
        "open-hat" => 6.,
        "clap" => 22.,
        "rimshot" => 75.,
        "high-tom" | "mid-tom" | "low-tom" => 10.,
        "crash" => 3.2,
        "ride" => 4.,
        "tambourine" => 18.,
        "shaker" => 35.,
        "cowbell" => 14.,
        _ => return 0.,
    };
    4. * std::f64::consts::LN_10 / rate
}

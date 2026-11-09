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
        "clap" => noise(index, 971) * (-t as f32 * 22.).exp() * 0.4,
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
pub struct Processor {
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
    pub fn process(&mut self, mut x: [f32; 2]) -> [f32; 2] {
        let a = &self.spec.args;
        match self.spec.kind.as_str() {
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

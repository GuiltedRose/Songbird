//! Bounded, offline analysis. Suggestions are measurements, never phoneme recognition.
use crate::codec::Asset;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub duration: f64,
    pub peak: f32,
    pub rms_db: f32,
    pub clipped_frames: u64,
    pub onset: Option<f64>,
    pub sustain: Option<f64>,
    pub release: Option<f64>,
    pub end: Option<f64>,
    pub root: Option<f64>,
    pub confidence: f64,
    pub voiced: Option<bool>,
    pub warnings: Vec<String>,
}
/// YIN cumulative-mean normalized difference, followed by parabolic lag refinement.
fn pitch(x: &[f32], rate: f64) -> Option<(f64, f64)> {
    let max = (rate / 40.).ceil() as usize;
    let min = (rate / 2000.).floor().max(2.) as usize;
    if x.len() <= max * 2 {
        return None;
    }
    let n = x.len() - max;
    let mut difference = vec![0.; max + 1];
    let mut sum = 0.;
    for lag in 1..=max {
        let mut d = 0.;
        for i in 0..n {
            let v = x[i] as f64 - x[i + lag] as f64;
            d += v * v;
        }
        sum += d;
        difference[lag] = if sum > 1e-12 {
            d * lag as f64 / sum
        } else {
            1.
        };
    }
    let mut candidate = None;
    let mut lag = min;
    while lag < max {
        if difference[lag] < 0.15 {
            while lag + 1 < max && difference[lag + 1] < difference[lag] {
                lag += 1;
            }
            candidate = Some(lag);
            break;
        }
        lag += 1;
    }
    let lag = candidate?;
    let a = difference[lag - 1];
    let b = difference[lag];
    let c = difference[lag + 1];
    let denom = a - 2. * b + c;
    let offset = if denom.abs() > 1e-12 {
        (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
    } else {
        0.
    };
    let hz = rate / (lag as f64 + offset);
    ((40.0..=2000.).contains(&hz)).then_some((hz, (1. - b).clamp(0., 1.)))
}
pub fn analyze(asset: &mut Asset) -> Result<Analysis> {
    ensure!(
        asset.duration() <= 30.,
        "voicebank samples must be at most 30 seconds; trim a copy before importing longer material"
    );
    let rate = asset.rate.min(12000);
    let mut mono = Vec::with_capacity((asset.duration() * rate as f64) as usize + 1);
    let mut sum = 0f64;
    let mut count = 0;
    let mut bucket = 0;
    let mut clipped = 0;
    for i in 0..asset.frames {
        let frame = asset.frame(i as i64)?;
        if frame.iter().any(|v| v.abs() >= 0.999) {
            clipped += 1;
        }
        sum += (frame[0] as f64 + frame[1] as f64) * 0.5;
        count += 1;
        let next = (i + 1) * rate as u64 / asset.rate as u64;
        if next > bucket {
            mono.push((sum / count as f64) as f32);
            bucket = next;
            sum = 0.;
            count = 0;
        }
    }
    if count > 0 {
        mono.push((sum / count as f64) as f32);
    }
    let hop = (rate / 100) as usize;
    let rms = mono
        .chunks(hop)
        .map(|c| (c.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt())
        .collect::<Vec<_>>();
    let maximum = rms.iter().copied().fold(0f64, f64::max);
    let threshold = (maximum * 0.06).max(0.001);
    let first = rms.iter().position(|v| *v >= threshold);
    let last = rms.iter().rposition(|v| *v >= threshold);
    let onset = first.map(|i| (i as f64 * 0.01 - 0.01).max(0.));
    let end = last.map(|i| ((i + 2) as f64 * 0.01).min(asset.duration()));
    let active = rms
        .iter()
        .filter(|v| **v >= threshold)
        .copied()
        .collect::<Vec<_>>();
    let rms_db = if active.is_empty() {
        -120.
    } else {
        (20. * (active.iter().map(|v| v * v).sum::<f64>() / active.len() as f64)
            .sqrt()
            .max(1e-6)
            .log10()) as f32
    };
    let mut pitches = vec![];
    let size = 1024usize;
    let from = (onset.unwrap_or(0.) * rate as f64) as usize;
    let to = (end.unwrap_or(0.) * rate as f64) as usize;
    if to.saturating_sub(from) >= size {
        let windows = (to - from - size) / 256 + 1;
        let stride = windows.div_ceil(64).max(1);
        for w in (0..windows).step_by(stride) {
            let at = from + w * 256;
            let x = &mono[at..at + size];
            let energy = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / size as f64;
            if energy.sqrt() >= threshold
                && let Some((hz, confidence)) = pitch(x, rate as f64)
            {
                pitches.push((at as f64 / rate as f64, hz, confidence));
            }
        }
    }
    let mut frequencies = pitches.iter().map(|p| p.1).collect::<Vec<_>>();
    frequencies.sort_by(f64::total_cmp);
    let median = frequencies.get(frequencies.len() / 2).copied();
    let stable = pitches
        .iter()
        .filter(|p| median.is_some_and(|m| (1200. * (p.1 / m).log2()).abs() < 60.))
        .collect::<Vec<_>>();
    let confidence = if pitches.is_empty() {
        0.
    } else {
        stable.iter().map(|p| p.2).sum::<f64>() / pitches.len() as f64
    };
    let root = if stable.len() >= 2 && confidence >= 0.75 {
        median
    } else {
        None
    };
    // Use only the longest contiguous stable region, not two separated vowels.
    let max_gap = if to > from {
        (to - from) as f64 / rate as f64 / 64. + 0.05
    } else {
        0.05
    };
    let mut best = (0usize, 0usize);
    let mut run = 0;
    for i in 0..stable.len() {
        if i > 0 && stable[i].0 - stable[i - 1].0 > max_gap {
            run = i;
        }
        if i + 1 - run > best.1 - best.0 {
            best = (run, i + 1);
        }
    }
    let (sustain, release) = if let Some(hz) = root.filter(|_| best.1 > best.0) {
        let first = stable[best.0].0;
        let finish =
            (stable[best.1 - 1].0 + size as f64 / rate as f64).min(end.unwrap_or(asset.duration()));
        let start = (first * asset.rate as f64) as i64;
        let period = (asset.rate as f64 / hz).ceil() as i64;
        let mut epoch = start;
        let mut peak = -f32::INFINITY;
        for i in start..(start + period).min(asset.frames as i64) {
            let v = asset.frame(i)?[0];
            if v > peak {
                peak = v;
                epoch = i;
            }
        }
        let start = (epoch as f64 / asset.rate as f64).max(onset.unwrap_or(0.));
        if finish - start >= 3. / hz {
            (Some(start), Some(finish))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let mut warnings = vec![];
    if clipped > 0 {
        warnings.push(format!(
            "{clipped} frames reach clipping level; consider a lower-input-level take"
        ));
    }
    if maximum < 0.01 || rms_db < -40. {
        warnings.push(
            "Very quiet recording; increase hardware input gain and record another take".into(),
        );
    }
    if onset.is_none() {
        warnings.push("No reliable audible region detected".into());
    }
    if root.is_none() {
        warnings.push("No stable root pitch: unvoiced, short, changing pitch, or uncertain; correct metadata manually".into());
    }
    Ok(Analysis {
        duration: asset.duration(),
        peak: asset.peak,
        rms_db,
        clipped_frames: clipped,
        onset,
        sustain,
        release,
        end,
        root,
        confidence,
        voiced: if root.is_some() {
            Some(true)
        } else if maximum >= 0.01 && pitches.is_empty() {
            Some(false)
        } else {
            None
        },
        warnings,
    })
}

//! Deterministic, note-aged additive instrument voices. No external assets.
use std::f64::consts::TAU;

/// Short spellings for compound pitched names; full names remain canonical.
pub fn alias(name: &str) -> &str {
    match name {
        "ag" => "acoustic-guitar",
        "ng" => "nylon-guitar",
        "eg" => "electric-guitar",
        "ep" => "electric-piano",
        "eb" => "electric-bass",
        "ub" => "upright-bass",
        "sl" => "soft-lead",
        _ => name,
    }
}

pub const NAMES: &[&str] = &[
    "piano",
    "electric-piano",
    "organ",
    "accordion",
    "acoustic-guitar",
    "nylon-guitar",
    "electric-guitar",
    "banjo",
    "mandolin",
    "upright-bass",
    "electric-bass",
    "violin",
    "cello",
    "strings",
    "flute",
    "clarinet",
    "saxophone",
    "trumpet",
    "marimba",
    "vibraphone",
];

// Relative partial frequency, amplitude, and decay multiplier. Nonharmonic
// modes are used only for struck instruments. Phase is integrated without
// wrapping so inharmonic partials remain continuous during pitch bends.
const PIANO: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2.002, 0.5, 1.4),
    (3.008, 0.28, 2.),
    (4.02, 0.16, 2.8),
    (5.04, 0.09, 3.5),
    (6.07, 0.05, 4.5),
];
const TINES: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2., 0.15, 1.5),
    (3., 0.08, 2.),
    (7., 0.22, 5.),
];
const PLUCK: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2., 0.48, 1.8),
    (3., 0.32, 2.6),
    (4., 0.2, 3.4),
    (5., 0.14, 4.2),
    (6., 0.09, 5.),
    (7., 0.06, 5.8),
    (8., 0.04, 6.6),
];
const BOW: &[(f64, f64, f64)] = &[
    (1., 0.7, 1.),
    (2., 0.65, 1.),
    (3., 0.48, 1.),
    (4., 0.35, 1.),
    (5., 0.28, 1.),
    (6., 0.18, 1.),
    (7., 0.12, 1.),
    (8., 0.09, 1.),
];
const REED: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2., 0.3, 1.),
    (3., 0.65, 1.),
    (4., 0.12, 1.),
    (5., 0.32, 1.),
    (6., 0.08, 1.),
    (7., 0.18, 1.),
    (9., 0.08, 1.),
];
const BRASS: &[(f64, f64, f64)] = &[
    (1., 0.6, 1.),
    (2., 1., 1.),
    (3., 0.8, 1.),
    (4., 0.6, 1.),
    (5., 0.45, 1.),
    (6., 0.3, 1.),
    (7., 0.2, 1.),
    (8., 0.12, 1.),
];
const FLUTE: &[(f64, f64, f64)] = &[(1., 1., 1.), (2., 0.14, 1.), (3., 0.06, 1.), (4., 0.02, 1.)];
const ORGAN: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2., 0.6, 1.),
    (3., 0.4, 1.),
    (4., 0.25, 1.),
    (6., 0.15, 1.),
    (8., 0.1, 1.),
];
const MARIMBA: &[(f64, f64, f64)] = &[(1., 1., 1.), (4., 0.4, 3.), (10., 0.15, 8.)];
const VIBES: &[(f64, f64, f64)] = &[
    (1., 1., 1.),
    (2.76, 0.28, 1.8),
    (5.4, 0.12, 3.),
    (8.93, 0.05, 5.),
];

/// Phase follows the renderer's pitch/bend integration. Time is note-local,
/// keeping plucked attacks and decay consistent across render block sizes.
pub fn sample(kind: &str, phase: f64, frequency: f64, rate: u32, time: f64) -> f32 {
    if !frequency.is_finite() || frequency <= 0. || rate == 0 || time < 0. {
        return 0.;
    }
    let (partials, attack, decay, brightness, body): (&[(f64, f64, f64)], f64, f64, f64, f64) =
        match kind {
            "piano" => (PIANO, 0.003, 1.5, 0.1, 0.),
            "electric-piano" => (TINES, 0.004, 1.1, 0., 0.),
            "organ" => (ORGAN, 0.006, 0., 0., 0.),
            "accordion" => (REED, 0.035, 0., 0.1, 0.),
            "acoustic-guitar" => (PLUCK, 0.002, 2.1, 0.2, 0.25),
            "nylon-guitar" => (PLUCK, 0.004, 2.5, 0.65, 0.1),
            "electric-guitar" => (PLUCK, 0.003, 1.2, 0.05, 0.4),
            "banjo" => (PLUCK, 0.001, 5., -0.15, 0.7),
            "mandolin" => (PLUCK, 0.002, 3.8, 0.05, 0.55),
            "upright-bass" => (PLUCK, 0.008, 1.8, 0.9, 0.2),
            "electric-bass" => (PLUCK, 0.004, 1., 0.55, 0.4),
            "violin" => (BOW, 0.07, 0., 0.05, 0.),
            "cello" => (BOW, 0.1, 0., 0.5, 0.),
            "strings" => (BOW, 0.18, 0., 0.3, 0.),
            "flute" => (FLUTE, 0.045, 0., 0., 0.),
            "clarinet" => (REED, 0.035, 0., 0.45, 0.),
            "saxophone" => (REED, 0.025, 0., -0.1, 0.),
            "trumpet" => (BRASS, 0.025, 0., 0., 0.),
            "marimba" => (MARIMBA, 0.002, 4., 0., 0.),
            "vibraphone" => (VIBES, 0.003, 0.8, 0., 0.),
            _ => return 0.,
        };
    let mut sum = 0.;
    let mut weight = 0.;
    for &(ratio, amplitude, damping) in partials {
        // Fade partials before Nyquist rather than folding them into the band.
        let cutoff = ((0.49 - frequency * ratio / rate as f64) / 0.04).clamp(0., 1.);
        let level = amplitude / ratio.powf(brightness) * cutoff;
        let age = (-time * decay * damping).exp();
        let angle = phase * ratio;
        let resonance = 1. + body * (ratio * 1.7).cos();
        sum += (angle * TAU).sin() * level * age * resonance;
        // Fixed normalization retains natural decay and high-note attenuation.
        weight += amplitude / ratio.powf(brightness) * (1. + body);
    }
    let tremolo = if kind == "vibraphone" {
        0.85 + 0.15 * (TAU * 5.2 * time).cos()
    } else {
        1.
    };
    // Smooth onset multiplies the existing user-controlled track ADSR.
    (sum / weight.max(1.) * (1. - (-time / attack).exp()) * tremolo) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_voices_are_distinct_deterministic_and_bounded() {
        let mut fingerprints = Vec::new();
        for &name in NAMES {
            let samples: Vec<_> = (0..4096)
                .map(|i| {
                    let time = i as f64 / 16000.;
                    sample(name, 220. * time, 220., 16000, time)
                })
                .collect();
            assert!(samples.iter().all(|x| x.is_finite() && x.abs() <= 1.));
            assert!(samples.iter().map(|x| x * x).sum::<f32>() > 0.1, "{name}");
            let fingerprint: Vec<_> = samples.iter().step_by(31).map(|x| x.to_bits()).collect();
            assert!(
                !fingerprints.contains(&fingerprint),
                "duplicate voice {name}"
            );
            fingerprints.push(fingerprint);
            assert_eq!(
                samples[100],
                sample(name, 220. * 100. / 16000., 220., 16000, 100. / 16000.)
            );
            assert_eq!(sample(name, 0., 220., 16000, 0.), 0.);
            assert_eq!(sample(name, 1., 9000., 16000, 0.1), 0.);
        }
    }

    #[test]
    fn struck_voices_decay_while_sustained_voices_hold() {
        let energy = |name, start| {
            (0..1600)
                .map(|i| {
                    let time = start + i as f64 / 16000.;
                    let x = sample(name, 220. * time, 220., 16000, time);
                    x * x
                })
                .sum::<f32>()
        };
        for name in [
            "piano",
            "electric-piano",
            "acoustic-guitar",
            "banjo",
            "marimba",
            "vibraphone",
        ] {
            assert!(energy(name, 2.) < energy(name, 0.1) * 0.2, "{name}");
        }
        for name in ["organ", "strings", "flute", "clarinet", "trumpet"] {
            assert!(energy(name, 2.) > energy(name, 0.1) * 0.5, "{name}");
        }
    }
}

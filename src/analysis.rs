//! Offline measurements of the shared renderer. No LUFS/true-peak approximation.
use crate::{engine::Renderer, language, project::Project};
use anyhow::Result;
#[derive(Default)]
pub struct Statistics {
    pub frames: u64,
    pub sample_peak: f64,
    pub clipped_frames: u64,
    left: f64,
    right: f64,
    cross: f64,
}
impl Statistics {
    pub fn push(&mut self, frame: [f32; 2]) {
        let [left, right] = frame.map(f64::from);
        self.frames += 1;
        let peak = left.abs().max(right.abs());
        self.sample_peak = self.sample_peak.max(peak);
        if peak > 1. {
            self.clipped_frames += 1;
        }
        self.left += left * left;
        self.right += right * right;
        self.cross += left * right;
    }
    pub fn rms_db(&self) -> f64 {
        20. * ((self.left + self.right) / (2. * self.frames.max(1) as f64))
            .sqrt()
            .max(1e-12)
            .log10()
    }
    pub fn correlation(&self) -> Option<f64> {
        let denominator = (self.left * self.right).sqrt();
        (denominator > 1e-20).then(|| (self.cross / denominator).clamp(-1., 1.))
    }
}
pub fn project(project: &Project) -> Result<String> {
    let mut renderer = Renderer::prepare(&project.root, language::parse(&project.source)?)?;
    let mut statistics = Statistics::default();
    let mut peaks = vec![0f32; renderer.tracks.len()];
    let mut buffer = [[0.; 2]; 1024];
    loop {
        let count = renderer.block(&mut buffer)?;
        if count == 0 {
            break;
        }
        for &frame in &buffer[..count] {
            statistics.push(frame);
        }
        for (peak, block_peak) in peaks.iter_mut().zip(&renderer.channel_peaks) {
            *peak = peak.max(*block_peak);
        }
    }
    let mut report = format!(
        "Offline pre-device-clamp master: {} frames; sample peak {:.2} dBFS; RMS {:.2} dBFS; clipped frames {}; stereo correlation {}. LUFS/LRA/true peak are not implemented.",
        statistics.frames,
        20. * statistics.sample_peak.max(1e-12).log10(),
        statistics.rms_db(),
        statistics.clipped_frames,
        statistics
            .correlation()
            .map_or("undefined (silence/empty channel)".into(), |v| format!(
                "{v:.3}"
            ))
    );
    for (i, track) in renderer.tracks.iter().enumerate() {
        report.push_str(&format!(
            "\nTrack {}: sample peak {:.2} dBFS; clipped frames {}; maximum insert gain reduction {:.2} dB",
            track.spec.name,
            20. * peaks[i].max(1e-12).log10(),
            renderer.channel_clipped[i], renderer.channel_reduction[i]
        ));
    }
    if let Some(mixer) = &renderer.mixer {
        for (name, meter) in mixer.names.iter().zip(&mixer.meters) {
            report.push_str(&format!("\nMixer {name}: sample peak {:.2} dBFS; clipped frames {}; maximum insert gain reduction {:.2} dB", 20. * meter.peak.max(1e-12).log10(), meter.clipped_frames, meter.reduction_db));
        }
    }
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correlation_peak_and_rms_are_measured_without_loudness_substitution() {
        let mut same = Statistics::default();
        let mut inverted = Statistics::default();
        for _ in 0..100 {
            same.push([0.5, 0.5]);
            inverted.push([0.5, -0.5]);
        }
        assert_eq!(same.correlation(), Some(1.));
        assert_eq!(inverted.correlation(), Some(-1.));
        assert!((same.rms_db() + 6.0206).abs() < 0.001);
        same.push([1.2, 0.]);
        assert_eq!(same.clipped_frames, 1);
        assert!(Statistics::default().correlation().is_none());
    }
}

//! Original deterministic synthetic sample banks. Never labeled as human recordings.
use crate::{
    codec,
    phonetics::{self, ContextProfile, Selection},
    voice::{Bank, Phoneme},
};
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, path::Path};
pub const MODEL: &str = "songbird-procedural-v1";
pub fn generate(root: &Path, path: &str, name: &str, selection: &Selection) -> Result<String> {
    let context = ContextProfile::load(root, selection)?;
    ensure!(
        !context.inventory.phones.is_empty(),
        "inventory has no phones"
    );
    for p in &context.inventory.phones {
        ensure!(
            p.manner != "auto",
            "phone {} needs an explicit procedural manner; language phone-shape PROFILE PHONE MANNER CUTOFF NASAL",
            p.id
        );
        ensure!(
            !p.vowel || p.formants.is_some(),
            "procedural vowel {} needs three formants; language phone-formants PROFILE PHONE F1 F2 F3",
            p.id
        );
    }
    let relative = Path::new(path);
    ensure!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "bank destination must be project-relative"
    );
    let dir = root.join(path);
    ensure!(
        !dir.exists(),
        "bank destination already exists; existing audio will not be replaced"
    );
    codec::directory(
        root,
        &codec::script_path(relative.parent().unwrap_or(Path::new(""))),
    )?;
    std::fs::create_dir(&dir)?;
    codec::directory(&dir, "samples")?;
    let mut samples = vec![];
    let rate = 16000u32;
    let root_hz = 220.;
    for (index, phone) in context.inventory.phones.iter().enumerate() {
        let file = format!("samples/phone-{index:03}.flac");
        let duration = if phone.id.contains('ː') { 0.9 } else { 0.65 };
        let mut signal = vec![[0.; 2]; (duration * rate as f64) as usize];
        let formants = phone.formants.unwrap_or([260., 1100., 2200.]);
        let mut peak = 0f32;
        let mut low = 0.;
        for (i, out) in signal.iter_mut().enumerate() {
            let t = i as f64 / rate as f64;
            let local = t - 0.03;
            if local < 0. {
                continue;
            }
            let end = duration - 0.03;
            let edge = (local / 0.015).min((end - t) / 0.03).clamp(0., 1.) as f32;
            let mut voiced = 0.;
            if phone.voiced || phone.vowel || phone.syllabic {
                for h in 1..=32 {
                    let f = root_hz * h as f64;
                    let mut envelope = 0.;
                    for (j, center) in formants.iter().enumerate() {
                        let glide = (local / (duration * 0.7)).clamp(0., 1.) as f32;
                        let center = phone
                            .formants_end
                            .map_or(*center, |end| *center + (end[j] - *center) * glide);

                        let bandwidth = 90. + j as f64 * 55.;
                        envelope += (-0.5 * ((f - center as f64) / bandwidth).powi(2)).exp()
                            / (1. + j as f64 * 0.6);
                    }
                    voiced +=
                        envelope / (h as f64).sqrt() * (std::f64::consts::TAU * f * local).sin();
                }
            }
            let noise = crate::dsp::noise(i as u64, index as u64 + 947);
            let alpha = 1. - (-std::f32::consts::TAU * phone.noise_cutoff / rate as f32).exp();
            low += alpha * (noise - low);
            let friction = noise - low;
            let value = if phone.stop || phone.manner == "affricate" {
                let burst = (-((local - 0.025) / 0.009).powi(2)).exp() as f32;
                friction
                    * (if phone.manner == "affricate" {
                        burst + 0.35
                    } else {
                        burst
                    })
                    * 0.8
                    + voiced as f32 * if phone.voiced { 0.12 } else { 0. }
            } else if phone.vowel || phone.syllabic {
                voiced as f32 * 0.35
            } else if phone.voiced {
                voiced as f32 * 0.2 + friction * 0.025
            } else {
                friction * 0.22
            };
            let modulation = if phone.manner == "trill" {
                0.3 + 0.7 * (std::f32::consts::TAU * 25. * local as f32).sin().abs()
            } else if phone.manner == "tap" {
                0.2 + 0.8 * (-((local - 0.025) / 0.012).powi(2)).exp() as f32
            } else {
                1.
            };
            let nasal = if phone.nasal {
                0.07 * (std::f64::consts::TAU * root_hz * local).sin() as f32
            } else {
                0.
            };
            let value = (value * modulation + nasal) * edge;
            *out = [value; 2];
            peak = peak.max(value.abs());
        }
        if peak > 0. {
            for frame in &mut signal {
                frame[0] *= 0.55 / peak;
                frame[1] = frame[0];
            }
        }
        let mut encoder = codec::Encoder::create(&dir.join(&file), rate, 1, 24)?;
        encoder.write(&signal)?;
        encoder.finish()?;
        samples.push(Phoneme {
            phoneme: phone.id.clone(),
            file,
            root: root_hz,
            low: 36,
            high: 84,
            onset: 0.025,
            sustain: 0.1,
            release: Some(duration - 0.08),
            end: duration - 0.025,
            voiced: phone.voiced || phone.vowel || phone.syllabic,
            style: "normal".into(),
            register: "alto".into(),
            gain: 1.,
        });
    }
    let bank = Bank {
        version: 1,
        name: name.into(),
        license: "CC0-1.0; original Songbird procedural waveforms, not human recordings".into(),
        samples,
        dictionary: BTreeMap::new(),
        language: Some(context.language.clone()),
        inventory: Some(
            selection
                .inventory
                .clone()
                .unwrap_or(context.pronunciation.language.clone()),
        ),
        provider: Some(context.provider.clone()),
        generation: Some(MODEL.into()),
    };
    phonetics::save_profile(&dir, &context.pronunciation)?;
    if context.inventory.language != context.pronunciation.language {
        phonetics::save_profile(&dir, &context.inventory)?;
    }
    crate::project::atomic(
        &dir.join("voicebank.toml"),
        toml::to_string_pretty(&bank)?.as_bytes(),
    )?;
    Ok(path.into())
}
pub fn default_bank(root: &Path, selection: &Selection) -> Result<String> {
    let context = ContextProfile::load(root, selection)?;
    let path = format!("voices/procedural-v1-{}", context.language);
    let manifest = root.join(&path).join("voicebank.toml");
    if manifest.exists() {
        let bank: Bank = phonetics::read_resource(&manifest)?;
        ensure!(
            bank.generation.as_deref() == Some(MODEL)
                && bank.language.as_deref() == Some(context.language.as_str())
                && bank.inventory.as_deref() == Some(context.inventory.language.as_str())
                && bank.provider.as_deref() == Some(context.provider.as_str()),
            "default bank path contains another bank; select an explicit bank or generate into a new path"
        );
        return Ok(path);
    }
    generate(
        root,
        &path,
        &format!("Procedural {} (synthetic)", context.language),
        selection,
    )
}

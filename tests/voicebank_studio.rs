use songbird::{
    codec::{self, Asset, Encoder},
    engine::Renderer,
    language,
    project::Project,
    studio::{self, Capture, Output, Request, Studio},
    voice_analysis,
};
use std::{f32::consts::TAU, fs, path::PathBuf};
struct Fixture {
    dir: PathBuf,
    project: Project,
}
impl Fixture {
    fn new() -> Self {
        let dir = codec::unique(&std::env::temp_dir(), "songbird-studio", "dir").unwrap();
        let project = Project::create(&dir, "project Studio;rate 16000".into()).unwrap();
        Self { dir, project }
    }
    fn command(&self, bank: Option<&str>, args: &[&str]) -> anyhow::Result<Output> {
        studio::execute(Request {
            project: self.project.clone(),
            bank: bank.map(str::to_owned),
            args: args.iter().map(|s| s.to_string()).collect(),
        })
    }
    fn audio(&self, name: &str, kind: &str, amp: f32) -> PathBuf {
        let p = self.dir.join(name);
        let mut e = Encoder::create(&p, 16000, 1, 24).unwrap();
        let data = (0..16000)
            .map(|i| {
                let t = i as f32 / 16000.;
                let x = if !(0.1..0.9).contains(&t) {
                    0.
                } else if kind == "noise" {
                    songbird::dsp::noise(i, 17) * amp
                } else {
                    ((TAU * 220. * t).sin() + 0.25 * (TAU * 440. * t).sin()) * amp
                };
                [x; 2]
            })
            .collect::<Vec<_>>();
        e.write(&data).unwrap();
        e.finish().unwrap();
        p
    }
    fn bank(&self) -> Studio {
        Studio::create(&self.dir, "voices/nova", "Nova", "english").unwrap()
    }
    fn populated(&self) -> Studio {
        let mut bank = self.bank();
        for phone in ["M", "IY", "S"] {
            let path = self.audio(
                &format!("{phone}.wav"),
                if phone == "S" { "noise" } else { "tone" },
                0.2,
            );
            let capture = Capture {
                phoneme: phone.into(),
                register: "alto".into(),
                expression: "normal".into(),
                recorded: false,
                phonetics: None,
            };
            bank.import(&path, capture).unwrap();
            let i = bank.session.selected.unwrap();
            let t = &mut bank.session.takes[i];
            t.low = 48;
            t.high = 72;
            t.onset = 0.1;
            t.sustain = 0.15;
            t.end = 0.89;
            t.release = Some(0.8);
            t.voiced = Some(phone != "S");
            t.root = Some(220.);
            bank.publish().unwrap();
        }
        bank
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn create_reopen_settings_and_target() {
    let f = Fixture::new();
    f.command(None, &["new", "voices/nova", "Nova", "english"])
        .unwrap();
    f.command(
        Some("voices/nova"),
        &["settings", "16000", "16", "2", "wav"],
    )
    .unwrap();
    f.command(Some("voices/nova"), &["target", "SH"]).unwrap();
    let bank = Studio::open(&f.dir, "voices/nova").unwrap();
    assert_eq!(bank.session.settings.channels, 2);
    assert_eq!(bank.session.target, "SH");
    assert!(
        f.command(Some("voices/nova"), &["settings", "1", "16", "2"])
            .is_err()
    );
    assert!(
        f.command(None, &["new", "../outside", "Bad", "english"])
            .is_err()
    );
    assert!(
        f.command(None, &["new", "voices/bad", "Bad", "zz-unavailable"])
            .is_err()
    );
}
#[test]
fn analysis_detects_pitch_silence_and_levels() {
    let f = Fixture::new();
    let p = f.audio("pitch.wav", "tone", 0.2);
    let mut a = Asset::decode(&p, &f.dir.join("cache")).unwrap();
    let report = voice_analysis::analyze(&mut a).unwrap();
    assert!((report.root.unwrap() - 220.).abs() < 2.);
    assert!(report.confidence > 0.8);
    assert!(report.onset.unwrap() >= 0.08 && report.onset.unwrap() <= 0.11);
    assert!(report.end.unwrap() > 0.88);
    assert!(report.sustain.is_some() && report.release.is_some());
    assert_eq!(report.voiced, Some(true));
    assert_eq!(report.clipped_frames, 0);
}
#[test]
fn clipped_quiet_and_silent_takes_are_diagnosed() {
    let f = Fixture::new();
    for (name, amp) in [
        ("clipped.wav", 2.),
        ("quiet.wav", 0.002),
        ("silent.wav", 0.),
    ] {
        let p = f.audio(name, "tone", amp);
        let mut a = Asset::decode(&p, &f.dir.join("cache")).unwrap();
        let report = voice_analysis::analyze(&mut a).unwrap();
        assert!(!report.warnings.is_empty());
        if amp > 1. {
            assert!(report.clipped_frames > 0);
        }
        if amp == 0. {
            assert!(report.onset.is_none() && report.root.is_none());
        }
    }
}
#[test]
fn unvoiced_noise_has_no_invented_pitch() {
    let f = Fixture::new();
    let p = f.audio("noise.flac", "noise", 0.2);
    let mut a = Asset::decode(&p, &f.dir.join("cache")).unwrap();
    let report = voice_analysis::analyze(&mut a).unwrap();
    assert!(report.root.is_none());
    assert_eq!(report.voiced, Some(false));
}
#[test]
fn original_takes_and_recording_metadata_are_preserved() {
    let f = Fixture::new();
    let mut bank = f.bank();
    let p = f.audio("source.flac", "tone", 0.2);
    let original = fs::read(&p).unwrap();
    let c = bank.capture();
    bank.import(&p, c.clone()).unwrap();
    bank.import(&p, c).unwrap();
    assert_eq!(bank.session.takes.len(), 2);
    let t = &bank.session.takes[0];
    assert_eq!((t.rate, t.channels, t.bits), (16000, 1, 24));
    assert_eq!(t.format, "flac");
    assert!(!t.capture.recorded);
    assert_ne!(t.file, bank.session.takes[1].file);
    let copied = fs::read(bank.dir.join(&t.file)).unwrap();
    f.command(Some("voices/nova"), &["take", "1"]).unwrap();
    f.command(Some("voices/nova"), &["trim", "0.12", "0.2", "0.8"])
        .unwrap();
    assert_eq!(fs::read(p).unwrap(), original);
    assert_eq!(fs::read(bank.dir.join(&t.file)).unwrap(), copied);
}
#[test]
fn analysis_refresh_does_not_overwrite_manual_values() {
    let f = Fixture::new();
    let mut bank = f.bank();
    let p = f.audio("source.wav", "tone", 0.2);
    bank.import(&p, bank.capture()).unwrap();
    f.command(Some("voices/nova"), &["set", "root", "250"])
        .unwrap();
    f.command(Some("voices/nova"), &["analyze"]).unwrap();
    let bank = Studio::open(&f.dir, "voices/nova").unwrap();
    assert_eq!(bank.selected().unwrap().root, Some(250.));
    f.command(Some("voices/nova"), &["suggest"]).unwrap();
    assert!(
        (Studio::open(&f.dir, "voices/nova")
            .unwrap()
            .selected()
            .unwrap()
            .root
            .unwrap()
            - 220.)
            .abs()
            < 2.
    );
}
#[test]
fn coverage_reports_phoneme_register_expression_and_pitch_gaps() {
    let f = Fixture::new();
    let bank = f.populated();
    let okay = bank
        .coverage(Some("me see"), Some("A3:1 B3:1"), "alto", "normal")
        .unwrap();
    assert!(okay.iter().any(|s| s == "Missing labels: none"));
    assert!(
        !okay
            .iter()
            .any(|s| s.starts_with("MISSING") || s.starts_with("OUT OF RANGE"))
    );
    let missing = bank.coverage(Some("take"), None, "alto", "normal").unwrap();
    assert!(missing.iter().any(|s| s.starts_with("MISSING T")));
    let register = bank
        .coverage(Some("me"), None, "soprano", "normal")
        .unwrap();
    assert!(register.iter().any(|s| s.contains("UNSUPPORTED REGISTER")));
    let expression = bank.coverage(Some("me"), None, "alto", "soft").unwrap();
    assert!(expression.iter().any(|s| s.contains("MISSING EXPRESSION")));
    let pitch = bank
        .coverage(Some("me"), Some("C6"), "alto", "normal")
        .unwrap();
    assert!(pitch.iter().any(|s| s.contains("OUT OF RANGE")));
    assert!(
        bank.coverage(Some("me see"), Some("A3"), "alto", "normal")
            .is_err()
    );
}
#[test]
fn guided_targets_and_dictionary_overrides() {
    let f = Fixture::new();
    f.populated();
    let guide = f.command(Some("voices/nova"), &["guide", "next"]).unwrap();
    assert!(
        guide
            .view
            .report
            .iter()
            .any(|s| s.contains("reference word"))
    );
    assert!(
        f.command(Some("voices/nova"), &["target", "NOTAPHONE"])
            .is_err()
    );
    f.command(Some("voices/nova"), &["dictionary", "MEE", "M IY"])
        .unwrap();
    let bank = Studio::open(&f.dir, "voices/nova").unwrap();
    assert_eq!(
        songbird::voice::phonemes("mee", &bank.meta.dictionary).unwrap(),
        vec![vec!["M", "IY"]]
    );
    assert!(
        f.command(Some("voices/nova"), &["dictionary", "BAD", "S T"])
            .is_err()
    );
}
#[test]
fn preferred_take_replaces_mapping_without_deleting_media() {
    let f = Fixture::new();
    let mut bank = f.populated();
    let old = bank
        .meta
        .samples
        .iter()
        .find(|p| p.phoneme == "IY")
        .unwrap()
        .file
        .clone();
    let p = f.audio("new.wav", "tone", 0.15);
    bank.import(
        &p,
        Capture {
            phoneme: "IY".into(),
            register: "alto".into(),
            expression: "normal".into(),
            recorded: false,
            phonetics: None,
        },
    )
    .unwrap();
    let n = bank.session.selected.unwrap();
    bank.session.takes[n].low = 48;
    bank.session.takes[n].high = 72;
    bank.publish().unwrap();
    assert_eq!(bank.meta.samples.len(), 3);
    assert!(bank.dir.join(old).exists());
    assert_eq!(bank.session.takes.len(), 4);
}
#[test]
fn manual_invalid_timing_and_bad_samples_are_rejected() {
    let f = Fixture::new();
    let mut bank = f.populated();
    let path = f.dir.join("corrupt.wav");
    fs::write(&path, b"bad").unwrap();
    assert!(bank.import(&path, bank.capture()).is_err());
    assert!(
        f.command(Some("voices/nova"), &["trim", "0.8", "0.2", "0.5"])
            .is_err()
    );
    f.command(Some("voices/nova"), &["set", "low", "C6"])
        .unwrap();
    assert!(f.command(Some("voices/nova"), &["use"]).is_err());
    let selected = Studio::open(&f.dir, "voices/nova")
        .unwrap()
        .selected()
        .unwrap()
        .file
        .clone();
    fs::remove_file(bank.dir.join(selected)).unwrap();
    assert!(f.command(Some("voices/nova"), &["replay"]).is_err());
}
fn render_all(mut r: Renderer) -> Vec<[f32; 2]> {
    let mut result = vec![];
    let mut block = [[0.; 2]; 257];
    loop {
        let n = r.block(&mut block).unwrap();
        if n == 0 {
            break;
        }
        result.extend_from_slice(&block[..n]);
    }
    result
}
#[test]
fn multi_phoneme_pcm_preview_is_deterministic() {
    let f = Fixture::new();
    f.populated();
    let preview = f
        .command(Some("voices/nova"), &["test", "me see", "A3:1 B3:1"])
        .unwrap()
        .preview
        .unwrap();
    let first = render_all(preview);
    assert!(first.iter().any(|f| f[0].abs() > 0.01));
    assert!(first.iter().all(|f| f.iter().all(|x| x.is_finite())));
    let again = render_all(
        f.command(Some("voices/nova"), &["test", "me see", "A3:1 B3:1"])
            .unwrap()
            .preview
            .unwrap(),
    );
    assert_eq!(first, again);
    assert!(
        f.command(Some("voices/nova"), &["test", "take me", "A3 B3"])
            .is_err()
    );
}
#[test]
fn portable_presets_harmonies_and_explicit_overrides() {
    let f = Fixture::new();
    f.populated();
    for args in [
        &["preset", "new", "Soft Harmony"][..],
        &["preset", "set", "Soft Harmony", "harmony", "0 7"],
        &["preset", "set", "Soft Harmony", "formant", "1.1"],
        &["preset", "effect", "Soft Harmony", "reverb", "0.15"],
    ] {
        f.command(Some("voices/nova"), args).unwrap();
    }
    assert!(
        f.command(Some("voices/nova"), &["presets"])
            .unwrap()
            .view
            .report
            .iter()
            .any(|s| s.contains("Soft Harmony"))
    );
    assert!(
        f.command(Some("voices/nova"), &["preset", "show", "Soft Harmony"])
            .unwrap()
            .view
            .report
            .iter()
            .any(|s| s.contains("harmonies"))
    );
    let src = "rate 16000;bars 1;track singer {vocal-preset \"voices/nova/preset-Soft-Harmony.toml\";lyrics \"me see\";notes \"A3:1 B3:1\";gain -12db;vibrato 0;formant 0.9}";
    let r = Renderer::prepare(&f.dir, language::parse(src).unwrap()).unwrap();
    assert_eq!(r.tracks.len(), 2);
    assert!((r.tracks[0].spec.gain - 10f32.powf(-12. / 20.)).abs() < 1e-5);
    assert_eq!(r.tracks[0].spec.vibrato, 0.);
    assert_eq!(r.tracks[0].spec.formant, 0.9);
    let audio = render_all(r);
    assert!(audio.iter().any(|f| f[0].abs() > 0.01));
    let mut p = f.project.clone();
    p.source = src.into();
    p.save().unwrap();
    let copied = p
        .save_as(&f.dir.parent().unwrap().join(format!(
            "{}-copy",
            f.dir.file_name().unwrap().to_string_lossy()
        )))
        .unwrap();
    assert!(Renderer::prepare(&copied.root, language::parse(&copied.source).unwrap()).is_ok());
    fs::remove_dir_all(copied.root).unwrap();
    assert!(
        f.command(
            Some("voices/nova"),
            &["preset", "set", "Soft Harmony", "formant", "8"]
        )
        .is_err()
    );
}
#[test]
fn export_import_bank_preserves_dictionary_presets_and_takes() {
    let f = Fixture::new();
    f.populated();
    f.command(Some("voices/nova"), &["preset", "new", "Backing"])
        .unwrap();
    let output = f.dir.join("library-bank");
    f.command(Some("voices/nova"), &["export", output.to_str().unwrap()])
        .unwrap();
    f.command(
        None,
        &["import-bank", output.to_str().unwrap(), "voices/reused"],
    )
    .unwrap();
    let restored = Studio::open(&f.dir, "voices/reused").unwrap();
    assert_eq!(restored.session.takes.len(), 3);
    assert!(restored.dir.join("preset-Backing.toml").exists());
    assert_eq!(restored.meta.samples.len(), 3);
    assert!(
        f.command(
            None,
            &["import-bank", output.to_str().unwrap(), "voices/reused"]
        )
        .is_err()
    );
}
#[test]
fn compound_syllable_samples_are_preferred_when_present() {
    let f = Fixture::new();
    let mut bank = f.bank();
    let p = f.audio("syllable.wav", "tone", 0.2);
    bank.import(
        &p,
        Capture {
            phoneme: "M.IY".into(),
            register: "alto".into(),
            expression: "normal".into(),
            recorded: false,
            phonetics: None,
        },
    )
    .unwrap();
    bank.publish().unwrap();
    let report = bank
        .coverage(Some("me"), Some("A3"), "alto", "normal")
        .unwrap();
    assert!(report.iter().any(|s| s == "Using syllable sample M.IY"));
    let r = f
        .command(Some("voices/nova"), &["test", "me", "A3:1"])
        .unwrap()
        .preview
        .unwrap();
    assert_eq!(r.tracks[0].units.len(), 1);
}
#[test]
fn corrupt_manifests_fail_without_touching_original_audio() {
    let f = Fixture::new();
    let bank = f.populated();
    let original = bank.dir.join(&bank.meta.samples[0].file);
    let before = fs::read(&original).unwrap();
    fs::write(bank.dir.join("studio.toml"), "version='bad'").unwrap();
    assert!(Studio::open(&f.dir, "voices/nova").is_err());
    assert_eq!(fs::read(original).unwrap(), before);
}
#[test]
fn existing_banks_can_edit_published_metadata_in_the_studio() {
    let f = Fixture::new();
    f.populated();
    f.command(Some("voices/nova"), &["sample", "2"]).unwrap();
    f.command(Some("voices/nova"), &["set", "root", "230"])
        .unwrap();
    f.command(Some("voices/nova"), &["set", "high", "D5"])
        .unwrap();
    f.command(Some("voices/nova"), &["use"]).unwrap();
    let b = Studio::open(&f.dir, "voices/nova").unwrap();
    assert_eq!(b.meta.samples.len(), 3);
    assert_eq!(b.meta.samples[1].root, 230.);
    assert_eq!(b.meta.samples[1].high, 74);
}
#[test]
fn capture_receipts_preserve_target_without_claiming_completion() {
    let f = Fixture::new();
    let b = f.bank();
    let source = f.audio("receipt.wav", "tone", 0.2);
    fs::create_dir_all(b.dir.join("recordings")).unwrap();
    let raw = b.dir.join("recordings/capture.wav");
    fs::copy(source, &raw).unwrap();
    studio::recording_started(&f.dir, "voices/nova", &raw, &b.capture()).unwrap();
    let files = fs::read_dir(b.dir.join("captures"))
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let text = fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
    assert!(
        text.contains("capture-started")
            && text.contains("AA")
            && text.contains("recordings/capture.wav")
    );
}
#[test]
fn bundled_multi_label_example_mixes_with_instruments() {
    let f = Fixture::new();
    let source =
        Project::open(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/voicebank-studio"))
            .unwrap();
    let p = source.save_as(&f.dir.join("demo")).unwrap();
    let r = Renderer::prepare(&p.root, language::parse(&p.source).unwrap()).unwrap();
    assert_eq!(r.tracks.len(), 4);
    let audio = render_all(r);
    assert!(audio.iter().any(|f| f[0].abs() > 0.01));
    assert!(audio.iter().all(|f| f.iter().all(|v| v.is_finite())));
}
#[test]
fn unsafe_harmony_values_are_rejected() {
    let f = Fixture::new();
    f.populated();
    f.command(Some("voices/nova"), &["preset", "new", "Test"])
        .unwrap();
    assert!(
        f.command(
            Some("voices/nova"),
            &["preset", "set", "Test", "harmony", "-2147483648"]
        )
        .is_err()
    );
}
#[test]
fn coverage_inspects_the_selected_preset() {
    let f = Fixture::new();
    f.populated();
    f.command(Some("voices/nova"), &["preset", "new", "Soft"])
        .unwrap();
    f.command(
        Some("voices/nova"),
        &["preset", "set", "Soft", "expression", "soft"],
    )
    .unwrap();
    let out = f
        .command(
            Some("voices/nova"),
            &["coverage", "me see", "A3 B3", "Soft"],
        )
        .unwrap();
    assert!(
        out.view
            .report
            .iter()
            .any(|s| s.contains("MISSING EXPRESSION soft"))
    );
}
#[test]
fn lyric_guidance_and_pronunciation_ambiguities_are_explicit() {
    let f = Fixture::new();
    f.populated();
    let out = f
        .command(Some("voices/nova"), &["guide", "take me"])
        .unwrap();
    assert_eq!(out.view.session.target, "T");
    let b = Studio::open(&f.dir, "voices/nova").unwrap();
    assert!(
        b.coverage(Some("read"), None, "alto", "normal")
            .unwrap()
            .iter()
            .any(|s| s.contains("AMBIGUOUS pronunciation"))
    );
}
#[test]
fn legacy_banks_open_and_edit_without_a_studio_manifest() {
    let f = Fixture::new();
    let b = f.populated();
    fs::remove_file(b.dir.join("studio.toml")).unwrap();
    let out = f.command(None, &["open", "voices/nova"]).unwrap();
    assert_eq!(out.view.session.takes.len(), 0);
    f.command(Some("voices/nova"), &["sample", "2"]).unwrap();
    f.command(Some("voices/nova"), &["set", "note", "B3"])
        .unwrap();
    f.command(Some("voices/nova"), &["use"]).unwrap();
    assert!(
        (Studio::open(&f.dir, "voices/nova").unwrap().meta.samples[1].root
            - songbird::dsp::hz(59.))
        .abs()
            < 1e-6
    );
}

#[test]
fn extreme_note_inputs_return_errors_without_panicking() {
    assert!(language::note("C2147483647").is_err());
    assert!(language::note("C-2147483648").is_err());
    assert!(language::parse("track t {transpose -2147483648;notes A3}").is_err());
    assert!(language::parse("track t {harmony \"-2147483648\";notes A3}").is_err());
}
#[test]
fn explicit_normal_expression_overrides_a_soft_preset() {
    let f = Fixture::new();
    f.populated();
    f.command(Some("voices/nova"), &["preset", "new", "Soft"])
        .unwrap();
    f.command(
        Some("voices/nova"),
        &["preset", "set", "Soft", "expression", "soft"],
    )
    .unwrap();
    let src = "rate 16000;bars 1;track voice {vocal-preset \"voices/nova/preset-Soft.toml\";expression normal;lyrics me;notes A3}";
    assert!(Renderer::prepare(&f.dir, language::parse(src).unwrap()).is_ok());
}
#[test]
fn syllable_only_bank_can_extend_its_vowel_into_a_melisma() {
    let f = Fixture::new();
    let mut b = f.bank();
    let p = f.audio("syllable.wav", "tone", 0.2);
    b.import(
        &p,
        Capture {
            phoneme: "M.IY".into(),
            register: "alto".into(),
            expression: "normal".into(),
            recorded: false,
            phonetics: None,
        },
    )
    .unwrap();
    b.publish().unwrap();
    let report = b
        .coverage(Some("me ~"), Some("A3 B3"), "alto", "normal")
        .unwrap();
    assert!(report.iter().any(|s| s.contains("Melisma uses")));
    let r = f
        .command(Some("voices/nova"), &["test", "me ~", "A3:1 B3:1"])
        .unwrap()
        .preview
        .unwrap();
    assert_eq!(r.tracks[0].units.len(), 2);
    assert!(r.tracks[0].units[1].sustain_only);
}

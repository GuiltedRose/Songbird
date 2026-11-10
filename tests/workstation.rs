use songbird::{
    codec::{self, Asset, Encoder},
    dsp,
    engine::Renderer,
    language::{self, Track, Voice},
    project::{History, Project},
    voice::{self, Bank, LoadedBank, Phoneme},
};
use std::{collections::BTreeMap, fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = codec::unique(&std::env::temp_dir(), "songbird-test", "dir").unwrap();
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn audio(&self, name: &str, channels: u16, bits: u16) -> PathBuf {
        let p = self.0.join(name);
        let mut e = Encoder::create(&p, 16000, channels, bits).unwrap();
        let data = (0..8000)
            .map(|i| {
                let x = (i as f32 * std::f32::consts::TAU * 220. / 16000.).sin() * 0.25;
                [x, x * 0.5]
            })
            .collect::<Vec<_>>();
        e.write(&data).unwrap();
        e.finish().unwrap();
        p
    }
    fn bank(&self) {
        fs::create_dir_all(self.0.join("voices/test")).unwrap();
        self.audio("voices/test/aa.wav", 1, 24);
        let b = Bank {
            version: 1,
            name: "Analytic vowel test; not a human voice".into(),
            license: "CC0-1.0".into(),
            samples: vec![Phoneme {
                phoneme: "AA".into(),
                file: "aa.wav".into(),
                root: 220.,
                low: 40,
                high: 90,
                onset: 0.,
                sustain: 0.03,
                release: None,
                end: 0.45,
                voiced: true,
                style: "normal".into(),
                register: "alto".into(),
                gain: 1.,
            }],
            dictionary: BTreeMap::from([("AH".into(), "AA".into())]),
            language: None,
            inventory: None,
            provider: None,
            generation: None,
        };
        fs::write(
            self.0.join("voices/test/voicebank.toml"),
            toml::to_string(&b).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn rendered(src: &str) -> Vec<[f32; 2]> {
    let f = Fixture::new();
    let score = language::parse(src).unwrap();
    let mut r = Renderer::prepare(&f.0, score).unwrap();
    let mut all = vec![];
    let mut b = [[0.; 2]; 127];
    loop {
        let n = r.block(&mut b).unwrap();
        if n == 0 {
            break;
        }
        all.extend_from_slice(&b[..n]);
    }
    all
}
#[test]
fn notes() {
    assert_eq!(language::note("C4").unwrap(), 60);
    assert_eq!(language::note("Bb2").unwrap(), 46);
    assert_eq!(language::note("F#3").unwrap(), 54);
    for bad in ["H4", "C", "C99", "C♯4"] {
        assert!(language::note(bad).is_err());
    }
}
#[test]
fn patterns() {
    let e = language::events("(C4 - [E4,G4]:1)*2", 0.5).unwrap();
    assert_eq!(e.len(), 6);
    assert_eq!(e[3].beat, 2.);
    assert_eq!(e[2].notes, vec![64, 67]);
    for bad in ["(C4", "C4)", "[C4,E4", "C4*9999", "C4:NaN"] {
        assert!(language::events(bad, 0.5).is_err());
    }
}
#[test]
fn parse_failures() {
    for bad in [
        "track a { notes \"C4\"",
        "tempo NaN",
        "signature 0/4",
        "master",
        "track a { delay 0 1 2 }",
        "voice v { bank \"abc\"",
        "project \"oops",
    ] {
        assert!(language::parse(bad).is_err(), "{bad}");
    }
}
#[test]
fn definitions_sections_and_timing() {
    let s=language::parse("tempo 90; signature 3/8; bars 4;let riff \"C4 E4\"; instrument soft { synth triangle; gain -3db };section verse at 2b { track lead { instrument soft; notes $riff; loop } }").unwrap();
    assert_eq!(s.bar_beats(), 1.5);
    assert_eq!(s.duration(), 4.);
    assert_eq!(s.tracks[0].instrument, "triangle");
    assert_eq!(s.tracks[0].at, 4. / 3.);
    assert_eq!(s.tracks[0].name, "verse/lead");
}
#[test]
fn timestamps() {
    assert_eq!(language::seconds("00:08.250", 120.).unwrap(), 8.25);
    assert_eq!(language::seconds("250ms", 120.).unwrap(), 0.25);
    assert_eq!(language::seconds("2b", 120.).unwrap(), 1.);
    assert!(language::seconds("-1", 120.).is_err());
}
#[test]
fn codecs_roundtrip() {
    let f = Fixture::new();
    for channels in [1, 2] {
        for bits in [16, 24] {
            for ext in ["wav", "flac"] {
                let p = f.audio(&format!("audio-{channels}-{bits}.{ext}"), channels, bits);
                let mut a = Asset::decode(&p, &f.0.join("cache")).unwrap();
                assert_eq!(a.frames, 8000);
                assert_eq!(a.channels, channels);
                let frame = a.frame(123).unwrap();
                let expected = (123f32 * std::f32::consts::TAU * 220. / 16000.).sin() * 0.25;
                assert!((frame[0] - expected).abs() < 0.00004);
                assert!(
                    (frame[1] - expected * if channels == 1 { 1. } else { 0.5 }).abs() < 0.00004
                );
                assert!(a.peak > 0.24);
                assert!(!a.waveform.is_empty());
            }
        }
    }
}
#[test]
fn flac_short_final_block() {
    let f = Fixture::new();
    for len in [1, 15, 16, 4096, 4097] {
        let p = f.0.join(format!("short-{len}.flac"));
        let mut e = Encoder::create(&p, 16000, 1, 24).unwrap();
        e.write(&vec![[0.125; 2]; len]).unwrap();
        e.finish().unwrap();
        let mut a = Asset::decode(&p, &f.0.join("cache")).unwrap();
        assert_eq!(a.frames, len as u64);
        assert_eq!(a.frame(len as i64 - 1).unwrap()[0], 0.125);
    }
}
#[test]
fn conversion_resampling() {
    let f = Fixture::new();
    let p = f.audio("input.flac", 2, 24);
    let out = f.0.join("output.wav");
    codec::convert(&p, &out, &f.0.join("cache"), 24000, 24).unwrap();
    let a = Asset::decode(&out, &f.0.join("cache")).unwrap();
    assert_eq!(a.frames, 12000);
    assert_eq!(a.rate, 24000);
    assert!(codec::convert(&p, &out, &f.0.join("cache"), 24000, 24).is_err());
}
#[test]
fn synths_are_finite_and_audible() {
    for kind in [
        "sine",
        "saw",
        "square",
        "triangle",
        "noise",
        "analog",
        "soft-lead",
    ] {
        let b = rendered(&format!(
            "rate 16000;bars 1;track x {{synth {kind};notes \"C4:4\"}}"
        ));
        assert!(b.iter().all(|f| f.iter().all(|v| v.is_finite())));
        let energy: f64 = b.iter().map(|f| f[0] as f64 * f[0] as f64).sum();
        assert!(energy > 1., "{kind}: {energy}");
    }
}
#[test]
fn scheduling_and_loop() {
    let b = rendered("rate 16000;tempo 120;bars 1;track a { notes \"- C4\";step 1;loop }");
    assert!(b[..8000].iter().all(|f| *f == [0.; 2]));
    assert!(b[8001..15000].iter().any(|f| f[0].abs() > 0.01));
    assert!(b[16000..24000].iter().all(|f| *f == [0.; 2]));
}
#[test]
fn mix_mute_solo_pan() {
    let a = rendered("rate 16000;bars 1;track a { notes C4:4;pan -1 }");
    assert!(a.iter().all(|f| f[1].abs() < 1e-6));
    let b = rendered(
        "rate 16000;bars 1;track a { notes C4:4;mute }; track b { notes E4:4;solo;gain 0 }",
    );
    assert!(b.iter().all(|f| *f == [0.; 2]));
}
#[test]
fn deterministic_render() {
    let src = "rate 16000;bars 1;track a {pattern \"kick snare hat clap\";loop;delay 0.1 0.3 0.25;reverb 0.2};master limit -1";
    let a = rendered(src);
    assert_eq!(a, rendered(src));
    assert!(a.iter().any(|f| f[0].abs() > 0.01));
}
#[test]
fn effects() {
    for (kind, args) in [
        ("eq", vec![3., -2., 1.]),
        ("compress", vec![-12., 4.]),
        ("limit", vec![-3.]),
        ("gate", vec![-30.]),
        ("denoise", vec![-50.]),
        ("delay", vec![0.01, 0.5, 0.3]),
        ("reverb", vec![0.2]),
        ("filter", vec![1000.]),
    ] {
        let e = language::Effect {
            kind: kind.into(),
            args,
        };
        let mut p = dsp::Processor::new(&e, 16000);
        let mut energy = 0.;
        for i in 0..16000 {
            let v = p.process(if i == 0 { [0.9; 2] } else { [0.; 2] });
            assert!(v.iter().all(|v| v.is_finite()));
            energy += v[0] * v[0];
        }
        assert!(energy >= 0.);
    }
}
#[test]
fn automation() {
    assert_eq!(dsp::automation(0.5, &[[0., 0.], [1., 1.]], 2.), 0.5);
    let b = rendered("rate 16000;bars 1;track a {notes C4:4;automate gain \"0=0 1=0\"}");
    assert!(b.iter().all(|f| *f == [0.; 2]));
}
#[test]
fn nondestructive_edits() {
    let f = Fixture::new();
    let path = f.audio("input.wav", 1, 24);
    let original = fs::read(&path).unwrap();
    let src = "rate 16000;bars 1;track a {import input.wav;trim 0.05 0.45;split 0.2;place at 0.5;fade in 20ms;fade out 20ms;reverse;stretch 1.2;pitch 2;normalize;duplicate 1;clip-gain -3db;group dialogue;clip-effect limit -1;pan -0.2}";
    let s = language::parse(src).unwrap();
    assert_eq!(s.tracks[0].clips.len(), 3);
    let mut r = Renderer::prepare(&f.0, s).unwrap();
    r.render(&f.0.join("mix.flac")).unwrap();
    assert!(r.peak > 0.1);
    assert_eq!(fs::read(path).unwrap(), original);
}
#[test]
fn missing_and_escaping_assets() {
    let f = Fixture::new();
    assert!(codec::safe_path(&f.0, "../secret.wav").is_err());
    assert!(codec::safe_path(&f.0, "/tmp/secret.wav").is_err());
    let s = language::parse("track a { import missing.flac }").unwrap();
    assert!(Renderer::prepare(&f.0, s).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", f.0.join("outside.wav")).unwrap();
        assert!(codec::safe_path(&f.0, "outside.wav").is_err());
    }
}
#[test]
fn history() {
    let mut h = History::default();
    h.checkpoint("first");
    assert_eq!(h.undo("second").unwrap(), "first");
    assert_eq!(h.redo("first").unwrap(), "second");
    h.checkpoint("third");
    assert!(h.redo("fourth").is_none());
}
#[test]
fn projects_save_reopen_recover() {
    let f = Fixture::new();
    let mut p =
        Project::create(&f.0.join("song"), "project One; track a {notes C4}".into()).unwrap();
    p.source = "project Two;track a {notes E4}".into();
    p.save().unwrap();
    assert_eq!(Project::open(&p.root).unwrap().source, p.source);
    Project::recover(&p.root).unwrap();
    assert!(Project::open(&p.root).unwrap().source.contains("One"));
}
#[test]
fn portable_save_as() {
    let f = Fixture::new();
    let external = f.audio("outside.flac", 2, 24);
    let mut p = Project::create(&f.0.join("original"), "project Portable".into()).unwrap();
    let imported = p.import(&external).unwrap();
    p.source = format!("project Portable; track a {{import \"{imported}\"}}");
    p.save().unwrap();
    let q = p.save_as(&f.0.join("copy")).unwrap();
    assert!(q.root.join(imported).exists());
    let s = language::parse(&q.source).unwrap();
    assert_eq!(Renderer::prepare(&q.root, s).unwrap().tracks.len(), 1);
}
#[test]
fn phonemes_and_alignment() {
    let f = Fixture::new();
    f.bank();
    let v = Voice {
        bank: "voices/test".into(),
        register: "alto".into(),
        style: String::new(),
        ..Default::default()
    };
    let bank = LoadedBank::load(&f.0, &v, &f.0.join("cache")).unwrap();
    assert_eq!(
        voice::phonemes("ah ~ @AA", &bank.meta.dictionary)
            .unwrap()
            .len(),
        3
    );
    assert!(voice::phonemes("notindictionary", &bank.meta.dictionary).is_err());
    let mut t = Track::new("singer".into());
    t.lyrics = "ah ~ @AA".into();
    let e = language::events("A3 B3 C4", 1.).unwrap();
    let u = voice::align(&bank, &v, &t, &e, 120.).unwrap();
    assert_eq!(u.len(), 3);
    assert_eq!(u[1].midi, 59);
    t.lyrics = "ah".into();
    assert!(voice::align(&bank, &v, &t, &e, 120.).is_err());
}
#[test]
fn vocal_output_and_determinism() {
    let f = Fixture::new();
    f.bank();
    let src = "rate 16000;bars 1;voice Test {language english;bank voices/test;register alto};track singer {use voice Test;lyrics \"@AA ~\";notes \"A3:2 C4:2\";vibrato 0.2;breathiness 0.1}";
    let mut r = Renderer::prepare(&f.0, language::parse(src).unwrap()).unwrap();
    let p = f.0.join("sung.flac");
    r.render(&p).unwrap();
    let a = Asset::decode(&p, &f.0.join("cache")).unwrap();
    assert_eq!(a.frames, 32000);
    assert!(a.peak > 0.05);
    let mut second = Renderer::prepare(&f.0, language::parse(src).unwrap()).unwrap();
    let q = f.0.join("again.flac");
    second.render(&q).unwrap();
    assert_eq!(fs::read(p).unwrap(), fs::read(q).unwrap());
}
#[test]
fn invalid_voicebank() {
    let f = Fixture::new();
    f.bank();
    let manifest = f.0.join("voices/test/voicebank.toml");
    let s = fs::read_to_string(&manifest)
        .unwrap()
        .replace("end = 0.45", "end = 10.0");
    fs::write(manifest, s).unwrap();
    assert!(voice::validate(&f.0, "voices/test").is_err());
}
#[test]
fn shared_renderer_block_size() {
    let f = Fixture::new();
    let s = language::parse(
        "rate 16000; bars 1;track a {notes \"C4 E4 G4 C5\";loop;reverb 0.3;delay 0.01 0.2 0.1}",
    )
    .unwrap();
    let mut r = Renderer::prepare(&f.0, s.clone()).unwrap();
    let mut q = Renderer::prepare(&f.0, s).unwrap();
    let mut a = vec![[0.; 2]; r.length as usize];
    r.block(&mut a).unwrap();
    let mut b = vec![];
    let mut block = [[0.; 2]; 113];
    loop {
        let n = q.block(&mut block).unwrap();
        if n == 0 {
            break;
        }
        b.extend_from_slice(&block[..n]);
    }
    assert_eq!(a, b);
}
#[test]
fn live_boundary_seek() {
    let f = Fixture::new();
    let s = language::parse("rate 16000; tempo 120;bars 2;track a {notes C4:4;loop;reverb 0.1}")
        .unwrap();
    let mut r = Renderer::prepare(&f.0, s.clone()).unwrap();
    assert_eq!(r.bar_frames(), 32000);
    r.seek(32000).unwrap();
    let mut q = Renderer::prepare(&f.0, s).unwrap();
    let mut prior = vec![[0.; 2]; 32000];
    q.block(&mut prior).unwrap();
    let mut a = [[0.; 2]; 100];
    let mut b = a;
    r.block(&mut a).unwrap();
    q.block(&mut b).unwrap();
    assert_eq!(a, b);
}
#[test]
fn dictionary_lookup() {
    for w in [
        "music",
        "beautiful",
        "synthesis",
        "hello",
        "dictionary",
        "workstation",
    ] {
        let p = voice::english(w).unwrap();
        assert!(!p.chars().any(|c| c.is_ascii_digit()));
        assert!(
            p.split_whitespace()
                .all(|p| p == "/" || p.chars().all(|c| c.is_ascii_uppercase()))
        );
    }
    assert!(voice::english("qzxwvwqqq").is_none());
    assert!(voice::english("beautiful").unwrap().contains('/'));
}
#[test]
fn interrupted_wav_recovery() {
    use std::io::{Seek, SeekFrom, Write};
    let f = Fixture::new();
    let original = f.audio("take.wav", 2, 24);
    let mut file = fs::OpenOptions::new().write(true).open(&original).unwrap();
    file.seek(SeekFrom::Start(4)).unwrap();
    file.write_all(&0u32.to_le_bytes()).unwrap();
    let bytes = fs::read(&original).unwrap();
    let data = bytes.windows(4).position(|v| v == b"data").unwrap() as u64;
    file.seek(SeekFrom::Start(data + 4)).unwrap();
    file.write_all(&0u32.to_le_bytes()).unwrap();
    drop(file);
    let before = fs::read(&original).unwrap();
    let recovered = codec::recover_wav(&original).unwrap();
    let a = Asset::decode(&recovered, &f.0.join("cache")).unwrap();
    assert_eq!(a.frames, 8000);
    assert!(a.peak > 0.2);
    assert_eq!(before, fs::read(original).unwrap());
}
#[test]
fn symlinked_project_directory_is_rejected() {
    #[cfg(unix)]
    {
        let f = Fixture::new();
        let outside = Fixture::new();
        let root = f.0.join("song");
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside.0, root.join("cache")).unwrap();
        assert!(Project::create(&root, "project Unsafe".into()).is_err());
    }
}
#[test]
fn live_preroll_is_off_playback_worker() {
    use songbird::audio::{LiveContext, Meters};
    use std::sync::Arc;
    let f = Fixture::new();
    let s = language::parse("rate 16000;tempo 120;bars 8;track a {notes C4:4;loop}").unwrap();
    let mut r = Renderer::prepare(&f.0, s).unwrap();
    let live = LiveContext {
        clock: None,
        state: Arc::new(Meters::default()),
        bar: 32000,
        rate: 16000,
    };
    live.preroll(&mut r).unwrap();
    assert!(r.position >= 64000);
    assert_eq!(r.position % 32000, 0);
}
#[test]
fn save_as_preserves_unused_takes() {
    let f = Fixture::new();
    let p = Project::create(&f.0.join("original"), "project Takes".into()).unwrap();
    fs::write(p.root.join("recordings/unused.wav"), b"original take").unwrap();
    let q = p.save_as(&f.0.join("copied")).unwrap();
    assert_eq!(
        fs::read(q.root.join("recordings/unused.wav")).unwrap(),
        b"original take"
    );
    assert!(p.save_as(&p.root.join("nested")).is_err());
}
#[test]
fn integrated_mix_reopen_and_two_formats() {
    let f = Fixture::new();
    let mut p = Project::create(&f.0.join("integrated"), "project Integration".into()).unwrap();
    let tone = f.audio("tone.wav", 1, 24);
    let tone2 = f.audio("tone.flac", 2, 24);
    let a = p.import(&tone).unwrap();
    let b = p.import(&tone2).unwrap();
    p.source = format!(
        "project Integration;rate 16000;bars 1;track drums {{pattern \"kick hat snare hat\";loop;gain -12db}};track bass {{synth saw;notes \"C2:2 G2:2\";gain -12db;filter 900}};track melody {{notes \"C4 E4 G4 C5\";gain -10db;reverb 0.2}};track wav {{import {a};trim 0.05 0.45;place at 0.3;fade in 10ms;reverse;pan -0.3;gain -9db}};track flac {{import {b};at 1;gain -9db;pan 0.4}};master limit -1"
    );
    p.save().unwrap();
    let q = Project::open(&p.root).unwrap();
    let mut first = Renderer::prepare(&p.root, language::parse(&p.source).unwrap()).unwrap();
    let mut second = Renderer::prepare(&q.root, language::parse(&q.source).unwrap()).unwrap();
    let wav = p.root.join("exports/full.wav");
    let flac = p.root.join("exports/full.flac");
    first.render(&wav).unwrap();
    second.render(&flac).unwrap();
    let mut a = Asset::decode(&wav, &p.root.join("cache")).unwrap();
    let mut b = Asset::decode(&flac, &p.root.join("cache")).unwrap();
    assert!(a.peak > 0.1 && a.peak <= 0.9);
    assert_eq!(a.frames, b.frames);
    for index in (0..a.frames as i64).step_by(79) {
        assert_eq!(a.frame(index).unwrap(), b.frame(index).unwrap());
    }
}
#[test]
fn flac_transient_has_bounded_representation() {
    let f = Fixture::new();
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/flac-transient-24.wav");
    let output = f.0.join("bounded.flac");
    codec::convert(&input, &output, &f.0.join("cache"), 48000, 24).unwrap();
    assert!(
        fs::metadata(&output).unwrap().len() < 32768,
        "FLAC prediction must fall back before expanding beyond PCM size"
    );
    let mut a = Asset::decode(&input, &f.0.join("cache")).unwrap();
    let mut b = Asset::decode(&output, &f.0.join("cache")).unwrap();
    assert_eq!(a.frames, b.frames);
    for i in 0..a.frames as i64 {
        assert_eq!(a.frame(i).unwrap(), b.frame(i).unwrap());
    }
    let stream = claxon::FlacReader::open(&output).unwrap();
    assert_ne!(stream.streaminfo().md5sum, [0; 16]);
}
#[test]
fn new_project_preserves_existing_source() {
    let f = Fixture::new();
    fs::write(f.0.join("composition.sb"), "my source").unwrap();
    assert!(Project::create(&f.0, "project New".into()).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("composition.sb")).unwrap(),
        "my source"
    );
}
#[test]
fn dictionary_variants_do_not_break_possessive_lookup() {
    for word in ["a's", "aaronson's", "activists'", "hello", "world"] {
        assert!(
            voice::english(word).is_some(),
            "dictionary entry {word} must be searchable after build-time sorting"
        );
    }
}

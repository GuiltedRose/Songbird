use songbird::{codec, engine::Renderer, language, project::Project};
struct Fixture(Project);
impl Fixture {
    fn new(source: &str) -> Self {
        let root = codec::unique(&std::env::temp_dir(), "songbird-sound-syntax", "dir").unwrap();
        Self(Project::create(&root, source.into()).unwrap())
    }
    fn render(&self) -> Renderer {
        Renderer::prepare(&self.0.root, language::parse(&self.0.source).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0.root);
    }
}
#[test]
fn constructor_end_patterns_and_fast_keep_modifier_order() {
    let source = "tempo 120\nbars 4\n$: s(\"bd*2,oh*4\").end(\"<.1 .2 .5 1>\").fast(2)";
    let fixture = Fixture::new(source);
    let mut renderer = fixture.render();
    let events = &renderer.tracks[0].events;
    assert_eq!(events.len(), 48);
    assert!(events.iter().any(|e| e.drum.as_deref() == Some("open-hat")));
    assert_eq!(events[0].playback.as_ref().unwrap().end, 0.1);
    assert!(
        events
            .iter()
            .filter(|e| (e.beat - 2.).abs() < 1e-8)
            .all(|e| e.playback.as_ref().unwrap().end == 0.2)
    );
    let mut block = [[0.; 2]; 1024];
    assert_eq!(renderer.block(&mut block).unwrap(), 1024);
    assert!(block.iter().flatten().any(|v| v.abs() > 0.001));
    assert!(block.iter().flatten().all(|v| v.is_finite()));
}
#[test]
fn regular_and_mini_tracks_and_native_controls_coexist() {
    let source = "track native { synth triangle; notes \"C4:1 E4:1\" }\ntrack oldmini { synth sine; notation mini; notes \"c3 <e3 g3>\" }\ntrack drums {\ns('[bd <hh oh>]*2').dec(.4)\ngain -12db\n}\n";
    let fixture = Fixture::new(source);
    let renderer = fixture.render();
    assert!(!renderer.tracks[0].spec.mini);
    assert!(renderer.tracks[1].spec.mini);
    assert!(renderer.tracks[2].spec.mini_sound);
    assert!(
        renderer.tracks[2]
            .events
            .iter()
            .all(|e| e.playback.as_ref().unwrap().decay == Some(0.4))
    );
    assert!((renderer.tracks[2].spec.gain - 10f32.powf(-12. / 20.)).abs() < 1e-6);
    assert!(
        renderer.tracks[0]
            .events
            .iter()
            .all(|e| e.playback.is_none())
    );
}
#[test]
fn shorthand_in_regular_patterns_and_native_percussion_are_distinct() {
    let fixture = Fixture::new(
        "track kit { instrument drumkit; pattern \"bd sd hh oh cp rim cr rd ht mt lt sh cb tb\" }",
    );
    let renderer = fixture.render();
    let names: Vec<_> = renderer.tracks[0]
        .events
        .iter()
        .map(|e| e.drum.as_deref().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
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
            "tambourine"
        ]
    );
    assert_ne!(
        songbird::dsp::drum("hat", 0.2, 9600),
        songbird::dsp::drum("open-hat", 0.2, 9600)
    );
}
#[test]
fn local_samples_override_aliases_and_end_uses_real_duration() {
    let fixture = Fixture::new("track kit {\ndrum bd \"samples/kick.wav\"\ns(\"bd\").end(.5)\n}");
    let mut encoder =
        codec::Encoder::create(&fixture.0.root.join("samples/kick.wav"), 48000, 1, 24).unwrap();
    encoder.write(&vec![[0.25; 2]; 4800]).unwrap();
    encoder.finish().unwrap();
    let mut renderer = fixture.render();
    assert_eq!(renderer.tracks[0].events[0].drum.as_deref(), Some("bd"));
    let mut data = vec![[0.; 2]; 4800];
    renderer.block(&mut data).unwrap();
    assert!(data[100][0] > 0.01);
    assert!(data[2500..].iter().flatten().all(|v| *v == 0.));
}
#[test]
fn malformed_expressions_and_unknown_functions_fail_without_evaluation() {
    for source in [
        "s(\"bd\").fast(0)",
        "s(\"bd\").fast(1+1)",
        "s(\"bd\").unknown(1)",
        "s(\"bd\").end(\"oops\")",
        "s(\"bd\").end(2)",
        "s(\"bd\").dec(-1)",
    ] {
        match language::parse(source) {
            Err(_) => {}
            Ok(score) => {
                let fixture = Fixture::new("project Test");
                assert!(
                    Renderer::prepare(&fixture.0.root, score).is_err(),
                    "{source}"
                );
            }
        }
    }
    let source = "# s(\"bd\") is just a comment\ntrack native { synth sine; notes \"C4\" }";
    assert_eq!(language::parse(source).unwrap().tracks.len(), 1);
}

#[test]
fn sections_and_whitespace_keep_native_structure() {
    let source =
        "section verse at 0b {\ntrack\tkit {\ns\t(\"bd hh\").slow(2).sustain(.5).decay(.2)\n}\n}\n";
    let fixture = Fixture::new(source);
    let renderer = fixture.render();
    assert_eq!(renderer.tracks[0].spec.name, "verse/kit");
    assert!(
        renderer.tracks[0]
            .events
            .iter()
            .all(|e| e.playback.as_ref().unwrap().sustain == 0.5)
    );
    let source = "section verse at 0b {\n$: sound(\"bd hh\")\n}\n";
    assert_eq!(
        language::parse(source).unwrap().tracks[0].name,
        "verse/__sound_1"
    );
}
#[test]
fn bundled_mixed_example_uses_the_existing_renderer() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/shorthand-session");
    let score =
        language::parse(include_str!("../examples/shorthand-session/composition.sb")).unwrap();
    assert_eq!(score.tracks.len(), 4);
    let mut renderer = Renderer::prepare(&root, score).unwrap();
    let mut block = [[0.; 2]; 256];
    assert_eq!(renderer.block(&mut block).unwrap(), 256);
    assert!(block.iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn modifier_order_is_observable_and_dense_or_gapped_controls_fail() {
    let a = Fixture::new("s(\"bd\").end(\"<.25 .5>\").fast(2)");
    let b = Fixture::new("s(\"bd\").fast(2).end(\"<.25 .5>\")");
    assert_eq!(
        a.render().tracks[0].events[1]
            .playback
            .as_ref()
            .unwrap()
            .end,
        0.5
    );
    assert_eq!(
        b.render().tracks[0].events[1]
            .playback
            .as_ref()
            .unwrap()
            .end,
        0.25
    );
    for source in [
        "s(\"oh*256\")",
        "s(\"bd\").end(\"~\")",
        "s(\"bd\").sustain(2)",
    ] {
        let score = language::parse(source).unwrap();
        let fixture = Fixture::new("project Test");
        assert!(
            Renderer::prepare(&fixture.0.root, score).is_err(),
            "{source}"
        );
    }
}

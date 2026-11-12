use songbird::{
    dsp::{Processor, effects_bypassed},
    language::{self, Effect},
    mixer::Mixer,
};
fn mixer(source: &str) -> Mixer {
    Mixer::compile(&language::parse(source).unwrap())
        .unwrap()
        .unwrap()
}
#[test]
fn automatic_channels_preserve_the_legacy_path() {
    let score = language::parse("track one { synth sine; notes C4 }").unwrap();
    assert!(Mixer::compile(&score).unwrap().is_none());
}
#[test]
fn multiple_sources_nested_buses_and_pre_post_sends_are_exact() {
    let mut graph = mixer(
        "track a { gain 0; send fx 0.5 pre } track b {} channel group { source a; source b; gain 0.5; route out } bus fx { route out } bus out { gain 0.5 }",
    );
    let mut master = [0.; 2];
    graph.begin_frame();
    graph.track(0, [1.; 2], [0.; 2], &mut master);
    graph.track(1, [1.; 2], [1.; 2], &mut master);
    graph.process(0., &mut master).unwrap();
    assert_eq!(master, [0.5; 2]);
    assert_eq!(graph.latency_frames(), 0);
    assert!(graph.meters.iter().any(|m| m.peak > 0.));
    let mut post = mixer("track a { send fx 0.5 post } bus fx {}");
    let mut master = [0.; 2];
    post.begin_frame();
    post.track(0, [1.; 2], [0.; 2], &mut master);
    post.process(0., &mut master).unwrap();
    assert_eq!(master, [0.; 2]);
}
#[test]
fn invalid_sources_routes_and_cycles_fail_before_processing() {
    for source in [
        "track a { route missing }",
        "bus a { route b } bus b { route a }",
        "bus a { send a 0 pre }",
        "channel a { source missing }",
        "track a {} channel x { source a } channel y { source a }",
        "track a { route master } channel x { source a }",
        "bus a { sidechain b } bus b { sidechain a }",
        "bus a { sidechain missing }",
    ] {
        let score = language::parse(source).unwrap();
        assert!(Mixer::compile(&score).is_err(), "{source}");
    }
    assert!(language::parse("track a { send fx 1 before }").is_err());
    assert!(language::parse("track a { bypass 2; filter 200 }").is_err());
}
#[test]
fn solo_bus_selects_sources_and_bypassed_inserts_are_transparent() {
    let graph = mixer("track a { route group } track b {} bus group { solo }");
    assert!(graph.track_audible(0));
    assert!(!graph.track_audible(1));
    let effect = Effect {
        kind: "limit".into(),
        args: vec![-20.],
    };
    let mut bypass = effects_bypassed(&[effect], &[1], 48000);
    assert_eq!(bypass[0].process([0.5; 2]), [0.5; 2]);
}
#[test]
fn mixer_automation_and_sidechain_reduce_actual_signal() {
    let mut graph = mixer(
        "track bass {} track kick {} channel duck { source bass; sidechain kick; compressor -24 4 1 100 6 0; automate gain \"0=1 1=0\" } ",
    );
    let mut last = [0.; 2];
    for i in 0..4800 {
        last = [0.; 2];
        graph.begin_frame();
        graph.track(0, [0.5; 2], [0.5; 2], &mut last);
        graph.track(1, [1.; 2], [1.; 2], &mut last);
        graph.process(i as f64 / 48000., &mut last).unwrap();
    }
    // Kick still reaches master; compare bass contribution above that signal.
    assert!((last[0] - 1.).abs() < 0.2);
    assert!(graph.meters[0].reduction_db > 10.);
}
#[test]
fn new_dsp_is_finite_stereo_safe_and_measurably_processes_audio() {
    for rate in [16000, 44100, 48000, 96000] {
        for (kind, args) in [
            ("highpass", vec![40., 0.707]),
            ("lowpass", vec![5000., 0.707]),
            ("bell", vec![1000., -6., 1.2]),
            ("lowshelf", vec![100., 6., 0.707]),
            ("highshelf", vec![4000., -6., 0.707]),
            ("noisegate", vec![-40., 2., 100., 20.]),
            ("compressor", vec![-20., 4., 5., 100., 6., 0.]),
            ("width", vec![0.]),
        ] {
            let mut processor = Processor::new(
                &Effect {
                    kind: kind.into(),
                    args,
                },
                rate,
            );
            for i in 0..rate {
                let sample = (i as f32 * 0.11).sin() * 0.5;
                let output = processor.process([sample, -sample]);
                assert!(output.iter().all(|v| v.is_finite() && v.abs() < 10.));
            }
        }
    }
    let mut width = Processor::new(
        &Effect {
            kind: "width".into(),
            args: vec![0.],
        },
        48000,
    );
    assert_eq!(width.process([1., -1.]), [0.; 2]);
}
#[test]
fn native_and_mini_route_through_the_same_graph() {
    let score = language::parse("track kick { instrument drumkit; notation mini; pattern \"kick snare\"; drum-only kick; route drums } track bass { synth saw; notes C2; route music } bus drums { route music } bus music { highpass 30 0.707 }").unwrap();
    assert!(score.tracks[0].mini);
    assert!(!score.tracks[1].mini);
    assert!(Mixer::compile(&score).unwrap().is_some());
}

#[test]
fn routed_renderer_seek_restore_and_legacy_identity_are_deterministic() {
    use songbird::{codec, engine::Renderer, project::Project};
    let root = codec::unique(&std::env::temp_dir(), "songbird-routing", "dir").unwrap();
    let source =
        "project Test; rate 16000; bars 1; track tone { synth sine; notes C4:4; gain 0.2 }";
    let project = Project::create(&root, source.into()).unwrap();
    let legacy = language::parse(source).unwrap();
    let mut routed = legacy.clone();
    routed.tracks[0].routing.route = Some("master".into());
    let mut a = Renderer::prepare(&project.root, legacy).unwrap();
    let mut b = Renderer::prepare(&project.root, routed).unwrap();
    let (mut left, mut right) = ([[0.; 2]; 512], [[0.; 2]; 512]);
    for _ in 0..4 {
        assert_eq!(a.block(&mut left).unwrap(), b.block(&mut right).unwrap());
        assert_eq!(left, right);
    }
    let delayed = language::parse("rate 16000; bars 1; track tone { synth sine; notes C4:4; gain 0.2; route group } bus group { delay 0.02 0.3 0.5 }").unwrap();
    let mut delayed = Renderer::prepare(&project.root, delayed).unwrap();
    delayed.block(&mut left).unwrap();
    delayed.seek(0).unwrap();
    delayed.block(&mut right).unwrap();
    assert_eq!(left, right);
    drop((a, b, delayed));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn bundled_hybrid_example_prepares_with_the_shared_engine() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/mixer-routing");
    let score = language::parse(include_str!("../examples/mixer-routing/composition.sb")).unwrap();
    let mut renderer = songbird::engine::Renderer::prepare(&root, score).unwrap();
    let mut block = [[0.; 2]; 256];
    assert_eq!(renderer.block(&mut block).unwrap(), 256);
    assert!(block.iter().flatten().all(|v| v.is_finite()));
}

#[test]
fn routed_projects_reopen_and_exports_have_deterministic_pcm() {
    use songbird::{codec, engine::Renderer, project::Project};
    let root = codec::unique(&std::env::temp_dir(), "songbird-routing-export", "dir").unwrap();
    let source = "project Export; rate 16000; bits 24; bars 1; track a { synth sine; notes C4:4; gain 0.2; route group } bus group { bell 500 -2 1; route master }";
    let project = Project::create(&root, source.into()).unwrap();
    let reopened = Project::open(&root).unwrap();
    assert_eq!(reopened.source, project.source);
    let score = language::parse(&reopened.source).unwrap();
    for extension in ["wav", "flac"] {
        for index in 0..2 {
            Renderer::prepare(&root, score.clone())
                .unwrap()
                .render(&root.join(format!("{index}.{extension}")))
                .unwrap();
        }
    }
    {
        let mut reference = codec::Asset::decode(&root.join("0.wav"), &root.join("cache")).unwrap();
        for name in ["1.wav", "0.flac", "1.flac"] {
            let mut candidate =
                codec::Asset::decode(&root.join(name), &root.join("cache")).unwrap();
            assert_eq!(reference.frames, candidate.frames);
            for frame in 0..reference.frames {
                assert_eq!(
                    reference.frame(frame as i64).unwrap(),
                    candidate.frame(frame as i64).unwrap()
                );
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

use songbird::{
    codec,
    engine::Renderer,
    language,
    timeline::{Clock, Meter, Tempo},
};
use std::collections::BTreeMap;
fn clock() -> Clock {
    Clock::new(
        120.,
        [4, 4],
        &[
            Tempo {
                beat: 4.,
                bpm: 140.,
                end: None,
                end_bpm: None,
            },
            Tempo {
                beat: 8.,
                bpm: 140.,
                end: Some(16.),
                end_bpm: Some(100.),
            },
        ],
        &[Meter {
            bar: 3,
            signature: [6, 8],
        }],
    )
    .unwrap()
}
#[test]
fn analytic_ramp_inverse_and_long_arrangement() {
    let c = clock();
    assert_eq!(c.seconds(4.), 2.);
    assert!((c.seconds(16.) - (2. + 240. / 140. + 60. / -5. * (100f64 / 140.).ln())).abs() < 1e-10);
    for beat in [0., 3., 4., 7., 8., 9., 15.99, 16., 100., 10000.] {
        assert!((c.beat(c.seconds(beat)) - beat).abs() < 1e-9);
    }
    assert_eq!(c.bpm(12.), 120.);
    assert_eq!(c.bar_beat(4), 11.);
    assert_eq!(c.position(9.5), (3, 4, 0, [6, 8]));
    let markers = BTreeMap::from([("chorus".into(), 8.)]);
    assert_eq!(c.parse("@chorus", &markers).unwrap(), c.seconds(8.));
    assert_eq!(c.parse("bar:3:4", &markers).unwrap(), c.seconds(9.5));
    assert!(c.parse("bar:3:7", &markers).is_err());
}
#[test]
fn conflicting_maps_are_rejected() {
    for source in [
        "tempo at 4b 140; tempo at 4b 150",
        "tempo ramp 2b 8b 120 140; tempo at 4b 160",
        "tempo ramp 8b 4b 120 140",
        "tempo at 4b 0",
        "signature at bar 1 3/4",
        "signature at bar 3 6/8; signature at bar 3 4/4",
    ] {
        assert!(language::parse(source).is_err(), "{source}");
    }
}
#[test]
fn scheduling_positions_and_automation_follow_map() {
    let score = language::parse("tempo 120; tempo at 4b 240; signature at bar 3 6/8; bars 4; section chorus at 8b { track melody { at 2b; notation mini; notes c4; cycle 4; loop; automate gain \"0b=0 2b=1\" } }").unwrap();
    assert_eq!(score.duration(), 4.5);
    assert_eq!(score.tracks[0].at, 3.5);
    assert_eq!(score.tracks[0].automation[0].points[1][0], 0.5);
    let marker = language::parse("marker chorus at 8b; tempo at @chorus 140; section verse at @chorus { track a { notes C4 } }").unwrap();
    assert_eq!(marker.tracks[0].at, 4.);
}
#[test]
fn rewind_and_loop_restore_effects_and_pcm() {
    let root = codec::unique(&std::env::temp_dir(), "songbird-clock", "dir").unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let score = language::parse("rate 16000; bars 2; tempo at 4b 140; track a { notation mini; notes \"c4 e4\"; loop; gain 0.1; delay 0.1 0.3 0.2 }").unwrap();
    let mut r = Renderer::prepare(&root, score).unwrap();
    r.seek(8000).unwrap();
    let mut first = [[0.; 2]; 500];
    r.block(&mut first).unwrap();
    r.seek(8000).unwrap();
    let mut again = [[0.; 2]; 500];
    r.block(&mut again).unwrap();
    assert_eq!(first, again);
    r.transport(8000, 8500, true).unwrap();
    let mut lap = [[0.; 2]; 500];
    r.block(&mut lap).unwrap();
    assert_eq!(first[128..], lap[128..]);
    r.block(&mut lap).unwrap();
    assert_eq!(first[128..], lap[128..]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn frame_schedule_follows_instant_changes_and_ramps() {
    let root = codec::unique(&std::env::temp_dir(), "songbird-map-frames", "dir").unwrap();
    std::fs::create_dir_all(root.join("cache")).unwrap();
    let mut encoder = codec::Encoder::create(&root.join("hit.wav"), 16000, 1, 24).unwrap();
    encoder.write(&[[0.4; 2]; 16]).unwrap();
    encoder.finish().unwrap();
    let score = language::parse("rate 16000; bars 4; tempo 120; tempo at 4b 240; tempo ramp 8b 12b 240 120; track a { notation mini; cycle 4; cycles 4; drum hit hit.wav; notes \"~ hit\" }").unwrap();
    let clock = score.clock().unwrap();
    let mut renderer = Renderer::prepare(&root, score).unwrap();
    let mut pcm = vec![];
    let mut block = [[0.; 2]; 512];
    loop {
        let n = renderer.block(&mut block).unwrap();
        if n == 0 {
            break;
        }
        pcm.extend_from_slice(&block[..n]);
    }
    for beat in [2., 6., 10., 14.] {
        let frame = (clock.seconds(beat) * 16000.).ceil() as usize;
        assert!(
            pcm[frame.saturating_sub(1)..frame + 3]
                .iter()
                .any(|x| x[0].abs() > 0.1)
        );
        assert!(
            pcm[frame.saturating_sub(20)..frame.saturating_sub(2)]
                .iter()
                .all(|x| x[0].abs() < 0.00001)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn count_in_does_not_advance_arrangement_and_uses_clock() {
    let root = codec::unique(&std::env::temp_dir(), "songbird-count", "dir").unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let score = language::parse("rate 16000; tempo 120; signature 6/8; bars 2").unwrap();
    let mut renderer = Renderer::prepare(&root, score).unwrap();
    renderer.count_in(1);
    assert_eq!(renderer.count_in_frames, 24000);
    let mut block = [[0.; 2]; 512];
    let mut peak = 0f32;
    while renderer.count_in_frames > 0 {
        let n = renderer.block(&mut block).unwrap();
        for frame in &block[..n] {
            peak = peak.max(frame[0].abs());
        }
    }
    assert_eq!(renderer.position, 0);
    assert!(peak > 0.01);
    renderer.block(&mut block).unwrap();
    assert_eq!(renderer.position, 512);
    std::fs::remove_dir_all(root).unwrap();
}

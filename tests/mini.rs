use songbird::{language, mini::Pattern};
fn events(text: &str, cycles: f64) -> Vec<language::Event> {
    Pattern::compile(text)
        .unwrap()
        .events(0., cycles, 4., 42)
        .unwrap()
}
fn starts(text: &str, cycles: f64) -> Vec<f64> {
    events(text, cycles).iter().map(|e| e.beat).collect()
}
#[test]
fn sequence_subdivision_rests_parallel_and_weights() {
    assert_eq!(starts("c4 ~ e4", 1.), vec![0., 8. / 3.]);
    assert_eq!(
        starts("[c4 [e4 g4] a4]", 1.),
        vec![0., 4. / 3., 2., 8. / 3.]
    );
    assert_eq!(starts("c4,e4,g4", 1.), vec![0., 0., 0.]);
    let weighted = events("c4@3 e4", 1.);
    assert_eq!(weighted[0].duration, 3.);
    assert_eq!(weighted[1].beat, 3.);
}
#[test]
fn cycles_alternation_replication_and_fractional_speed() {
    assert_eq!(
        events("<c4 e4 g4>", 3.)
            .iter()
            .map(|e| e.notes[0])
            .collect::<Vec<_>>(),
        vec![60, 64, 67]
    );
    assert_eq!(starts("[c4 e4]*2", 1.), vec![0., 1., 2., 3.]);
    assert_eq!(starts("[c4 e4]/2", 2.), vec![0., 4.]);
    assert_eq!(events("c4/2", 2.)[0].duration, 8.);
    assert_eq!(starts("c4!3 e4", 1.), vec![0., 1., 2., 3.]);
    assert_eq!(starts("<c4!3 e4>", 4.), vec![0., 4., 8., 12.]);
    assert_eq!(starts("c4*2.5", 1.), vec![0., 1.6, 3.2]);
}
#[test]
fn euclidean_rotation_and_polymeter() {
    assert_eq!(starts("kick(3,8)", 1.), vec![0., 1.5, 3.]);
    assert_eq!(starts("kick(3,8,1)", 1.), vec![0.5, 2., 3.5]);
    assert_eq!(events("hat(0,8)", 1.).len(), 0);
    let p = events("{kick snare, hat*3}%8", 1.);
    assert_eq!(
        p.iter()
            .filter(|e| e.drum.as_deref() == Some("kick"))
            .count(),
        4
    );
    assert_eq!(
        p.iter()
            .filter(|e| e.drum.as_deref() == Some("hat"))
            .count(),
        24
    );
    let lcm = events("{c4 e4 g4, c3 g3}", 1.);
    assert_eq!(lcm.len(), 12);
}
#[test]
fn randomness_replays_and_range_queries_agree() {
    assert_eq!(events("hat?1", 4.).len(), 0);
    assert_eq!(events("hat?0", 4.).len(), 4);
    let replicated = events("hat?0.5!128", 1.);
    assert!(!replicated.is_empty() && replicated.len() < 128);
    let p = Pattern::compile("[c4|e4|g4]*8?0.25").unwrap();
    let all = p.events(0., 4., 4., 91).unwrap();
    assert_eq!(all, p.events(0., 4., 4., 91).unwrap());
    assert_ne!(all, p.events(0., 4., 4., 92).unwrap());
    let mut split = p.events(0., 2., 4., 91).unwrap();
    split.extend(p.events(2., 4., 4., 91).unwrap());
    assert_eq!(all, split);
}
#[test]
fn malformed_and_resource_exhausting_patterns_fail() {
    for bad in [
        "",
        "[]",
        "[c4",
        "c4]",
        "c4*0",
        "c4/0",
        "c4@0",
        "c4!0",
        "hat?2",
        "kick(9,8)",
        "{c4}%0",
        "c4||e4",
        "c4(3)",
    ] {
        assert!(Pattern::compile(bad).is_err(), "{bad}");
    }
    assert!(Pattern::compile(&format!("{}c4{}", "[".repeat(30), "]".repeat(30))).is_err());
    let p = Pattern::compile("[c4*64]*64").unwrap();
    assert!(p.events(0., 100., 4., 0).is_err());
}
#[test]
fn explicit_frontend_preserves_legacy_and_survives_serialization() {
    let legacy = language::parse("track melody { notes \"[C4,E4,G4]:2 (C4 E4)*2\" }").unwrap();
    assert!(!legacy.tracks[0].mini);
    assert_eq!(
        language::events(&legacy.tracks[0].sequence, 0.5)
            .unwrap()
            .len(),
        5
    );
    let mini = language::parse(
        "track melody { notation mini; cycle 3; cycles 4; seed 99; notes \"[c4 e4] <g4 a4>\" }",
    )
    .unwrap();
    let reopened: language::Score = toml::from_str(&toml::to_string(&mini).unwrap()).unwrap();
    assert!(reopened.tracks[0].mini);
    assert_eq!(reopened.tracks[0].seed, 99);
    assert_eq!(reopened.tracks[0].cycle_beats, 3.);
}

use songbird::{
    audio::Meters,
    codec, language,
    recording::{self, Gate, Spans},
    storage::{Lease, Limits},
};
use std::{
    fs,
    sync::{Arc, atomic::Ordering},
};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = codec::unique(&std::env::temp_dir(), "songbird-daw", "dir").unwrap();
        fs::create_dir_all(root.join("recordings")).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn punch_count_in_and_loop_takes_preserve_originals() {
    let f = Fixture::new();
    let transport = Arc::new(Meters::default());
    let mut gate = Gate {
        transport: transport.clone(),
        start: 100,
        end: 200,
        looping: false,
        spans: Arc::new(Spans::default()),
    };
    transport.count_in.store(20, Ordering::Relaxed);
    transport.frames.store(100, Ordering::Relaxed);
    assert_eq!(gate.status(), -1);
    transport.count_in.store(0, Ordering::Relaxed);
    assert_eq!(gate.status(), 0);
    transport.frames.store(200, Ordering::Relaxed);
    assert_eq!(gate.status(), 1);
    gate.looping = true;
    transport.frames.store(100, Ordering::Relaxed);
    assert_eq!(gate.status(), 0);
    let file = f.0.join("recordings/original.wav");
    fs::write(&file, b"original").unwrap();
    gate.spans.frames[1].store(16000, Ordering::Relaxed);
    gate.spans.count.store(1, Ordering::Release);
    gate.save_spans(&file, 32000, 16000).unwrap();
    let take = recording::register(&f.0, &file, 2., 100.).unwrap();
    let entries = recording::takes(&f.0).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].source, Some((0., 1.)));
    assert_eq!(entries[1].source, Some((1., 2.)));
    assert_eq!(take.at, 1.9);
    let score = language::parse(&recording::clip(&entries[1], "comp", 4., None).unwrap()).unwrap();
    assert_eq!(score.tracks[0].clips[0].start, 1.);
    assert_eq!(fs::read(file).unwrap(), b"original");
}
#[test]
fn aggregate_quota_retains_leased_cache_and_evicts_orphans_only() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    fs::create_dir(&cache).unwrap();
    let limits = Limits {
        cache_bytes: 100,
        minimum_free_bytes: 0,
    };
    fs::write(cache.join("pcm-orphan.raw"), [0; 60]).unwrap();
    fs::write(cache.join("original.wav"), b"never evict").unwrap();
    let lease = Lease::reserve(cache.join("pcm-active.raw"), 80, &limits).unwrap();
    assert!(!cache.join("pcm-orphan.raw").exists());
    fs::write(&lease.path, [0; 80]).unwrap();
    assert!(Lease::reserve(cache.join("pcm-other.raw"), 30, &limits).is_err());
    assert!(lease.path.exists());
    drop(lease);
    assert!(Lease::reserve(cache.join("pcm-other.raw"), 30, &limits).is_ok());
    assert_eq!(
        fs::read(cache.join("original.wav")).unwrap(),
        b"never evict"
    );
}
#[test]
fn clip_selection_snap_relink_and_section_repeat() {
    let score = language::parse("tempo at 4b 240; section verse at 4b repeat 2 every 4b { track a { import one.wav; clip-id first; import two.wav; select-clip first; relink replacement.wav; trim 0s 1s; snap 0.5b; place 1.2b } }").unwrap();
    assert_eq!(score.tracks.len(), 2);
    assert_eq!(score.tracks[0].at, 2.);
    assert_eq!(score.tracks[1].at, 3.);
    let clip = &score.tracks[0].clips[0];
    assert_eq!(clip.path, "replacement.wav");
    assert_eq!(clip.at, 0.25);
    assert_eq!(score.tracks[0].clips[1].path, "two.wav");
}
#[test]
fn effect_and_vocal_automation_validate_boundaries() {
    assert!(language::parse("track a { delay 0.1 0.2 0.3; automate effect.1.2 \"0=0.1 1=0.5\"; automate formant \"0=1 1=1.5\" }").is_ok());
    assert!(
        language::parse("track a { delay 0.1 0.2 0.3; automate effect.1.1 \"0=0.1\" }").is_err()
    );
    assert!(language::parse("track a { automate formant \"0=3\" }").is_err());
}

#[test]
fn timeline_clip_splits_and_saved_transport_are_non_destructive() {
    let f = Fixture::new();
    let score = language::parse(
        "tempo at 4b 240; track comp { at 4b; import original.wav; trim 0s 2s; split-at 2b }",
    )
    .unwrap();
    assert_eq!(score.tracks[0].clips.len(), 2);
    assert_eq!(score.tracks[0].clips[0].end, Some(0.5));
    assert_eq!(score.tracks[0].clips[1].start, 0.5);
    let transport = songbird::timeline::Transport {
        cursor: 2.,
        range: Some((1., 3.)),
        looping: true,
    };
    transport.save(&f.0).unwrap();
    let reopened = songbird::timeline::Transport::load(&f.0).unwrap();
    assert_eq!(reopened.range, Some((1., 3.)));
    assert!(reopened.looping);
    let path = f.0.join("impossible.wav");
    assert!(songbird::storage::preflight(&path, u64::MAX, 1).is_err());
    assert!(!path.exists());
}

use songbird::{
    codec::{self, Asset, Encoder},
    engine::Renderer,
    export, language,
    project::Project,
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = codec::unique(&std::env::temp_dir(), "songbird-mini-render", "dir").unwrap();
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pcm(root: &std::path::Path, source: &str) -> Vec<[f32; 2]> {
    let mut renderer = Renderer::prepare(root, language::parse(source).unwrap()).unwrap();
    let mut result = Vec::new();
    let mut block = [[0.; 2]; 1024];
    loop {
        let n = renderer.block(&mut block).unwrap();
        if n == 0 {
            break;
        }
        result.extend_from_slice(&block[..n]);
    }
    result
}
#[test]
fn mini_playback_export_region_stems_and_reopen_share_pcm() {
    let f = Fixture::new();
    let source = "rate 16000; bits 24; bars 1; track melody { notation mini; notes \"<c4 e4>*4\"; loop; gain 0.1; delay 0.08 0.3 0.2; filter 1000; automate filter \"0=200 1=1800\"; automate pan \"0=-0.5 1=0.5\" step }";
    let project = Project::create(&f.0, source.into()).unwrap();
    let first = pcm(&f.0, source);
    assert_eq!(first, pcm(&f.0, &Project::open(&f.0).unwrap().source));
    let score = language::parse(&project.source).unwrap();
    let path = f.0.join("exports/region.wav");
    let report = export::region(&f.0, score.clone(), &path, 0.5, 1.5).unwrap();
    assert_eq!(report.frames, 16000);
    let mut asset = Asset::decode(&path, &f.0.join("cache")).unwrap();
    for i in [0, 100, 1000, 15999] {
        let value = asset.sample(i as f64, 1.).unwrap();
        for (ch, sample) in value.iter().enumerate() {
            assert!((*sample - first[8000 + i][ch]).abs() < 0.000002);
        }
    }
    assert!(export::region(&f.0, score.clone(), &path, 0.5, 1.5).is_err());
    let stems = f.0.join("exports/stems");
    let reports = export::stems(&f.0, &score, &stems).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].frames, first.len() as u64);
    assert!(export::stems(&f.0, &score, &stems).is_err());
}
#[test]
fn custom_samples_and_overlapping_singers_are_checked() {
    let f = Fixture::new();
    let mut encoder = Encoder::create(&f.0.join("hit.wav"), 16000, 1, 24).unwrap();
    encoder.write(&[[0.1; 2]; 100]).unwrap();
    encoder.finish().unwrap();
    let score = language::parse(
        "rate 16000; track sample { notation mini; drum hit hit.wav; notes \"hit*4\" }",
    )
    .unwrap();
    assert!(Renderer::prepare(&f.0, score).is_ok());
    let score =
        language::parse("rate 16000; track sample { notation mini; notes missing }").unwrap();
    assert!(Renderer::prepare(&f.0, score).is_err());
    let score = language::parse("rate 16000; voice test { bank default:la } track singer { notation mini; use voice test; notes \"c4,e4\"; lyrics salve }").unwrap();
    assert!(
        Renderer::prepare(&f.0, score)
            .err()
            .unwrap()
            .to_string()
            .contains("overlapping")
    );
}
#[test]
fn automation_interpolation_and_validation() {
    let points = [[0., 0.], [1., 1.]];
    assert_eq!(songbird::dsp::automation(0.5, &points, 0.), 0.5);
    assert_eq!(songbird::dsp::automation_step(0.5, &points, 0.), 0.);
    assert_eq!(songbird::dsp::automation_step(1., &points, 0.), 1.);
    assert!(language::parse("track a { automate filter \"0=200\" }").is_err());
    assert!(language::parse("track a { automate gain \"0=0 1=1\" spline }").is_err());
}

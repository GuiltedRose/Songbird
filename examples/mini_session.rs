//! cargo run --offline --example mini_session -- /tmp/songbird-mini-session
use anyhow::{Context, Result};
use songbird::{codec::Encoder, engine::Renderer, language, project::Project};
use std::path::PathBuf;
fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("pass a new project directory")?,
    );
    let project = Project::create(&root, include_str!("mini-session/composition.sb").into())?;
    let mut encoder = Encoder::create(&root.join("samples/generated.wav"), 16000, 1, 24)?;
    let data: Vec<_> = (0..8000)
        .map(|i| {
            let x = (std::f32::consts::TAU * 220. * i as f32 / 16000.).sin() * 0.15;
            [x, x]
        })
        .collect();
    encoder.write(&data)?;
    encoder.finish()?;
    for extension in ["flac", "wav"] {
        let mut renderer = Renderer::prepare(&root, language::parse(&project.source)?)?;
        renderer.render(&root.join(format!("exports/mix.{extension}")))?;
        println!(
            "{extension}: {} frames, peak {}, clips {}",
            renderer.position, renderer.peak, renderer.clipped
        );
    }
    let reopened = Project::open(&root)?;
    let mut renderer = Renderer::prepare(&root, language::parse(&reopened.source)?)?;
    let mut flac =
        songbird::codec::Asset::decode(&root.join("exports/mix.flac"), &root.join("cache"))?;
    let mut wav =
        songbird::codec::Asset::decode(&root.join("exports/mix.wav"), &root.join("cache"))?;
    let mut position = 0i64;
    let mut block = [[0.; 2]; 1024];
    loop {
        let n = renderer.block(&mut block)?;
        if n == 0 {
            break;
        }
        for frame in &block[..n] {
            let a = flac.frame(position)?;
            let b = wav.frame(position)?;
            for channel in 0..2 {
                anyhow::ensure!(
                    (frame[channel] - a[channel]).abs() < 0.000001
                        && (a[channel] - b[channel]).abs() < 0.000001,
                    "save/reopen or codec PCM mismatch at frame {position}"
                );
            }
            position += 1;
        }
    }
    anyhow::ensure!(
        position as u64 == flac.frames && flac.frames == wav.frames,
        "export length mismatch"
    );
    println!("Save/reopen and FLAC/WAV PCM equivalence verified for {position} frames");
    let report = songbird::export::region(
        &root,
        language::parse(&project.source)?,
        &root.join("exports/region.flac"),
        2.,
        6.,
    )?;
    println!("Region: {report:?}");
    let stems = songbird::export::stems(
        &root,
        &language::parse(&project.source)?,
        &root.join("exports/stems"),
    )?;
    println!("{} isolated stems", stems.len());
    Ok(())
}

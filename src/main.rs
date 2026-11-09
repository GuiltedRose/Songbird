use anyhow::{Context, Result, bail};
use songbird::{audio, codec, engine::Renderer, gui, language, project::Project, voice};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let arg = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .context("missing argument; songbird help")
    };
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => println!(
            "songbird [PROJECT]\nsongbird new DIR\nsongbird check PROJECT\nsongbird render PROJECT OUTPUT.flac|wav\nsongbird convert INPUT OUTPUT RATE BITS\nsongbird devices\nsongbird voice-check PROJECT BANK_PATH\nsongbird recover PROJECT\nsongbird recover-take TAKE.wav\nsongbird gui-smoke PROJECT\nsongbird bench PROJECT OUTPUT.wav\nNo command starts recording or playback without an explicit GUI action."
        ),
        Some("gui-smoke") => gui::smoke(Project::open(Path::new(arg(1)?))?)?,
        Some("recover-take") => println!("{}", codec::recover_wav(Path::new(arg(1)?))?.display()),
        Some("new") => {
            let p = Project::create(Path::new(arg(1)?), gui::DEFAULT.into())?;
            println!("Created {}", p.root.display());
        }
        Some("check") => {
            let p = Project::open(Path::new(arg(1)?))?;
            let r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
            println!(
                "{}: {} tracks, {:.3}s; assets parsed and decoded",
                r.score.name,
                r.tracks.len(),
                r.score.duration()
            );
        }
        Some("render" | "bench") => {
            let start = Instant::now();
            let p = Project::open(Path::new(arg(1)?))?;
            let mut r = Renderer::prepare(&p.root, language::parse(&p.source)?)?;
            let prepared = start.elapsed();
            r.render(Path::new(arg(2)?))?;
            println!(
                "{} frames; peak {:.6}; {} samples over full scale; prepare {:?}; total {:?}; render speed {:.2}x",
                r.length,
                r.peak,
                r.clipped,
                prepared,
                start.elapsed(),
                r.score.duration() / start.elapsed().as_secs_f64()
            );
        }
        Some("convert") => {
            let cache = std::env::temp_dir().join("songbird-convert");
            codec::convert(
                Path::new(arg(1)?),
                Path::new(arg(2)?),
                &cache,
                arg(3)?.parse()?,
                arg(4)?.parse()?,
            )?;
        }
        Some("devices") => {
            for d in audio::devices()? {
                println!("{d}");
            }
        }
        Some("voice-check") => println!("{}", voice::validate(Path::new(arg(1)?), arg(2)?)?),
        Some("recover") => Project::recover(Path::new(arg(1)?))?,
        Some(s) if s.starts_with('-') => bail!("unknown option {s}"),
        _ => {
            let root = PathBuf::from(
                args.first()
                    .map(String::as_str)
                    .unwrap_or("songbird-project"),
            );
            let p = if root.join("project.toml").exists() {
                Project::open(&root)?
            } else {
                Project::create(&root, gui::DEFAULT.into())?
            };
            gui::run(p)?;
        }
    }
    Ok(())
}

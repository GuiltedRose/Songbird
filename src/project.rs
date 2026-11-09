use crate::{codec, language};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub name: String,
    pub composition: String,
}
#[derive(Clone)]
pub struct Project {
    pub root: PathBuf,
    pub source: String,
}
pub(crate) fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap();
    let tmp = codec::unique(parent, "save", "tmp")?;
    let mut f = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path)?;
    // Windows does not permit opening directories using File::open for fsync.
    // The staged file is synced above; preserve directory fsync on Unix.
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
impl Project {
    pub fn create(root: &Path, source: String) -> Result<Self> {
        ensure!(
            !root.join("project.toml").exists(),
            "project already exists"
        );
        ensure!(
            !root.join("composition.sb").exists() && !root.join("project.previous.toml").exists(),
            "existing source or recovery state found; choose a new project directory"
        );
        fs::create_dir_all(root)?;
        for dir in ["voices", "recordings", "samples", "cache", "exports"] {
            codec::directory(root, dir)?;
        }
        let p = Self {
            root: root.canonicalize()?,
            source,
        };
        p.save()?;
        Ok(p)
    }
    pub fn open(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        for dir in ["voices", "recordings", "samples", "cache", "exports"] {
            codec::directory(&root, dir)?;
        }
        let manifest = root.join("project.toml");
        ensure!(
            fs::metadata(&manifest)?.len() <= 65536,
            "manifest exceeds 64 KiB"
        );
        let meta: Manifest = toml::from_str(&fs::read_to_string(manifest)?)?;
        ensure!(
            meta.version == 1,
            "unsupported project version {}",
            meta.version
        );
        let path = codec::safe_path(&root, &meta.composition)?;
        ensure!(
            fs::metadata(&path)?.len() <= 1_048_576,
            "composition exceeds 1 MiB"
        );
        let source = fs::read_to_string(path)?;
        language::parse(&source)
            .context("project composition is invalid; original files preserved")?;
        Ok(Self { root, source })
    }
    pub fn save_draft(&self) -> Result<()> {
        ensure!(self.source.len() <= 1_048_576, "draft exceeds 1 MiB");
        atomic(&self.root.join("recovery.sb"), self.source.as_bytes())
    }
    pub fn save(&self) -> Result<()> {
        let score = language::parse(&self.source)?;
        // Immutable generations make the manifest rename the commit point for both files.
        let filename = codec::unique(&self.root, "composition", "sb")?;
        let name = filename.file_name().unwrap().to_string_lossy().into_owned();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&filename)?;
        file.write_all(self.source.as_bytes())?;
        file.sync_all()?;
        let meta = Manifest {
            version: 1,
            name: score.name,
            composition: name,
        };
        let current = self.root.join("project.toml");
        if current.exists() {
            atomic(
                &self.root.join("project.previous.toml"),
                &fs::read(&current)?,
            )?;
        }
        atomic(&current, toml::to_string_pretty(&meta)?.as_bytes())?;
        atomic(&self.root.join("composition.sb"), self.source.as_bytes())?;
        Ok(())
    }
    pub fn recover(root: &Path) -> Result<()> {
        let previous = root.join("project.previous.toml");
        let text = fs::read_to_string(&previous)?;
        let meta: Manifest = toml::from_str(&text)?;
        ensure!(meta.version == 1, "unsupported recovery version");
        codec::safe_path(root, &meta.composition)?;
        atomic(&root.join("project.toml"), text.as_bytes())
    }
    pub fn save_as(&self, target: &Path) -> Result<Self> {
        ensure!(!target.exists(), "Save As target must not exist");
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        ensure!(
            !parent
                .canonicalize()?
                .starts_with(self.root.canonicalize()?),
            "Save As must be outside the original project"
        );
        let mut score = language::parse(&self.source)?;
        crate::studio::apply_presets(&self.root, &mut score)?;
        fs::create_dir_all(target)?;
        let copy = |name: &str| -> Result<()> {
            let src = codec::safe_path(&self.root, name)?;
            let dest = target.join(name);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            if !dest.exists() {
                fs::copy(src, dest)?;
            }
            Ok(())
        };
        for t in &score.tracks {
            if let Some(preset) = &t.vocal_preset {
                copy(preset)?;
            }
            for c in &t.clips {
                copy(&c.path)?;
            }
            for path in t.drums.values() {
                copy(path)?;
            }
        }
        for v in score.voices.values() {
            let name = format!("{}/voicebank.toml", v.bank);
            copy(&name)?;
            let bank: crate::voice::Bank =
                toml::from_str(&fs::read_to_string(codec::safe_path(&self.root, &name)?)?)?;
            for s in bank.samples {
                copy(&format!("{}/{}", v.bank, s.file))?;
            }
        }
        // Include alternate and unused takes, not only assets referenced by the score.
        if self.root.join("language.toml").is_file() {
            copy("language.toml")?;
        }
        let mut stack = vec![
            "languages".to_owned(),
            "recordings".to_owned(),
            "samples".to_owned(),
            "voices".to_owned(),
        ];
        stack.extend(score.voices.values().map(|v| v.bank.clone()));
        let mut visited = std::collections::BTreeSet::new();
        let mut count = 0;
        while let Some(relative) = stack.pop() {
            if !visited.insert(relative.clone()) {
                continue;
            }
            let path = self.root.join(&relative);
            if !path.exists() {
                continue;
            }
            let meta = fs::symlink_metadata(&path)?;
            ensure!(
                !meta.file_type().is_symlink(),
                "Save As refuses symlinked asset trees: {relative}"
            );
            if meta.is_dir() {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    count += 1;
                    ensure!(count <= 100000, "project tree exceeds 100000 entries");
                    stack.push(format!(
                        "{relative}/{}",
                        entry
                            .file_name()
                            .to_str()
                            .context("asset filename must be UTF-8")?
                    ));
                }
            } else if meta.is_file() {
                copy(&relative)?;
            }
        }
        Self::create(target, self.source.clone())
    }
    pub fn import(&self, path: &Path) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        ensure!(
            ["wav", "flac"].contains(&extension.as_str()),
            "import expects WAV or FLAC"
        );
        let _validated = codec::Asset::decode(path, &self.root.join("cache"))?;
        let target = codec::unique(&self.root.join("samples"), "import", &extension)?;
        let mut dest = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        std::io::copy(&mut File::open(path)?, &mut dest)?;
        dest.sync_all()?;
        Ok(codec::script_path(target.strip_prefix(&self.root)?))
    }
}
#[derive(Default)]
pub struct History {
    past: Vec<String>,
    future: Vec<String>,
}
impl History {
    pub fn checkpoint(&mut self, source: &str) {
        if self.past.last().is_none_or(|s| s != source) {
            self.past.push(source.into());
            if self.past.len() > 100 {
                self.past.remove(0);
            }
            self.future.clear();
        }
    }
    pub fn undo(&mut self, current: &str) -> Option<String> {
        let p = self.past.pop()?;
        self.future.push(current.into());
        Some(p)
    }
    pub fn redo(&mut self, current: &str) -> Option<String> {
        let p = self.future.pop()?;
        self.past.push(current.into());
        Some(p)
    }
}

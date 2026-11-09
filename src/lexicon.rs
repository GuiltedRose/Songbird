//! Bounded, lazy TSV dictionaries: selected resources only, no startup dictionary loading.
use anyhow::{Context, Result, ensure};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
#[derive(Clone, Default, Debug)]
pub struct Lexicon {
    pub base: Option<PathBuf>,
    cache: RefCell<BTreeMap<String, Option<String>>>,
}
impl Lexicon {
    pub fn lookup(&self, file: &str, word: &str, sensitive: bool) -> Result<Option<String>> {
        let key = if sensitive {
            word.into()
        } else {
            word.to_lowercase()
        };
        if let Some(value) = self.cache.borrow().get(&key) {
            return Ok(value.clone());
        }
        let base = self
            .base
            .as_deref()
            .context("external dictionary has no resource directory")?;
        let path = crate::codec::safe_path(base, file)?;
        let f = File::open(path)?;
        ensure!(
            f.metadata()?.len() <= 128 * 1024 * 1024,
            "dictionary limit is 128 MiB"
        );
        let mut reader = BufReader::new(f);
        let mut found = None;
        let mut lines = 0;
        loop {
            let mut row = String::new();
            let n = reader.by_ref().take(8193).read_line(&mut row)?;
            if n == 0 {
                break;
            }
            ensure!(n <= 8192, "dictionary row exceeds 8192 bytes");
            lines += 1;
            ensure!(lines <= 2_000_000, "dictionary row limit exceeded");
            let row = row.trim_end_matches(['\r', '\n']);
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            let (entry, pron) = row
                .split_once('\t')
                .context("dictionary requires WORD<TAB>PRONUNCIATION")?;
            let entry = if sensitive {
                entry.into()
            } else {
                entry.to_lowercase()
            };
            if entry == key {
                if let Some(old) = &found {
                    ensure!(
                        old == pron,
                        "ambiguous dictionary pronunciation for {word}; add an explicit voicebank dictionary override"
                    );
                }
                found = Some(pron.to_owned());
            }
        }
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= 4096 {
            cache.clear();
        }
        cache.insert(key, found.clone());
        Ok(found)
    }
}
pub fn copy_into(source_base: &Path, file: &str, destination: &Path) -> Result<String> {
    let source = crate::codec::safe_path(source_base, file)?;
    ensure!(
        source.metadata()?.len() <= 128 * 1024 * 1024,
        "dictionary exceeds limit"
    );
    if source.parent() == Some(destination) {
        return Ok(source.file_name().unwrap().to_string_lossy().into());
    }
    let target = crate::codec::unique(destination, "lexicon", "tsv")?;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)?;
    std::io::copy(&mut File::open(source)?, &mut out)?;
    out.sync_all()?;
    Ok(target.file_name().unwrap().to_string_lossy().into())
}

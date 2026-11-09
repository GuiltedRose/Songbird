use songbird::{codec, locale, phonetics, procedural};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = codec::unique(&std::env::temp_dir(), "songbird-languages", "dir").unwrap();
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn selection(tag: &str) -> phonetics::Selection {
    phonetics::Selection {
        language: Some(tag.into()),
        ..Default::default()
    }
}

#[test]
fn locale_normalization_and_precedence() {
    assert_eq!(locale::normalize("de_DE.UTF-8").unwrap(), "de-DE");
    assert_eq!(locale::normalize("C.UTF-8").unwrap(), "en");
    assert_eq!(locale::normalize("sr_RS@latin").unwrap(), "sr-Latn-RS");
    assert_eq!(
        locale::linux_locale(Some("fr_FR"), Some("de"), Some("en")),
        "fr-FR"
    );
    assert!(locale::normalize("../../outside").is_err());
    assert_eq!(
        locale::Catalog::load("la", None).unwrap().text("Play"),
        "Play"
    );
}

#[test]
fn inventories_and_punctuation_do_not_add_syllables() {
    let f = Fixture::new();
    for (tag, word) in [
        ("de", "hallo"),
        ("fr", "bonjour"),
        ("ja", "こんにちは"),
        ("la", "salve"),
    ] {
        let context = phonetics::ContextProfile::load(&f.0, &selection(tag)).unwrap();
        let plain = context.pronounce(word, &BTreeMap::new()).unwrap();
        let marked = context
            .pronounce(&format!("{word}！"), &BTreeMap::new())
            .unwrap();
        assert_eq!(plain.syllables.len(), marked.syllables.len());
        assert_eq!(marked.phrase_marks.len(), 1);
        assert!(!plain.syllables.is_empty());
        for syllable in plain.syllables {
            for phone in syllable.phones {
                assert!(context.inventory.phone(&phone).is_some());
            }
        }
    }
    assert!(phonetics::ContextProfile::load(&f.0, &selection("zz-unavailable")).is_err());
    assert_eq!(
        phonetics::ContextProfile::legacy(&f.0).unwrap().language,
        "en"
    );
}

#[test]
fn resource_profile_and_lazy_dictionary_survive_copy() {
    let source = Fixture::new();
    let destination = Fixture::new();
    fs::create_dir_all(source.0.join("languages")).unwrap();
    let mut profile = phonetics::Profile::load(&source.0, "la").unwrap();
    profile.language = "xx".into();
    profile.dictionary.clear();
    profile.dictionary_file = Some("words.tsv".into());
    profile.lexicon.base = Some(source.0.join("languages"));
    fs::write(
        source.0.join("languages/words.tsv"),
        "example\ts a / l w eː\n",
    )
    .unwrap();
    phonetics::save_profile(&destination.0, &profile).unwrap();
    let loaded = phonetics::ContextProfile::load(&destination.0, &selection("xx")).unwrap();
    assert_eq!(
        loaded
            .pronounce("example", &BTreeMap::new())
            .unwrap()
            .syllables
            .len(),
        2
    );
    profile.dictionary_file = Some("../outside.tsv".into());
    assert!(profile.validate().is_err());
}

#[test]
fn procedural_bank_has_provenance_and_never_overwrites() {
    let f = Fixture::new();
    let path = procedural::default_bank(&f.0, &selection("la")).unwrap();
    let manifest = f.0.join(&path).join("voicebank.toml");
    let before = fs::read(&manifest).unwrap();
    let bank: songbird::voice::Bank =
        toml::from_str(std::str::from_utf8(&before).unwrap()).unwrap();
    assert_eq!(bank.generation.as_deref(), Some(procedural::MODEL));
    assert_eq!(bank.language.as_deref(), Some("la"));
    assert!(!bank.samples.is_empty());
    assert_eq!(
        procedural::default_bank(&f.0, &selection("la")).unwrap(),
        path
    );
    assert_eq!(fs::read(&manifest).unwrap(), before);
    assert!(procedural::generate(&f.0, &path, "replacement", &selection("la")).is_err());
}

#[test]
fn english_stress_and_manual_melisma_remain_distinct() {
    let f = Fixture::new();
    let context = phonetics::ContextProfile::legacy(&f.0).unwrap();
    let spoken = context.pronounce("hello", &BTreeMap::new()).unwrap();
    assert!(spoken.syllables.iter().any(|s| s.stress == Some(1)));
    let manual = context.pronounce("@M.IY1 ~!", &BTreeMap::new()).unwrap();
    assert_eq!(manual.syllables.len(), 2);
    assert_eq!(manual.syllables[0].stress, Some(1));
    assert_eq!(manual.syllables[1].phones, vec!["~"]);
    assert_eq!(manual.phrase_marks.len(), 1);
}

# Languages and language packs

Open `help language` in Songbird for this offline guide. Each bundled language has
an English-written guide: [English](languages/english.md), [German](languages/german.md),
[French](languages/french.md), [Japanese](languages/japanese.md), and
[Latin](languages/latin.md). Latin is voicebank-only. UI catalogs currently cover
English, German, French and Japanese; missing translations fall back to English.
Command names and score syntax stay the same in every interface language.

## Choose settings

In the command console:

```text
language list
language interface auto
language project de
language default de
language inspect de
language pronounce de "hallo welt"
voicebank default de
help de
```

`interface` changes UI preference; `project` saves project defaults in
`language.toml`; `default` selects the language for newly created banks. Existing
banks retain their inventories. `voicebank default` creates or selects an original
procedural bank. `voicebank generate voices/my-synth MySynth la` makes a separate
Latin synthetic bank. Generated audio is labeled procedural in bank metadata and
is not a human recording. Profiles need explicit synthesis recipes for this feature.

A voice declaration can use `bank "default"` or `bank "default:la"` to generate a
bank on preparation. Source `language`, `inventory`, and `provider` settings may
also be given at score, voice and track level; see the command reference. Inventory
selection names a profile, whose `inventory` field identifies the actual phone
system. Combining incompatible physical sample inventories is rejected.

Legacy banks with no language metadata stay English regardless of OS language.
New-bank automatic selection chooses an installed native-language profile, then
English. Linux uses LC_ALL, LC_MESSAGES, LANG in that order; Windows uses preferred
UI languages; macOS uses preferred languages. Regional tags try their base language.
A requested missing singing profile produces an error rather than English lyrics.

## Config and reload

Add to your normal `config.toml` (locations are in the main README):

```toml
[language]
interface = "auto"
project_default = "auto"
pronunciation_default = "auto"
# resources = "/absolute/path/to/resources"
# font_file = "/absolute/path/to/UnicodeFont.ttf"

[voicebank]
default_language = "auto"
default_inventory = "auto"
```

By default, user resources live beside config.toml in `resources/`. Catalogs use
`locales/TAG.toml`; pronunciation profiles use `languages/TAG.toml`. Project-local
`languages/` takes precedence for pronunciation and travels with Save As. Banks
embed their own profiles. Language preferences and appearance reload with config;
`language reload` refreshes UI resources. Re-evaluate after changing pronunciation
resources. A system CJK font is used when available; install one or configure
`font_file` if Japanese glyphs are missing. Songbird does not download fonts.

## Add another language without changing Rust

Start with a bundled file in `resources/languages/`, or create a project profile:

```text
language new es Spanish
language phone es a vowel voiced "Open vowel" "Hold a"
language phone-formants es a 730 1090 2440
language dictionary es hola "a"
language pronounce es hola
```

The example demonstrates authoring, not Spanish pronunciation. Add the complete
phone inventory and correct mappings before use. `language alias TAG ALIAS PHONE`
adds compatible spellings; `language rule TAG TEXT "PHONES"` adds a literal rule.
Use `language set TAG provider dictionary|rules|manual|cmudict` (choose one value);
CMUdict is English-only. Profile fields include licensing, attribution, capabilities,
tokenization, syllabification, timing and syllabic consonants. Manual lyrics such
as `@a` bypass spelling lookup; `~` extends a sung vowel over another note.

For larger dictionaries, use UTF-8 `WORD<TAB>PRONUNCIATION` TSV:

```text
language dictionary-file es /absolute/path/words.tsv phones
```

Use `ipa` instead of `phones` for IPA strings. Phone-mode spaces separate phones;
`/` separates syllables. IPA-mode uses matching inventory symbols/aliases, stress
marks and explicit syllable dots. Ambiguous alternatives need an explicit override.
Only the selected dictionary is scanned lazily, with a bounded cache. Limits are
128 MiB per TSV, 8192 bytes per row, two million rows; profile TOML is limited to
1 MiB. External dictionaries are copied into project/bank resources for portability.
No online lookup or automatic resource download occurs.

Distribute `languages/*.toml`, referenced TSV files, and optional `locales/*.toml`
as a resource directory. `language install /absolute/path/pack` validates and
copies it to user resources without overwriting existing files. Review dataset
licenses and attribution before distribution. Custom profiles do not require a
custom UI catalog. Built-in help pages remain English; custom language authors
should supply English documentation alongside their pack.

## Punctuation and validation

Punctuation records phrase boundaries without adding syllables or rests. Place
rests explicitly in notes. Japanese dictionary matching is not a general tokenizer;
spellings absent from a pack require a dictionary entry or manual phonemes.
See [research and design decisions](LANGUAGE_RESEARCH.md) for provider alternatives
and the limits of punctuation handling.

The multilingual/procedural regression tests now pass in the Linux development
workspace; the later mini-notation pass repaired the Latin inventory identifier
validation. No human banks, listening fixtures or physical-device validation were
available. From the repository root, with the Rust toolchain and platform prerequisites
in [platform support](PLATFORMS.md) installed, run:

```sh
cargo fmt --check
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
cargo run --offline --release
```

If dependencies are not cached, run `cargo fetch` first. In the app, create a throwaway
project and try `voicebank default` for each bundled tag, coverage, preview, save,
reopen and export. Check locale selection on each native OS. Listen for transitions,
syllable alignment and Unicode rendering. Tests do not establish naturalness.
No service activation is required. Back up projects/config before replacing an
installed executable; rollback uses the previous executable and that backup. Banks
and packs are ordinary project/user files; keep generated banks if scores reference them.

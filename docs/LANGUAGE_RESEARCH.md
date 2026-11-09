# Pronunciation and punctuation research

Research reviewed November 9, 2026. These are design findings, not listening-test results.

## Pronunciation resources

OS language preferences identify a preferred language; they do not provide a
singing inventory, dictionary, or voicebank. Songbird discovers installed TOML
profiles instead of adding a Rust branch for each language. Each profile declares
its inventory, provider, aliases, syllabification and optional synthesis recipes.
Unknown vocal languages report missing resources rather than silently becoming English.

[ipa-dict](https://github.com/open-dict-data/ipa-dict) supplies multilingual IPA
wordlists. Its source datasets have individual licenses: review each source before
redistribution. Songbird can attach a local TSV through `language dictionary-file`;
IPA symbols must match the selected inventory or its aliases. Multiple conflicting
pronunciations require an explicit dictionary override. This is a resource adapter,
not a claim that every IPA wordlist already matches a singing inventory.

[eSpeak NG](https://github.com/espeak-ng/espeak-ng/blob/master/docs/languages.md)
is an alternative for broader grapheme-to-phoneme coverage. Its
[dictionary documentation](https://github.com/espeak-ng/espeak-ng/blob/master/docs/dictionary.md)
describes language-specific rules and dictionaries. A future optional adapter could
enumerate an installed engine, translate its phones, and retain explicit overrides.
It is not bundled, invoked, or downloaded by this implementation. Engine availability,
licenses, dialect mapping, subprocess failure and syllabification would need explicit
handling before such an adapter shipped. Speech output is not a singing voicebank.

Current providers are embedded English CMUdict, local dictionaries, literal
longest-match rules, and manual phonemes. Starter non-English dictionaries are
small authored examples, not complete language support. Procedural defaults provide
original synthetic samples for integration checks, not native-speaker pronunciation
or human singing quality. New languages need resource files but no Rust enumeration.

## Punctuation and phrasing

[Unicode UAX #29](https://www.unicode.org/reports/tr29/) distinguishes text boundaries
and allows language-specific tailoring. Japanese text must not be treated as one
syllable per Unicode character. Songbird's dictionary mode performs longest known
word matching; it is not a morphological analyzer or a full UAX #29 implementation.
Unknown words produce diagnostics and can be supplied explicitly.

[W3C SSML](https://www.w3.org/TR/speech-synthesis/) separates phoneme pronunciation,
breaks, and prosody. The corresponding Songbird decision is to keep written phrase
marks separate from the sung syllable sequence. Commas, sentence marks and Japanese
punctuation can be retained as phrase metadata. They never create extra sung notes
or automatic rests. Score rests and durations remain authoritative. Internal
apostrophes are preserved and curly apostrophes normalized for dictionary lookup;
quotation-edge apostrophes are stripped. Elisions and dialect spellings should use
explicit dictionary entries. Stress can be supplied in profile pronunciations;
there is no automatic punctuation-driven pitch, breath, stress or duration model.

Manual `@` phonemes and `~` melisma retain their musical meaning. Phoneme dots in
manual tokens separate phone identifiers. These choices avoid punctuation changing
a carefully authored note count. Future phrasing controls should be explicit and
separate from spelling, with native-speaker fixtures and listening checks.

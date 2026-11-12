# French voicebanks

This guide is intentionally written in English. Open it offline with `help fr`.

The bundled `fr` profile uses starter IPA dictionary. Nasal vowels have distinct identifiers. Liaison and elision are not inferred automatically.

```text
language inspect fr
language pronounce fr "bonjour monde"
voicebank default fr
```

The default command creates/selects a synthetic procedural bank. Use the Studio
coverage and preview controls to inspect it; it contains no human vocal recordings.
For your own bank, use `voicebank new voices/my-fr MyVoice fr`, follow its
inventory guide, record/import authorized takes, inspect timing and publish samples.
Do not assume an English recording inventory covers this language.

Use the exact phone IDs shown by `language inspect`; aliases can map alternate
labels. Add missing words with `language dictionary fr WORD "PHONES / PHONES"`
or attach a properly licensed TSV. Non-English starter wordlists are deliberately
small. Manual `@PHONE.PHONE` lyrics are available when spelling lookup is absent.
Written punctuation adds no notes; score rests control pauses.

See [language packs](../LANGUAGES.md) for selection precedence, resource authoring,
configuration, licensing and operator verification. Neither human pronunciation
quality nor this increment's runtime behavior has been validated. Save and reopen
a test project and listen before relying on a bank in a production song.

For exact sound IDs, IPA, examples and a recording checklist, open the
[French phonetics guide](../phonetics/fr.md).

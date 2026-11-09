# Latin voicebanks

This guide is intentionally written in English. Open it offline with `help la`.

The bundled `la` profile uses starter classical-style dictionary. Latin is available for voicebanks only. There is no Latin UI catalog. Classical-style starter choices are not an ecclesiastical pronunciation model.

```text
language inspect la
language pronounce la "salve mundi"
voicebank default la
```

The default command creates/selects a synthetic procedural bank. Use the Studio
coverage and preview controls to inspect it; it contains no human vocal recordings.
For your own bank, use `voicebank new voices/my-la MyVoice la`, follow its
inventory guide, record/import authorized takes, inspect timing and publish samples.
Do not assume an English recording inventory covers this language.

Use the exact phone IDs shown by `language inspect`; aliases can map alternate
labels. Add missing words with `language dictionary la WORD "PHONES / PHONES"`
or attach a properly licensed TSV. Non-English starter wordlists are deliberately
small. Manual `@PHONE.PHONE` lyrics are available when spelling lookup is absent.
Written punctuation adds no notes; score rests control pauses.

See [language packs](../LANGUAGES.md) for selection precedence, resource authoring,
configuration, licensing and operator verification. Neither human pronunciation
quality nor this increment's runtime behavior has been validated. Save and reopen
a test project and listen before relying on a bank in a production song.

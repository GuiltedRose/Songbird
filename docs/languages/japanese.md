# Japanese voicebanks

This guide is intentionally written in English. Open it offline with `help ja`.

The bundled `ja` profile uses starter dictionary and romanized phone IDs. Dictionary matching handles known unspaced words. Kana are not assumed to equal syllables. The inventory includes syllabic N; this starter is not a complete mora or gemination model.

```text
language inspect ja
language pronounce ja "こんにちは 世界"
voicebank default ja
```

The default command creates/selects a synthetic procedural bank. Use the Studio
coverage and preview controls to inspect it; it contains no human vocal recordings.
For your own bank, use `voicebank new voices/my-ja MyVoice ja`, follow its
inventory guide, record/import authorized takes, inspect timing and publish samples.
Do not assume an English recording inventory covers this language.

Use the exact phone IDs shown by `language inspect`; aliases can map alternate
labels. Add missing words with `language dictionary ja WORD "PHONES / PHONES"`
or attach a properly licensed TSV. Non-English starter wordlists are deliberately
small. Manual `@PHONE.PHONE` lyrics are available when spelling lookup is absent.
Written punctuation adds no notes; score rests control pauses.

See [language packs](../LANGUAGES.md) for selection precedence, resource authoring,
configuration, licensing and operator verification. Neither human pronunciation
quality nor this increment's runtime behavior has been validated. Save and reopen
a test project and listen before relying on a bank in a production song.

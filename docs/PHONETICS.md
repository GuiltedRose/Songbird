# Phonetics: which sounds should I record?

A phoneme is a speech sound, not a spelling letter. Songbird's **ID** is the code
used to label a take. **IPA** is an international notation explaining how a sound
is pronounced. These may differ: English IY is IPA [i], and Japanese r is [ɾ].
Record the sound in the highlighted part of the example, not the letter name.

## Choose the target language first

| Language | Shipped inventory | Separate recording checklist |
|---|---|---|
| English | 39 stressless ARPABET IDs | [English](phonetics/en.md) |
| German | 47 starter IPA IDs | [German](phonetics/de.md) |
| French | 37 starter IPA IDs | [French](phonetics/fr.md) |
| Japanese | 25 starter IDs | [Japanese](phonetics/ja.md) |
| Latin | 26 starter IDs; voicebanks only | [Latin](phonetics/la.md) |

These are the shipped profile inventories, not a claim that all these sounds are
already recorded or that every dialect/allophone is represented. Non-English
starter dictionaries are small. Check `language inspect TAG` and use explicit
pronunciation entries when necessary. A procedural bank produces synthesized
fixtures; it is not a complete recorded human singer. Custom profiles may change
IDs and coverage requirements. No additional language is claimed here.

Create the bank with its target language, for example:

```text
voicebank new voices/my-de "My German Voice" de
```

Use `en`, `de`, `fr`, `ja`, or `la` at creation. Open that language's checklist
before recording. Do not label an existing English bank as another language just
to suppress missing sounds. Keep dialect and pronunciation choices consistent
across its recordings; use another bank/profile for intentionally different choices.

## Recording every kind of sound

Use a quiet room, the same microphone position and gain, and a comfortable pitch
for voiced material. Bank settings specify WAV/FLAC, 16/24-bit, mono/stereo and a
supported device rate; 48 kHz / 24-bit / mono / FLAC is a useful starting point,
not a requirement to override your hardware. Each take must be at most 30 seconds
for Studio analysis. Short clear takes are easier to prepare than long word lists.
Leave short silence around useful audio; avoid clipping, room music and loud breaths.

- Vowels: hold the vowel quality and a steady comfortable pitch. Do not turn a
  steady vowel into an English-style glide unless the target is a diphthong.
- Diphthongs: preserve the intended movement between vowel qualities.
- Voiced consonants: retain the buzz/hum and natural transitions; record at a pitch
  compatible with nearby vowels. Do not add the vowel from the spoken letter name.
- Unvoiced friction/breath: do not force a sung pitch; uncertain tuner readings are
  normal. Verify `voiced false` rather than fabricating a root.
- Stops/affricates: retain closure, attack and release. Do not try to sustain a stop.
- Nasal vowels: preserve nasal airflow without appending a separate n or m.

Songbird names captures uniquely and stores originals with capture metadata. You
do not need to rename files to the phoneme ID. The take's selected sound label,
not the filename, determines its mapping. External files can keep their names;
import with the correct target ID and prepare the copied take. Avoid hand-editing
bank paths: sample references must stay inside the bank folder.

For each row: record → replay original → inspect levels/root/timing → replay trimmed
→ Use selected take → check coverage. The language pages include a manual checklist;
`voicebank missing` and `voicebank coverage "LYRICS" "NOTES"` inspect actual published
mappings. Passing coverage does not establish pronunciation or singing quality.
Test short phrases in your chosen language before expanding to a full song, then
save and export the bank as described in the [beginner workflow](FIRST_VOICEBANK.md).

## External audio supplements

The [official IPA interactive chart](https://www.internationalphoneticassociation.org/IPAcharts/IPA_charts_TI/IPA_charts_TI.html)
provides symbol descriptions and speaker recordings. [IPAchart.com](https://www.ipachart.com/)
is another interactive audio chart; its articulations are general examples, not
language-specific rules. [IPA Handbook recordings](https://www.internationalphoneticassociation.org/content/sound-recordings)
provide language illustrations subject to the listed availability and terms.
Links/content were checked on 2026-11-12; individual audio playback was not tested.
The older UVic IPALab page still describes obsolete QuickTime/browser requirements,
so it is not the recommended beginner playback link. Wiktionary can supplement
individual word checks but is community-edited and dialect-dependent; it is not
used as Songbird's authoritative inventory.

Links open only when clicked. Songbird never fetches these sounds automatically,
and essential pronunciation/recording instructions remain embedded offline.

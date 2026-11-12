# Japanese phonetics for Songbird

[Phonetics overview](../PHONETICS.md) · [First voicebank](../FIRST_VOICEBANK.md)

This checklist contains exactly the **25 shipped IDs** in
`resources/languages/ja.toml` (`ja-phones`). It describes recording
targets, not sounds already recorded in your bank. Custom profiles may differ;
check `language inspect ja` and Studio coverage for your actual profile.

## Variety and scope

Starter Japanese inventory, not a full narrow transcription of every allophone. sh/j/ch/f/u/r are internal ASCII labels whose IPA differs from their spelling. N is a moraic nasal with context-dependent place. Vowel devoicing, long vowels, gemination and accent require appropriate dictionary/timing choices; this list does not add unsupported geminate or accent labels. Kana examples include a following vowel to illustrate consonants: isolate the selected onset when recording a consonant sample.

## Recording checklist

Bold text identifies the relevant part of the example. Example words are cues,
not instructions to record the entire word. IPA in square brackets is a broad
pronunciation guide; the **ID** column is what Songbird commands require.
Mark your own checklist only after recording, listening, preparing and Using the
take. `voicebank missing` is the actual saved-bank coverage check.

| Done | ID | IPA | Example / target | How to form it | Recording / common mistake |
|---|---|---|---|---|---|
| [ ] | `a` | [a] | **あ** (a) | open a | Hold the vowel steadily; keep its quality instead of adding another vowel. |
| [ ] | `i` | [i] | **い** (i) | close front ee | Hold the vowel steadily; keep its quality instead of adding another vowel. |
| [ ] | `u` | [ɯᵝ] | **う** (u) | back compressed-lip vowel; not English rounded oo | Hold the vowel steadily; keep its quality instead of adding another vowel. |
| [ ] | `e` | [e] | **え** (e) | steady e; avoid an English diphthong | Hold the vowel steadily; keep its quality instead of adding another vowel. |
| [ ] | `o` | [o] | **お** (o) | steady o; avoid an English glide | Hold the vowel steadily; keep its quality instead of adding another vowel. |
| [ ] | `k` | [k] | **か** (ka) | k onset before a | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `g` | [ɡ] | **が** (ga) | voiced g onset | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `s` | [s] | **さ** (sa) | s onset | Record the friction/breath without singing a note or appending uh. |
| [ ] | `z` | [z ~ dz] | **ざ** (za) | z or affricated z according to context/variety | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `sh` | [ɕ] | **し** (shi) | palatalized hiss; not simply English sh | Record the friction/breath without singing a note or appending uh. |
| [ ] | `j` | [dʑ ~ ʑ] | **じ** (ji) | palatalized voiced affricate/friction | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `t` | [t] | **た** (ta) | t onset | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `d` | [d] | **だ** (da) | voiced d onset | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `ch` | [tɕ] | **ち** (chi) | palatalized closure into friction | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `ts` | [ts] | **つ** (tsu) | t directly into s | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `n` | [n] | **な** (na) | tongue-tip nasal | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `h` | [h] | **は** (ha) | breathy onset | Record the friction/breath without singing a note or appending uh. |
| [ ] | `f` | [ɸ] | **ふ** (fu) | friction between lips, not lower lip against teeth | Record the friction/breath without singing a note or appending uh. |
| [ ] | `b` | [b] | **ば** (ba) | voiced lip closure | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `p` | [p] | **ぱ** (pa) | voiceless lip closure | Capture closure and natural release; do not try to sustain a stop. |
| [ ] | `m` | [m] | **ま** (ma) | closed-lip nasal | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `y` | [j] | **や** (ya) | front-tongue glide | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `r` | [ɾ] | **ら** (ra) | brief tongue tap; not English r or a long trill | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `w` | [w] | **わ** (wa) | rounded/back glide | Record a short clear voiced sound; avoid adding the spoken letter name. |
| [ ] | `N` | [ɴ ~ n ~ m ~ ŋ] | ほ**ん** (hon) | moraic nasal; place changes with following sound | Record a short clear voiced sound; avoid adding the spoken letter name. |

## Reference audio and further reading

The [official IPA interactive chart](https://www.internationalphoneticassociation.org/IPAcharts/IPA_charts_TI/IPA_charts_TI.html)
provides symbol descriptions and speaker examples. The
[IPA Handbook audio index](https://www.internationalphoneticassociation.org/content/sound-recordings)
is a supplemental language-illustration resource; availability varies by item.
[IPAchart.com](https://www.ipachart.com/) offers HTML5 audio for general IPA
symbols and warns that language/context-specific realizations can differ.
These pages were checked on 2026-11-12; audio playback and every individual clip
were not tested. No external audio is bundled, automatically fetched, or treated
as permission to redistribute a recording. Essential instructions above work offline.

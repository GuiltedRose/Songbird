# Custom singing voicebanks

Songbird uses your own phoneme recordings, not text-to-speech or a remote model.
No training labels or human recordings are manufactured. The included example
voicebank is a generated analytic vowel for engine testing, not evidence of
intelligible English singing or a substitute for recording a real bank.

## Record and label

1. Record clean, dry material you own or may use. Keep originals. Use a steady
   pitch and level, avoid clipping, record consonants and vowels as well as
   natural syllables. FLAC and WAV mono/stereo PCM are supported.
2. Create a project-local bank using `voice-new "voices/nova" Nova`, or copy an
   existing Songbird bank directory into `voices/`. Bank import means copying
   the portable directory and validating it; other voicebank formats are not read.
3. Edit `voices/nova/voicebank.toml`. Label samples with stressless ARPABET symbols;
   specify seconds for onset, sustain, and end, and fundamental frequency in Hz.
   Use the same phoneme in multiple entries for registers, styles, and pitches.
4. For voiced vowels, choose a sustain point at a consistent glottal epoch and
   a stable pitch region. Root pitch is supplied by you; there is no automatic
   root-frequency or pitch-mark analyzer. The synthesis grid depends on accurate
   labels. Sustains must contain at least three root periods.
5. Run `voice-check "voices/nova"`. It checks files and timing, not singing quality.
   Audition chromatic notes and phrases across every mapped pitch/style/register.

```toml
version = 1
name = "Nova"
license = "Your license and recording attribution"

[dictionary]
NOVA = "N OW / V AH"
BEYOND = "B IH / AA N D"

[[samples]]
phoneme = "AA"
file = "samples/aa-alto-normal-A3.flac"
root = 220.0
low = 50
high = 65
onset = 0.02
sustain = 0.12
end = 0.9
voiced = true
style = "normal"
register = "alto"

[[samples]]
phoneme = "T"
file = "samples/t.wav"
root = 220.0
low = 40
high = 80
onset = 0.0
sustain = 0.005
end = 0.07
voiced = false
style = "normal"
register = "alto"
```

Files resolve relative to the bank manifest and cannot escape that bank directory.
Each style and register needs coverage for every requested phoneme and note.
A named register requires exact matching. The default expression is `normal`;
`expression soft` selects recordings labeled `soft`, not a fabricated model style.
When several matching samples cover a note, the closest root pitch is selected.
Keep pitch ranges near the recorded root; octave-scale shifts reduce quality.

## Lyrics and notes

```text
voice Nova {
    language english
    bank "voices/nova"
    register alto
}
track singer {
    use voice Nova
    lyrics "Take me beyond the stars"
    notes "C4:1 E4:1 G4:0.5 A4:0.5 G4:1 C5:2"
    expression normal
    vibrato 0.2
    breathiness 0.1
    automate bend "0=0 0.2=0.4 0.4=0"
    gain -6db
}
```

The six notes match six syllables: TAKE, ME, BE, YOND, THE, STARS. Notes count
sounding monophonic events, not rests. A chord in a singer track is an error.
Use one track per singer for simultaneous voices. `~` extends the preceding vowel
onto another note: `lyrics "me ~"; notes "C4:1 E4:2"`. More than one syllable per
note is not implicit: adjust the note sequence or provide a dictionary override.

Pronunciation priority is bank dictionary, explicit built-in examples, then the
bundled CMU dictionary. CMU stress digits are removed, and a deterministic English
onset rule splits multisyllable entries. Automatic syllabification is approximate.
Bank dictionaries use `/` for explicit syllable boundaries. `@T.EY.K` overrides a
single syllable directly in lyrics. Unknown words require a dictionary entry or
an override; missing phonemes never silently fall back to an oscillator.

Consonants receive up to 75 ms each (at most 35% of syllable duration collectively);
vowels receive the remainder. Phoneme boundaries overlap by 4 ms with short edge
fades. Consonants use duration-mapped sample playback; vowels use pitch-synchronous
overlap/add, retaining source grain shape to reduce formant movement. Sustains are
mapped across the labeled stable region. Vibrato and bend alter the synthesis
pitch grid. Breathiness adds low-level deterministic noise. Dynamics use the same
track gain/automation, effects, editing, and renderer as every other track.

This is a basic time-domain engine. It does not promise natural coarticulation,
automatic pitch analysis, professional formant correction, or intelligibility
with arbitrary isolated-phone recordings. Those require real-bank listening tests
and likely DSP refinement. No appropriately licensed human English voicebank was
provided in this session; full lyric acceptance remains outstanding.

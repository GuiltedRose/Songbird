# Voicebanks and vocal presets

New to voicebanks? Start with [Creating Your First Voicebank](FIRST_VOICEBANK.md).
It assumes no music/audio knowledge, explains the terms, walks through the GUI and
necessary commands, and builds your first “me see” from three recorded sounds.
This page is the command and file-format reference for later use. Voicebank Studio runs
inside Songbird's prompt and right-hand panel. File decoding, analysis and preview
preparation run on its background worker; audio callbacks do not perform this work.

## Studio commands

Commands below belong in the prompt. Paths/names/lyrics containing spaces need
quotes. `PATH` for a bank is project-relative, such as `voices/nova`; import/export
source/destination paths may be absolute. Open a bank before using its commands.

| Command | Operation |
|---|---|
| `voicebank help` | List Studio commands |
| `voicebank new PATH NAME [LANGUAGE|auto]` | Create a bank from an installed language profile |
| `voicebank open PATH` | Reopen a bank and its saved Studio session |
| `voicebank close` | Hide/close the Studio view after takes finish |
| `voicebank register NAME` | Set the register for future takes and coverage |
| `voicebank expression NAME` | Set the expression for future takes and coverage |
| `voicebank settings RATE BITS CHANNELS [flac\|wav]` | Bank recording/preview settings; default 48000/24/1/FLAC |
| `voicebank tempo BPM` | Preview tempo, 20–400; default 120 |
| `voicebank target PHONE` | Select an ARPABET phone or dot-separated syllable |
| `voicebank guide` | Explain the current phone(s) with reference words |
| `voicebank guide next` | Select a missing target from the 39-phone English inventory |
| `voicebank guide "LYRICS"` | Select a missing phone target for the lyrics |
| `voicebank record [PHONE]` | Start hardware capture for that target and profile |
| `record-stop` / `voicebank record-stop` | Stop capture; finalize and import/analyze in the background |
| `voicebank import FILE [PHONE]` | Copy authorized WAV/FLAC material and create an analyzed take |
| `voicebank takes` | List take numbers, labels, profiles, duration, and source paths |
| `voicebank take NUMBER` | Select a take (one-based) |
| `voicebank samples` | List published sample mappings |
| `voicebank sample NUMBER` | Select a published sample for editing, including legacy banks |
| `voicebank replay [raw]` | Audition the selected take; default uses onset/end trim |
| `voicebank analyze` | Refresh analysis, retaining manual metadata |
| `voicebank suggest` | Explicitly replace editable metadata with current suggestions |
| `voicebank trim ONSET SUSTAIN END` | Set non-destructive source times in seconds |
| `voicebank set FIELD VALUE` | Correct selected-take metadata; fields below |
| `voicebank use` | Publish the preferred take or update its sample mapping |
| `voicebank coverage ["LYRICS" ["NOTES" [PRESET]]]` | Report label/profile/range/audio issues; optionally include preset harmonies/pitch |
| `voicebank validate` / `voicebank missing` | Inspect full English phone coverage for current profile, plus file/metadata validity |
| `voicebank dictionary WORD "PHONES / PHONES"` | Add/replace an explicit pronunciation and syllable boundaries |
| `voicebank license "TEXT"` | Store recording license/attribution |
| `voicebank test "LYRICS" "NOTES" [PRESET]` | Direct singing preview, without an export |
| `voicebank presets` | List saved presets |
| `voicebank preset show NAME` | Show a preset's current parameters and effects |
| `voicebank preset new NAME` | Create a portable, editable preset in the active bank |
| `voicebank preset set NAME FIELD VALUE` | Change a preset setting |
| `voicebank preset effect NAME EFFECT ...` | Append an effect with normal Songbird effect syntax |
| `voicebank preset clear-effects NAME` | Remove the preset's effect chain |
| `voicebank save` | Save bank/session metadata |
| `voicebank export NEW_DIRECTORY` | Copy a portable bank outside its own source tree |
| `voicebank import-bank SOURCE PATH` | Validate and copy an existing bank into this project |

Use `devices`, `input "NAME"`, `output "NAME"`, and `monitor on|off` for device
selection and monitoring. Studio capture uses its own `settings` channels/rate.
Preview/replay require stopping the current transport first; `stop` ends a preview.
You can edit the bank while a preview plays, but stop before auditioning again.
Finish a bank recording before switching banks or modifying its capture profile.

### Selected-take fields

`root` is Hz (40–2000); `note` sets the same root using a note name. `low` and
`high` are note names defining the supported range. `voiced` takes `true`/`false`.
`onset`, `sustain`, `release`, `end` are source seconds. `phoneme`, `register`,
`expression` edit the take's labels without changing previously recorded audio.
Changing the Studio's next-recording target/profile does not relabel old takes.

Publishing validates `0 <= onset <= sustain < end <= duration`, an optional
release within sustain/end, finite root/range values, known labels and actual audio.
A voiced sustain must contain at least three root periods. Silent takes are rejected
for publication; clipping/quiet warnings remain visible and should be reviewed.
Unpitched material can use a neutral reference root rather than a detected F0.

### Preset fields

| Field | Values |
|---|---|
| `register`, `expression` | Named sample profiles |
| `gain` | Linear 0–4, or dB through the command (`-9db`) |
| `pan` | -1..1 |
| `vibrato`, `breathiness` | 0..1 |
| `pitch` | -24..24 semitones, including fractional offsets |
| `dynamics` | Linear 0..2 vocal amplitude scaling |
| `formant` | 0.5..2 grain-envelope adjustment ratio, independent of note pitch |
| `harmony` | Quoted list of 1–8 semitone offsets, each -24..24; e.g. `"0 7 12"` |

Preset effects use the [existing effect reference](LANGUAGE.md#effects). A maximum
of 16 effects is allowed per preset. Harmony expands to separate singer tracks;
normal project track/singer/effect limits still apply. Coverage with notes and a
preset checks each harmony's shifted pitch against its register/style mappings.

## Voicebank files and compatibility

```text
voices/nova/
    voicebank.toml             # Playable mappings and dictionary
    studio.toml                # Takes, analysis, selection, recording settings
    originals/                 # Unique imported/prepared source copies
    recordings/                # Hardware captures; FLAC retains recovery WAV
    captures/                  # Immutable capture-start receipts
    history/                   # Previous changed metadata
    preset-Soft-Harmony.toml   # Portable vocal preset
    cache/                     # Regenerable PCM; excluded from exports
```

Version 1 banks remain readable. Studio metadata is optional for existing banks;
`voicebank sample NUMBER` brings an existing mapping into the preparation view.
A new optional `release` field separates a stable vowel from its coda; old samples
without it retain their prior end-of-sustain interpretation.

Example playable entry:

```toml
version = 1
name = "Nova"
license = "Your recording attribution and license"

[dictionary]
NOVA = "N OW / V AH"

[[samples]]
phoneme = "IY"
file = "originals/your-take.flac"
root = 220.0
low = 54
high = 60
onset = 0.1
sustain = 0.2
release = 0.9
end = 1.0
voiced = true
style = "normal"
register = "alto"
```

Sample paths stay inside the bank. Repeated phoneme entries may cover additional
roots, styles or registers. Selection uses matching labels/profile/range and the
closest root. A complete compatible syllable label (`M.IY`) is preferred over its
separate phones. Missing phones are reported, never silently replaced by unrelated
ones. Original takes and metadata history remain when a mapping is replaced.

Example preset:

```toml
version = 1
name = "Soft Harmony"
bank = "."
register = "alto"
expression = "normal"
gain = 0.35
pan = -0.15
vibrato = 0.15
breathiness = 0.2
pitch = 0.0
dynamics = 1.0
formant = 1.0
harmonies = [0, 7]
[[effects]]
kind = "reverb"
args = [0.2]
```

`bank` is relative to the preset file's directory; the Studio writes `.` for its
own bank. Use `vocal-preset "voices/nova/preset-Soft-Harmony.toml"` inside a track.
The language also accepts direct singer-track `formant`, `dynamics`, `vocal-pitch`,
and `harmony "0 7"` settings. Explicit track settings override preset controls;
track effects follow preset effects. Presets introduce their own voice reference,
so a separate `voice` block is unnecessary for such a track.

## Analysis and synthesis behavior

Analysis is local and bounded: Studio handles samples up to 30 seconds and at
most 5,760,000 decoded interleaved samples. It measures clipping/active RMS,
estimates silence boundaries, uses YIN lag differences for pitch, and suggests
voicing/root and a stable voiced region with a pitch-epoch starting point.
It analyzes at most 64 pitch windows at up to 12 kHz. Results have confidence and
warnings. It does not recognize arbitrary phonemes, guarantee a vowel boundary,
or certify pitch range, human identity, intelligibility or recording rights.

The singer uses pitch-synchronous vowel overlap/add with an integrated pitch grid,
short boundary crossfades, retained attacks/codas, explicit root mappings, vibrato,
bends, breathiness and dynamics. Consonants retain their local source spectrum
rather than compressing a long take into a few milliseconds; stops are not looped.
Formant ratios adjust grain shape while leaving the assigned pitch grid separate.
These are lightweight DSP methods; extreme shifts, inaccurate labels, transitions
and changing source pitch still need listening and correction.

All bank edits are metadata or new files. Changed manifests keep prior snapshots;
originals are never silently overwritten. A capture-start receipt preserves its
chosen target/profile and WAV path even if completion is interrupted. A receipt
is not proof of a completed/validated recording. Recover its WAV with Songbird's
[recovery workflow](OPERATIONS.md#project-safety-and-recovery), then import it.
Manifests/session saves are separately atomic; an interrupted multi-file save can
require selecting a previous metadata snapshot from `history/`. Edit a bank in one Songbird instance at a time; atomic file replacement is not
a concurrent multi-writer transaction. Keep external backups of valuable recordings. Export/import uses create-new destinations and
rejects escaping paths/symlinked trees.

The bundled [multi-label example](../examples/voicebank-studio/README.md) contains
original generated audio. It exercises the workflow; it is not a full human
English bank. Real-device recording/playback and intelligible human singing
require actual authorized samples and hardware listening tests.

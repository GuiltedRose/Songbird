# Create your first voicebank

Build a singer from your own recordings, prepare its sounds in Voicebank Studio,
then use it in a song. All commands below go in **Songbird's command prompt**:
press Ctrl+L, type a command, and press Enter. Composition examples go in the
main editor (Ctrl+E). Nothing requires a separate voice editor or internet service.

A **voicebank** stores recorded sounds and their pitch/timing labels. A **vocal
preset** chooses a bank and adds performance settings such as harmony, expression,
and effects. Reuse the bank with several presets without recording it again.

## 1. Create a bank

Open your project and enter:

```text
voicebank new voices/nova "Nova" english
voicebank register alto
voicebank expression normal
voicebank settings 48000 24 1 flac
```

The path is relative to the project. Choose a new path; existing banks are never
overwritten by `new`. This creates `voices/nova/voicebank.toml`, a `studio.toml`
session, and folders for originals/history as needed. Studio appears in the right
panel. FLAC is the default capture format; WAV is also supported.

For a bank already in the project, use `voicebank open voices/nova`. It reopens
your takes, selected target, recording settings, and preparation metadata.

## 2. Prepare the microphone

```text
devices
input "EXACT INPUT DEVICE NAME"
output "EXACT OUTPUT DEVICE NAME"
monitor off
```

Use the names printed by `devices`. Record in a quiet room, use headphones when
playing reference music, and avoid clipping. If your device needs another rate,
change `voicebank settings`; for example `44100 24 1 flac`.
Set `channels 2` only for general recording; bank capture uses the channel count
in `voicebank settings`. Set monitoring before starting a take. See
[device setup](OPERATIONS.md#device-and-recording-acceptance) for hardware details.

## 3. Record a useful first word's sounds

Start with **“me”**, whose phones are M and IY. Phones use stressless ARPABET labels:

```text
voicebank guide "me"
voicebank target IY
voicebank guide
voicebank record
```

The panel shows IY, the sound “ee” in **see**, and its pronunciation reference.
Sing a steady, comfortable “ee” for about a second, leaving a little quiet on
either side. Then enter:

```text
record-stop
```

Wait for finalization and analysis to finish. The waveform and suggested metadata
appear in Studio. The recording is real input capture; a new, uniquely named
take is created each time. Record another take with `voicebank record IY` if
needed. For a natural stop consonant, capture its release rather than trying to
hold it like a vowel. Individual takes/imports for Studio may be up to 30 seconds.

## 4. Inspect, audition, and choose a take

```text
voicebank takes
voicebank take 1
voicebank replay raw
stop
voicebank replay
stop
```

Take numbers are one-based and local to the bank. Raw replay plays the original;
normal replay uses the current onset/end trim. The panel displays the source
waveform, peak, active-region RMS, clipping count, and onset/sustain/release/end
markers. Meter feedback is available while recording.

Quiet/clipping warnings describe the original take. Prefer recording again with
better hardware levels to trying to repair distortion. Analysis never identifies
the phoneme: **you supply the target**. Every suggestion remains editable.

## 5. Correct pitch and timing

Use the values shown in your own waveform; these numbers are illustrative:

```text
voicebank trim 0.10 0.20 1.10
voicebank set release 1.00
voicebank set note A3
voicebank set low G3
voicebank set high C4
voicebank set voiced true
voicebank use
```

Only use timings inside your recording. The fields mean:

| Field | Meaning |
|---|---|
| onset | First included source sound, in seconds |
| sustain | Start of stable voiced material, ideally a consistent pitch epoch |
| release | End of stable material / start of the coda; optional |
| end | Last included source time |
| root / note | Recorded root frequency in Hz, or a note such as A3 |
| low / high | Lowest/highest MIDI note names this sample may serve |
| voiced | Whether this material has a pitched, periodic source |

For a voiced sample, the stable region must contain at least three root periods.
Keep pitch mappings near the recorded note; record additional roots for a wider
range. Fricatives such as S/SH are normally unvoiced: `voicebank set voiced false`.
An unvoiced sample need not have a measured fundamental; the engine stores a
neutral pitch reference when necessary, not a detected pitch claim.

`voicebank use` publishes the selected take into the bank's playable mappings.
Changing a published take and using it again updates that mapping. Selecting
another take for the same label/register/expression/range replaces the preferred
mapping while preserving both takes and their originals. Changes are metadata;
source recordings are not trimmed or overwritten.

To rerun analysis, use `voicebank analyze`. It refreshes the report without
changing manual values. `voicebank suggest` explicitly copies the latest
suggestions into the selected take. Verify them before publishing again.

## 6. Add consonants and more vowels

```text
voicebank target M
voicebank guide
voicebank record
record-stop
```

Perform a short closed-lip “mmm” at the same reference pitch as your IY take. Inspect/replay it, correct the voiced/root/range
metadata, and publish it with `voicebank use`. Now you have the sounds for “me.”
Repeat for S (“sss”) to prepare “see.” Use `voicebank guide "me see"` to select
an unfilled phone target for that phrase. For the full inventory,
`voicebank guide next` advances to a missing label in the current register/style.
Each target includes a description and reference word in the panel.

You can also record a whole syllable: `voicebank record M.IY`. Label its sustain
on the vowel and its release before the coda. When the complete syllable has a
compatible sample, synthesis uses it in preference to separate phones. Otherwise
it uses the individual phones; it does not substitute an unrelated sound.

For an existing authorized recording:

```text
voicebank import "/absolute/path/to/ee.wav" IY
```

Windows prompt paths can use `C:/Users/you/Music/ee.wav`. Studio copies the original
and analyzes it. Select and prepare that take as usual. For long source material,
use Songbird's audio-track trim/export workflow to make a short preparation copy;
keep the original source project. No external DAW is needed.

## 7. Check coverage before singing

```text
voicebank coverage "me see" "A3:1 B3:1"
voicebank validate
voicebank missing
```

The sample notes assume your takes cover A3 and B3; use notes inside your own
recorded ranges if you chose another starting pitch.

Coverage reports required/available/missing phones, register/style mismatches,
out-of-range notes, dictionary ambiguities, and invalid audio/metadata. A lyric
coverage result applies to those lyrics, notes and settings—it is not certification
of a full English bank or proof of vocal quality. `validate`/`missing` without
lyrics check the 39-phone inventory for the active register/expression.

To correct pronunciation, add an explicit dictionary entry:

```text
voicebank dictionary MEE "M IY"
voicebank dictionary NOVA "N OW / V AH"
```

Spaces separate phones; `/` separates syllables. For a one-off lyric override,
use `@M.IY` instead of the word. Each syllable needs a vowel. Songbird uses the
bank dictionary before its built-in English pronunciations and bundled CMUdict.

## 8. Preview singing and correct problems

Stop song playback first, then:

```text
voicebank test "me see" "A3:1 B3:1"
```

This auditions the phrase through the song renderer and selected output device;
you do not have to export a song. Enter `stop`, correct the sample or dictionary,
and run the preview again. Try `voicebank test "me ~" "A3:1 C4:1"` for a melisma:
`~` continues the preceding vowel onto another note.

If preview fails, follow its missing-phone/range diagnostic and run coverage.
If consonants are unclear, compare the raw take with the prepared trim; avoid
cutting off the release. If the wrong syllable boundary is used, add a dictionary
override. For unstable pitch, inspect the suggested root and stable region, record
a steadier sample, or reduce the interval from its recorded pitch. For noisy or
unintelligible audio, re-record—metadata validation cannot establish intelligibility.

## 9. Save and reuse the bank

```text
voicebank license "Your recording license and attribution"
voicebank save
voicebank export "/existing/library/Nova"
```

Choose a new destination whose parent already exists. The export includes original
takes, bank/studio metadata, presets, dictionaries, capture receipts, and history;
regenerable cache is omitted. Preparation operations save automatically too.

In another Songbird project:

```text
voicebank import-bank "/existing/library/Nova" voices/nova
voicebank open voices/nova
```

No bank format conversion or external application is needed. If opening an older
Songbird bank with no Studio session, use `voicebank samples` and then
`voicebank sample 1` to edit a published sample in Studio.

## 10. Make a backing-vocal preset

```text
voicebank preset new "Soft Harmony"
voicebank preset set "Soft Harmony" gain -9db
voicebank preset set "Soft Harmony" vibrato 0.15
voicebank preset set "Soft Harmony" breathiness 0.2
voicebank preset set "Soft Harmony" harmony "0 7"
voicebank preset effect "Soft Harmony" reverb 0.2
voicebank coverage "me see" "A3:1 B3:1" "Soft Harmony"
voicebank test "me see" "A3:1 B3:1" "Soft Harmony"
```

Harmony offsets are semitones: `0 7` sings the written notes plus a fifth above.
Your bank must cover **both** lines. If the upper line is out of range, record
another root/register or change the harmony. Presets can also set `register`,
`expression`, `pan`, `pitch`, `dynamics`, and `formant`. An expression such as
`soft` selects samples actually labeled soft; it is not an invented replacement
for missing recordings.

Use `voicebank presets` to list your presets and `voicebank preset show "Soft Harmony"`
to inspect its settings. The console prints the portable preset path. Add it to a song's editor:

```text
track backing {
    vocal-preset "voices/nova/preset-Soft-Harmony.toml"
    lyrics "me see"
    notes "A3:1 B3:1"
    at 16b
}
```

A preset references its bank relative to the preset file. Copy/export the bank
together with its presets to reuse them. Explicit track controls such as
`gain -12db`, `vibrato 0`, or `formant 0.95` override preset values; track effects
follow preset effects. Press Ctrl+Enter to evaluate, Ctrl+P to play alongside your
music, and Ctrl+S to save. Export FLAC/WAV normally when the arrangement is ready.

See the [command and format reference](VOICEBANKS.md) for every Studio command.
The [included engineering example](../examples/voicebank-studio/README.md) lets
you explore the interface with generated M/IY/S material. It is not a human singer
or evidence of complete, intelligible English singing. Final listening and
microphone/device acceptance require your own authorized recordings and hardware.

## Live pitch coach and voice practice

Open/create a bank and show the Tracks panel to see **Pitch coach**. **Live tuner
while recording** enables analysis for the next bank capture. Click **Record bank
target** (or use `voicebank record`), then **Stop bank recording** when finished.
No microphone is opened by viewing the coach or starting Songbird. Monitoring
remains independently controlled and off by default. Practice takes are real bank
recordings: they are preserved and analyzed through the normal take workflow.
Keep bank takes within the existing 30-second analysis limit.

The coach displays the detected note, frequency in Hz, cents flat/sharp relative
to the nearest equal-tempered note, input RMS/peak levels, clipping, and tracking
periodicity. Turn on **Use a target note** to choose your own reference instead
(A4 = 440 Hz; targets E1–B6). Within ±10 cents is displayed as “In tune”; this is
an informational display tolerance, not an automatic correction. Choose targets
that are comfortable for you. No target or score assigns a gender to your voice.
Pitch is one aspect of voice; this first version does not measure resonance,
formants, gender, vocal health, or pronunciation.

A short trace shows up to 60 measurements, nominally 12 seconds. The graph spans
±100 cents around your target, or the current nearest note when no target is
selected. Values beyond that span reach the edge; the numerical cents readout
shows the full difference. Gaps indicate missing/uncertain tracking. **Recent
pitch spread** is the standard deviation in cents over up to ten consecutive
confident measurements (about two seconds); it appears after four measurements.
Intended note changes and vibrato increase this number too—it is not a quality
score. Cents are hundredths of a semitone; 100 cents is one semitone.

Silence, low-level input, unvoiced consonants, clipping, and uncertain periodicity
withhold the note rather than guessing. Notes require periodicity ≥85% and
AC RMS ≥0.003 (roughly -50 dBFS); these are detector thresholds, not calibrated
probabilities. Harmonics, room noise, multiple voices, and octave errors can still
mislead pitch detection. The detector covers approximately 40–2000 Hz and mono
analysis averages stereo channels. Readings expire after 750 ms without a fresh
result. Numeric measurements update about five times per second.

After finalization, review the selected take's **Measured root**. **Apply measured
root** explicitly copies a sufficiently confident offline estimate into that
take's root frequency, preserving its audio, trims, range, and existing published
samples. Review range/trim metadata and use `voicebank use` separately to update
the playable bank. `voicebank analyze` refreshes suggestions without overwriting
manual metadata; `voicebank set note A3` or `voicebank set root 220` remains
available for manual corrections. The live target never changes recorded pitch
or writes root metadata. Existing bank/import defaults remain compatible.

All analysis is local. The audio callback performs only a nonblocking sample
handoff to a bounded side queue. A dedicated worker downsamples to at most 8 kHz,
analyzes a 1024-sample window using the existing YIN detector, and publishes a
bounded history. GUI reads do not block that worker. Overloaded analysis drops
only tuner samples and resets continuity; recording-buffer overruns retain their
existing error handling. There are no new network services or dependencies.

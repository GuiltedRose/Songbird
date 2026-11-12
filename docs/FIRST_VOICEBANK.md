# Create your first voicebank

You do not need to know music theory, phonetic symbols, or audio engineering.
This guide starts with your microphone and ends with your own voice singing
**“me see.”** You will record just three sounds first, not an entire language.

A **voicebank** is a folder of short voice recordings plus instructions telling
Songbird which sound each recording contains and how to play it at different
notes. You supply the recordings. Songbird joins and changes their pitch to sing
lyrics. It does not learn your entire voice from one sentence, and it does not
recognize which sound you recorded: you choose the sound label before recording.

Your first goal is two understandable words. A polished, expressive singer takes
more sounds, practice, and listening. Everything here works offline.

## Before you begin: the words and controls

| Word | Plain-language meaning |
|---|---|
| Project | A Songbird folder containing a composition, recordings, and other assets. Check its path at the top of the window. |
| Phoneme / phone | One speech sound, not one alphabet letter. “See” contains an “sss” sound and an “ee” sound. |
| Sound label | A short code naming a sound. The English bank uses ARPABET, a spelling system for sounds: `S` means “sss,” `IY` means “ee.” You can use the descriptions in the Sound dropdown instead of memorizing codes. |
| Take | One attempt at recording a sound. You can keep several attempts and choose the best. |
| Pitch / note | How high or low a sound is. Names such as A3 and C4 identify pitches; the number identifies the octave. Higher octaves have higher pitches. |
| Hz | Vibrations per second, another way to describe pitch. A3 is 220 Hz with the standard tuning used here. |
| Root | The pitch you actually recorded. Songbird needs this label to transpose the sample correctly. Changing the label does not retune the recording itself. |
| Voiced / unvoiced | A voiced sound has vocal-fold vibration and usually a detectable pitch, like “ee” or “mmm.” An unvoiced sound like “sss” usually has no musical pitch. |
| Register | A name grouping recordings for a part of your range. We use `main` here. It is a label you choose, not a gender classification or a measured pitch. |
| Expression | A label for how you performed a sound, such as `normal` or `soft`. Start with `normal`. |
| Trim | Instructions selecting which part of a take to play. Original audio remains intact. |
| Publish / Use | Make a prepared take available to the singing engine. Recording a take alone does not publish it. |
| Vocal preset | Reusable settings choosing a bank, its recording profile, and optional effects. Make one after your first words work. |

There are two places to type:

- **Command prompt:** press **Ctrl+L**, type a command, then press **Enter**. All
  `voicebank ...` examples go here. Run one line at a time and wait for it to finish.
- **Composition editor:** press **Ctrl+E**. This is where the music code lives.
  Only the song example near the end belongs here.

**F1 / Help** opens these guides. Studio's **Beginner guide** button opens this
page. Commands and errors appear in the console. Enter `console` if it is hidden.
The tools below stay in the existing sidebar; the main editor remains your music
workspace. If the sidebar is hidden, enter `tracks` once to show it.

## 0. Choose a project folder

Open Songbird. The project path at the top tells you where your bank will live.
You can use your existing project, but a separate practice project keeps your
first experiments together. Save any composition edits with **Ctrl+S** first.

To make a practice project, enter this in **Songbird's command prompt**:

```text
new "/absolute/path/to/My First Voicebank"
```

Replace the quoted path with a new folder location on your computer. On Windows,
a path can look like `C:/Users/your-name/Music/My First Voicebank`; on macOS,
`/Users/your-name/Music/My First Voicebank`; on Linux,
`/home/your-name/Music/My First Voicebank`. Replace `your-name` with your actual
account name. Keep the quotes if the path contains spaces. Do not paste the
placeholder unchanged. Wait for “Project opened” and check the path at the top.

Stop playback and finish any active recording before creating or opening a
project. A new project starts with example music; leave it stopped while
recording your first sounds. Nothing starts the microphone automatically.

## 1. Create a bank

Enter these commands one at a time:

```text
voicebank new voices/nova "Nova" english
voicebank register main
voicebank expression normal
voicebank settings 48000 24 1 flac
```

“Nova” is just the bank name; choose your own if you like. `voices/nova` is its
folder **inside the current project**. Choose a new folder name when creating a
bank; Songbird will not overwrite an existing bank with `new`.

The recording settings mean:

| Value | What it controls |
|---|---|
| `48000` | Sample rate: 48,000 audio measurements per second. A common recording setting. |
| `24` | Bit depth: recording precision. Start with 24-bit. |
| `1` | Mono: one microphone channel. `2` means stereo. One microphone usually needs mono. |
| `flac` | Lossless compressed audio, preserving recorded detail. `wav` is also supported. |

You should see Voicebank Studio in the right sidebar. Songbird creates
`voices/nova/voicebank.toml` and a Studio session file there. You do not need to
write those files by hand.

Already created this bank? Reopen it instead:

```text
voicebank open voices/nova
```

If a directory or TOML file is reported missing, check the project path at the top.
That bank must belong to the project you currently have open. Reopening preserves
takes and preparation metadata; it does not start capture.

## Select your target language and open its phonetics guide

The final `english` argument above selects English when creating the bank. For
another language, create a separate bank with the desired tag instead, such as
`voicebank new voices/my-fr "My French Voice" fr`. Select language at creation,
then open its guide before recording:

- [English sounds](phonetics/en.md)
- [German sounds](phonetics/de.md)
- [French sounds](phonetics/fr.md)
- [Japanese sounds](phonetics/ja.md)
- [Latin sounds](phonetics/la.md) (voicebanks only)

The **ID**, IPA, highlighted example and recording checklist explain exactly what
to record. “Me see” below is an English starter exercise; it is not a pronunciation
recipe for every language. For another bank, choose short words with known entries
in its language dictionary and use its coverage report. Follow this sequence:
create bank and select language → open phonetics guide → record sounds → validate
and Use samples → test singing → save/export the bank. See the
[phonetics overview](PHONETICS.md) for naming, formats and source-audio handling.

## 2. Prepare the microphone

Expand **Audio devices & recording** above the editor:

1. Select your microphone under **Input** and headphones or speakers under **Output**.
   **System Default** uses your operating system's current default device.
2. Click **Refresh devices** if a device is missing after connecting it.
3. Leave **Monitor input** off for your first attempt. Monitoring means hearing
   your microphone through Songbird as you record; use headphones if you enable it.
4. Choose a quiet place. For the first take, do not play the backing music.

**Important:** the general **Record** button makes a song recording. For bank
sounds, use **Record bank target** inside Studio's **Pitch coach** section.
Bank recording rate, bit depth, channels, and format come from
`voicebank settings`, not the general recording dropdowns.

You will check levels during the first short recording below. If it clips, lower
microphone gain in your microphone/interface or operating system and record again.
Clipping means the input was too loud and became distorted; lowering playback
volume afterward cannot repair it.

If capture reports an unsupported rate, try:

```text
voicebank settings 44100 24 1 flac
```

If necessary choose another supported device/channel setting; do not keep
retrying a configuration the device rejects. Advanced users can still use
`devices`, `input "EXACT DEVICE NAME"`, `output "EXACT DEVICE NAME"`, and
`monitor off`. See [device setup](OPERATIONS.md#device-and-recording-acceptance).

## 3. Record a useful first word's sounds

We will build these three sounds:

| Label | What to record | First use |
|---|---|---|
| `IY` | “ee,” as in **see**; one steady held vowel | The vowel in both “me” and “see” |
| `M` | A short closed-lip “mmm,” not the letter name “em” | The beginning of “me” |
| `S` | “sss,” not the letter name “ess” | The beginning of “see” |

In Studio's **Sound** dropdown, select **IY**. Its description and reference word
appear in the sidebar. The equivalent command is `voicebank target IY`.

1. Leave **Live tuner while recording** enabled. You can leave **Use a target note**
   off; you do not need to know your note yet.
2. Click **Record bank target**.
3. Leave a little silence, then sing a comfortable, steady “ee” for about one
   second. Leave a little silence after it. Avoid adding a spoken “see” or “eye.”
4. Click **Stop bank recording**. Wait for finalization and analysis to finish.

A new take and waveform should appear. If not, read the console before continuing.
Each attempt has its own file; retrying does not overwrite your previous take.
Keep individual bank takes under 30 seconds. The button equivalents are
`voicebank record` and `record-stop`, entered in the command prompt.

While recording, Pitch coach may show a note such as **A3 · 220 Hz**. Your note can
be different. The goal is a comfortable steady sound, not reaching a particular
range. A flat/sharp indicator compares your pitch with the nearest note, or with
your chosen target. **Cents** measure that difference: 100 cents is one semitone,
the distance between neighboring piano keys. The target is guidance, not automatic
pitch correction. Silence and unvoiced sounds may show no note; that is expected.

## 4. Inspect, audition, and choose a take

Use the **Take** dropdown to select your attempt, then click **Listen original**.
Use the main **Stop** button to stop a preview. Click **Listen trimmed** to hear
only the portion selected by the current trim markers.

You should hear your “ee” clearly, without crackling, obvious background music,
or a chopped-off beginning/end. Check level and clipping warnings. If it is too
quiet, noisy, or distorted, adjust your recording setup and make another take.
Choose the better attempt in the dropdown; you do not have to delete the others.

The command equivalents are:

```text
voicebank takes
voicebank take 1
voicebank replay raw
stop
voicebank replay
stop
```

`1` means the first take in this bank, not necessarily the newest or the best.
Use the number listed for your own attempt. “Raw” means the original recording.

## 5. Correct pitch and timing

The analysis proposes starting values. Review them before **Use selected take**.
You do not need to manually enter every setting if the suggestions and playback
are already correct.

### Check the root pitch

For the `IY` take, look for **Measured root**. If it matches the steady note you
recorded, click **Apply measured root**. This explicitly updates the selected
take's root label; it neither changes the audio nor publishes it.

Remember the nearest note name shown, such as A3. Later preview examples use A3;
**replace A3 with your own recorded note wherever needed**. Do not relabel a
recording as A3 just to match the examples. An incorrect root label makes the
engine transpose from the wrong starting point.

If there is no reliable root, record a clearer steady vowel and try again. **Re-analyze**
refreshes measurements while preserving your manual edits. Experienced users can
enter `voicebank set root 220` or `voicebank set note A3` only when those values
actually describe the recording. No root note is expected for `S`.

### Understand the trim markers

All marker times are seconds counted from the beginning of the original take.
The waveform shows loudness over time; listen as well as looking at it.

| Marker | Where it belongs |
|---|---|
| onset | Where the useful sound begins, after unwanted leading silence. Keep a consonant's attack. |
| sustain | Where the stable part begins. For a voiced syllable, place this in the held vowel, after its initial attack. |
| release | Optional: where the held part ends and the final sound/tail begins. Keep natural endings. |
| end | Where the useful sound finishes, before unwanted trailing silence. |

If the prepared replay sounds correct, keep the suggested markers. If you need
to correct them, enter `voicebank trim ONSET SUSTAIN END` with your own times.
**For illustration only**, if your recording starts at 0.10 seconds, becomes
steady at 0.20, and ends at 1.10:

```text
voicebank trim 0.10 0.20 1.10
voicebank set release 1.00
```

Do not paste these values unless they match your recording. Onset must be at or
before sustain, sustain must be before end, and end must be inside the file.
An optional release belongs between sustain and end. Trims only change playback
instructions; your source file stays intact. Listen trimmed again after edits.

### Check voicing and range, then publish

`IY` and `M` are voiced. If their voicing label is uncertain, confirm it with:

```text
voicebank set voiced true
```

The **low/high** note labels tell Songbird which requested notes this sample may
serve. Initial labels are near the measured root. Keep your first preview at the
recorded note. If you later set a wider range, that changes permission to reuse a
sample—it does not prove the sample will sound good over that range. Add new
recordings at other pitches when needed. A voiced sustain must contain at least
three cycles of its root pitch; an extremely short stable region will be rejected.

Click **Use selected take**. Its command equivalent is:

```text
voicebank use
```

This validates the take and adds it to the playable bank. If it fails, correct the
reported issue first. Using another take with matching sound/profile/range may
replace the preferred playable mapping, while retaining the original takes.

`voicebank suggest` is a separate advanced command that copies the analysis's
suggested timing/root/voicing into editable metadata. It can replace your manual
edits; use **Re-analyze** when you only want a fresh report.

## 6. Add consonants and more vowels

Repeat the record → listen → prepare → Use workflow for **M**, then **S**:

- Select **M** in the Sound dropdown. Hum a short closed-lip “mmm” at about the
  same comfortable pitch as your `IY`. It is voiced: review its measured root and
  confirm `voiced true` if needed. Click **Use selected take** after checking it.
- Select **S**. Record a short, clear “sss.” It is unvoiced, so a missing tuner note
  is normal. After stopping, enter `voicebank set voiced false`, review the trim,
  listen, and click **Use selected take**. Do not invent a sung root for it.

Now your bank has the pieces for **“me see”**. A neutral pitch reference may be
stored for unvoiced playback; that is not a claim that “sss” has a musical pitch.

For other consonants, follow the reference words in the panel. Do not try to hold
a short stop sound like “t” as if it were a vowel; capture its natural beginning
and release. **Next missing sound** chooses another missing label for your current
language/register/expression. It is useful after this three-sound exercise.

Later you can record a whole syllable with `voicebank record M.IY`. A dot joins
sound labels into one syllable; “M.IY” means “me.” Place sustain inside its vowel.
A compatible complete-syllable recording is preferred over separate sounds when
available. Otherwise the engine uses the individual sounds, not an unrelated sample.

To bring in a short recording you already own or have permission to use:

```text
voicebank import "/absolute/path/to/ee.wav" IY
```

Replace the path with your actual file. Windows paths can use forward slashes,
such as `C:/Users/your-name/Music/ee.wav`. Import copies the source into the bank
and analyzes it; select and prepare that take normally. Sources over 30 seconds
need a short preparation copy; retain the full original.

## 7. Check coverage before singing

**Coverage** asks whether your published samples can sing the requested sounds
and notes. A take sitting in the dropdown does not count until you have used it.
Enter:

```text
voicebank coverage "me see" "A3:1 A3:1"
```

Replace both A3 names with your recorded note if different. `A3:1` means “sing A3
for one beat.” A beat is a unit of musical time; the song's tempo controls how
quickly beats pass. The two words here get one note each at the same pitch.

If you see a missing sound, record and Use that label. If a profile does not match,
check that all three samples use the same `main` register and `normal` expression.
If a requested note is outside the allowed range, try your actual recorded pitch
or make another sample near the desired note rather than widening limits blindly.

These commands check the larger inventory:

```text
voicebank validate
voicebank missing
```

An English bank has 39 base sound labels in this inventory. A three-sound bank will
report many missing labels: that is normal. “Me see” can work without the rest,
but other lyrics need their own coverage. Passing coverage checks structure and
availability; it does not guarantee clear or natural singing.

Songbird has built-in English pronunciations. If you later need a custom word,
a dictionary entry supplies its sounds. For example `voicebank dictionary MEE "M IY"`
spells a one-syllable word with two phones. `/` separates syllables in longer
entries. See the [reference](VOICEBANKS.md) when you need this; it is not required
for the first “me see” exercise.

## 8. Preview singing and correct problems

Stop any song/sample playback, then enter:

```text
voicebank test "me see" "A3:1 A3:1"
```

Again, use your recorded note in place of A3. You should hear two joined syllables:
“mmm-ee” followed by “sss-ee.” This is the first test of word formation from your
own bank. It uses the selected output device and needs no exported song. Enter
`stop` or use the main Stop button before replaying or changing samples.

| What happens | What to check |
|---|---|
| Missing bank directory / TOML | Check the current project path, then reopen the bank with `voicebank open voices/nova`. |
| Missing sound even though you recorded it | Select its take and Use it; check the register/expression and coverage report. |
| Voice is silent or unexpectedly clipped | Check selected output and source replay, input levels, trims, and the console error. |
| Words sound like isolated tones | Check that M and S are actual consonant recordings and the IY vowel is clear; synthetic test fixtures are not human speech. |
| Wrong pitch | Check the root against the recorded sound; a target note never retunes a recording. |
| Chopped beginning or ending | Compare Listen original with Listen trimmed. Restore the attack or tail using your own marker times. |
| Strange sound on higher/lower notes | Stay near the recorded pitch and add samples at new roots rather than stretching one sample too far. |
| Uncertain pitch on S | Expected: S is unvoiced. Set its voicing false instead of guessing a note. |

When both words work at the recorded pitch, try a nearby note already inside the
sample ranges. For example `A3:1 B3:1` moves the second word upward. Only then try
longer phrases and more sounds. Naturalness is a listening judgment; metadata
checks cannot replace it.

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

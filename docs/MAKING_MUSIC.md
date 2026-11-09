# Make a song with code

A Songbird composition is the score for your recording session. Describe what
plays, how it sounds, and when it enters. Listen, change a few lines, and evaluate
again. Recorded performances and samples become tracks in that same score.

This walkthrough builds **Night Walk**, a sixteen-bar piece with an intro, a
verse, and an ending. Open the complete example from the repository:

```sh
./target/release/songbird examples/songwriting
```

On Windows, use `.\target\release\songbird.exe examples/songwriting`. You can also
open the folder through Songbird's `open` prompt command. Stop audio before
switching projects. To make your own copy, enter `save-as "/absolute/new/folder"`
in the prompt; choose a new destination outside the example, with an existing
parent directory.

Press Ctrl+P to listen. The entire score is in
[composition.sb](../examples/songwriting/composition.sb). For the following
steps, experiment in the editor or start a new project and build the score up.
Ctrl+E focuses the editor; Ctrl+L focuses the action prompt. Composition snippets
below belong in the editor; recording/export commands belong in the prompt.

## 1. Set the feel and write a beat

Start with:

```text
project "Night Walk"
tempo 108
signature 4/4
bars 16

track drums {
    instrument drumkit
    pattern "kick - snare hat kick hat snare hat"
    gain -15db
    loop
}
```

Press Ctrl+Enter, then Ctrl+P. At this tempo, the beat is relaxed rather than
rushed. Each pattern item occupies half a quarter-note beat, so eight items fill
one 4/4 bar. Read it as **one-and, two-and, three-and, four-and**:

| Count | 1 | and | 2 | and | 3 | and | 4 | and |
|---|---|---|---|---|---|---|---|---|
| Sound | kick | rest | snare | hat | kick | hat | snare | hat |

Replace the first rest with `hat` to fill out the rhythm. Replace the second
`kick` with `-` to make it sparser. Evaluate after each change and listen to how
the groove changes. Keep one version you like before adding more instruments.

`loop` repeats this one-bar idea through the song. For a fixed number of bars,
remove `loop` and write `pattern "(kick - snare hat kick hat snare hat)*4"`.
The parentheses group the bar; `*4` plays it four times.

## 2. Give the beat a bass line

Add this beside the drum track:

```text
track bass {
    synth saw
    notes "C2:1 -:1 G2:1 Bb2:1"
    adsr 0.01 0.15 0.55 0.08
    filter 800
    gain -15db
    loop
}
```

The low C establishes a tonal center. G reinforces it; Bb adds a minor/blues
color. Each `:1` lasts one beat, making the four items one bar. The rest leaves
space instead of putting a note on every beat. Both tracks start together and
repeat independently on the same transport.

Try `C2:2 G2:1 Bb2:1` for a longer opening note. Try `C3` instead of `C2` to hear
the octave difference. Change one thing at a time and keep listening.

The saw gives the bass a bright starting sound. `filter 800` softens it. ADSR
controls how quickly notes begin, settle, sustain, and release. To hear another
character, replace `saw` with `triangle` or `square` and adjust the filter.

## 3. Add harmony, then a melody

Add a chord track:

```text
track chords {
    synth triangle
    notes "[C3,Eb3,G3]:4 [Ab2,C3,Eb3]:4 [Bb2,D3,F3]:4 [G2,B2,D3]:4"
    gain -18db
    reverb 0.22
    loop
}
```

Brackets play notes together. Each chord lasts four beats: one bar. This gives
you a four-bar harmonic cycle, while the drums and bass repeat their one-bar
patterns underneath. The G-major chord's B natural adds tension before returning
to C minor. For closer harmonic support, develop a four-bar bass line following
the chord roots C2, Ab1, Bb1, and G1.

Now add a short melody above the harmony:

```text
track melody {
    synth square
    notes "G4:1 Eb4:1 D4:1 C4:1"
    filter 1800
    gain -20db
    pan 0.2
    loop
}
```

G, Eb, and C belong to the opening C-minor chord. D passes between Eb and C,
giving the phrase movement. Change the last note to `-:1` and hear the phrase
leave a gap. Try `G4:2 Eb4:1 C4:1` to stretch the first note instead.

If the lead obscures the chords, lower its gain before adding more effects.
A small pan offset separates it from the centered bass. Add
`delay 0.25 0.25 0.15` for a quiet echo; listen for whether the echo helps the
rhythm or crowds it. Add `master limit -1` outside all tracks to set an output
ceiling, and watch the meters as you balance the parts.

## 4. Keep a sound as a reusable preset

Move sound settings into a named instrument above the tracks that use it:

```text
instrument warm_bass {
    synth saw
    adsr 0.01 0.15 0.55 0.08
    filter 800
    gain -15db
}

track bass {
    instrument warm_bass
    notes "(C2:1 -:1 G2:1 Bb2:1)*8"
}
```

The instrument defines the sound; the track supplies the performance. Use the
same instrument in another track to keep its sound consistent. Change the preset
and evaluate to update its uses together. Presets are saved with the composition;
copy a definition into another project to reuse it there.

## 5. Turn the loop into an arrangement

An arrangement introduces, develops, and resolves your ideas. In the complete
example, the drums run underneath three sections:

| Section | Starts | Length | Musical role |
|---|---|---|---|
| Intro | `0b` | 4 bars | Establish drums and bass |
| Verse | `16b` | 8 bars | Bring in chords and melody |
| Ending | `48b` | 4 bars | Resolve to C, then leave room for effect tails |

In 4/4, four bars contain sixteen quarter-note beats. A section supplies an
offset to the tracks inside it:

```text
section verse at 16b {
    track bass {
        instrument warm_bass
        notes "(C2:1 -:1 G2:1 Bb2:1)*8"
    }
}
```

This plays eight one-bar bass phrases starting after the intro. Section names
label entrances; they do not automatically stop tracks. Use explicit pattern
lengths instead of `loop` when a part should end at a section boundary. In the
example, the drums repeat fourteen times and then stop. The closing notes also
finish after bar fourteen, leaving two bars for the echo and reverb to fade.

Use the [complete score](../examples/songwriting/composition.sb) to see all three
sections together. For a variation, change only the final four melody phrases,
remove the hats during the intro, or add a new answering track halfway through
the verse. Preserve the overall lengths so the ending stays in place.

## 6. Add your own performance

Record a spoken introduction, a sung phrase, or an instrument over the music.
In the prompt, run `devices`, select your input using `input "DEVICE NAME"`, then:

```text
channels 1
monitor off
record opening flac
```

Perform the take, then enter `record-stop`. Wait for its saved path and add it to
the editor:

```text
track opening {
    import "recordings/REPLACE_WITH_YOUR_SAVED_TAKE.flac"
    place at 0b
    gain -3db
    fade in 20ms
    fade out 100ms
}
```

Use the actual console path. `place at 16b` would put it at the verse entrance.
Move it by ear as needed; capture is not automatically latency-compensated.
Lower the music's gain beneath speech, or use
`automate gain "0b=0.4 8b=0.4 12b=1"` on a music track to bring it back up after
the introduction. Automation times are relative to that track's start, and its
gain values are linear. Adjust the points to match your performance's length.

For an existing WAV or FLAC, enter `import "/path/to/your/file.wav"` in the prompt.
Songbird copies it into the project and adds a track. Trim and place that clip in
the score. Original audio stays intact; see the [editing reference](LANGUAGE.md#non-destructive-clips).

## 7. Write a part for a custom singer

A **voicebank** supplies the recorded sounds of a singer. A **pronunciation
dictionary** maps words to phonemes. Songbird includes an English dictionary;
a bank can override pronunciations and syllable boundaries for your performance.

Follow [Creating Your First Voicebank](FIRST_VOICEBANK.md) to record, analyze,
label, and audition your singer inside Songbird.
Then assign that voice to a track, give it lyrics, and write one note per syllable.
Use `~` for another note on the preceding vowel. The bank must cover the requested
phonemes, register, expression, and pitches. The bundled analytic voice demonstrates
vowels; use an English phoneme bank for full lyrics.

- [Voicebank creation, lyric alignment, and pronunciation overrides](VOICEBANKS.md)
- [Bundled English pronunciation dictionary](../data/cmudict.dict)
- [Dictionary source and licensing](../data/README.md)

## 8. Finish and export

Listen through the entire arrangement. Check whether entrances land where you
want them, speech is clear, and the ending has time to decay. Lower track gains
if the output clips. Press Ctrl+S to save, then enter these prompt commands:

```text
render "night-walk.flac"
render "night-walk.wav"
```

Your finished files are in the project's `exports/` directory. The render follows
the composition's `bars` length and includes its instruments, recordings, vocals,
automation, and effects. Use a new output name for each revision.

For more ways to develop the piece, keep the [language reference](LANGUAGE.md)
nearby: it covers sample instruments, patterns, transformations, effects,
automation, sections, and all prompt commands.

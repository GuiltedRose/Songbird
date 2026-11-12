# Composition language

UTF-8 text, `.sb`. Newlines and semicolons separate statements. Braces delimit
blocks; double quotes preserve whitespace; `\"`, `\\`, `\n`, and `\t` are escapes.
A `#` starting a token begins a comment; the sharp in `F#4` remains a note.
Errors report statement indexes, counting semicolons and braces as delimiters.
Unknown commands, assets, instruments, pronunciations, and malformed values fail.
No arbitrary code, shell commands, imports of executable code, or script networking.

```ebnf
score      = { global | definition | voice | instrument | track | section } ;
voice      = "voice" name "{" { voice-setting } "}" ;
instrument = "instrument" name "{" { sound-setting } "}" ;
track      = "track" name "{" { track-setting | clip-edit | effect } "}" ;
section    = "section" name "at" time "{" { track } "}" ;
sequence   = { item [ "*" integer ] } ;
item       = (note | "-" | drum | chord) [ ":" beats ]
           | "(" sequence ")" ;
chord      = "[" note { "," note } "]" ;
note       = letter [ "#" | "b" ] octave ;
```

Names may be quoted. Nested pattern groups are sequential, not simultaneous;
chords supply polyphony. A chord has no whitespace inside its brackets. Rests
advance time. Patterns are quoted when they contain whitespace. Default step is
0.5 quarter-note beats; `:duration` overrides an item's duration. Group `*N`
repeats its contents N times. Notes are MIDI 0–127; middle C is `C4` (60).

## Global settings and definitions

```text
project "Midnight Garden"
tempo 140
signature 4/4
bars 8
rate 48000
bits 24
let riff "(C2 - G2 -)*2"
instrument pluck {
    synth saw
    adsr 0.005 0.12 0.3 0.05
    pitch-envelope 2 0.03
    filter 1800
}
section verse at 0b {
    track bass { instrument pluck; notes $riff; loop }
}
section chorus at 16b {
    track bass { instrument pluck; notes "[C3,E3,G3]:4" }
}
master limit -1
```

Initial tempo/signature settings can be extended with tempo/meter maps; see
[TIMELINE.md](TIMELINE.md). Live evaluation applies prepared changes at a bar boundary. Sections add an absolute offset
to their tracks and qualify names (`verse/bass`). They cannot nest. `bars` sets
the exact export end; include extra bars to capture effect tails. Sections beyond
that end are intentionally outside the render range.

`let NAME "pattern"` defines a pattern used by `notes $NAME` or `pattern $NAME`.
Instrument definitions must precede uses. Put `instrument NAME` first in a track:
it copies sound settings, sample mappings, and effects from the definition.
Each track name must be unique within its section.

Times: seconds (`1.25` or `1.25s`), `250ms`, `00:08.250`, `01:02:03.5`,
or quarter-note beats (`8b`). `at` positions a track; `place at` positions the
selected clip relative to the track. Signature affects bar length; tempo maps
integrate quarter-note durations into elapsed seconds.

## Instruments and tracks

Built-ins: `sine`, `saw`, `square`, `triangle`, `noise`, `analog` (saw),
`soft-lead` (sine), `drumkit`, `sample`. Saw and square use polyBLEP; triangle
uses a bounded band-limited harmonic sum. Drum tokens are `kick`, `snare`, `hat`,
and `clap`, with deterministic procedural defaults. Load your own kit:

```text
instrument kit {
    synth drumkit
    drum kick "samples/kick.flac"
    drum snare "samples/snare.wav"
}
track drums { instrument kit; pattern "kick hat snare hat"; loop }
track piano {
    sample "samples/piano-C4.flac"
    sample-root C4
    notes "[C4,E4,G4] - G4 C5"
}
```

A sample instrument uses its first sample and triggers it from note events;
its default root is C4. Import-only tracks play their clips at their placements.
Sample-instrument note durations bound each trigger. Kit triggers are likewise
bounded by the pattern item's duration. Use longer `:duration` for long tails.

| Setting | Meaning |
|---|---|
| `notes "C4 E4 G4"` / `pattern "kick - snare -"` | Sequence |
| `step 0.25` | Default item length, in beats |
| `loop` | Repeat sequence through arrangement end |
| `repeat 3` | Expand the current sequence three times |
| `transpose -12` | Semitone transpose |
| `retrograde` | Reverse event order with durations retained |
| `adsr 0.01 0.1 0.7 0.1` | Attack/decay seconds, sustain 0–1, release seconds |
| `pitch-envelope 12 0.08` | Initial semitone offset and exponential decay seconds |
| `gain -6db` / `gain 0.5` | Track gain, dB or linear |
| `pan -0.25` | Equal-power pan, -1 left to +1 right |
| `mute` / `solo` | Track playback selection |
| `at 8b` | Track offset |
| `automate gain "0=0 1=1 3=0.5"` | Piecewise linear gain at local times |
| `automate pan "0=-1 4=1"` | Piecewise linear pan |
| `automate bend "0=0 0.5=2 1=0"` | Semitone bend for oscillators and singers |

Automation times increase strictly. Gain automation is linear, not dB. Each
parameter should have one automation statement; later statements take precedence.
Track pan uses equal-power attenuation, including a -3 dB coefficient at center.
Clip pan and track pan both apply when used together. Master processing follows
track summation. PCM export/output clamps samples outside full scale; meters and
render diagnostics report clipping. A master limiter can prevent it.

## Non-destructive clips

Edits always reference source audio; they never rewrite it. Most commands operate
on the most recent clip. A split appends its right half and selects that half.
Duplicate appends and selects its new copy. Delete removes the current last clip.
Edit the source text to address any earlier clip or group of clips; a `group`
label is metadata, not an implicit bulk-operation selector.

```text
track dialogue {
    import "recordings/scene.wav"
    trim 0.3 4.8
    split 2.0
    place at 8b
    clip-gain -2db
    clip-pan -0.1
    fade in 30ms
    fade out 120ms
    reverse
    stretch 1.1
    pitch -2
    normalize
    group scene-one
    clip-effect eq -2 1 0
    duplicate 16b
    import "recordings/alternate.flac"
    trim 0.1 2.3
    crossfade 80ms
}
```

`trim START END` and `crop START END` define source bounds. Split requires explicit
bounds and a source time strictly inside them. Move (`move TIME` or `place at TIME`)
and duplicate positions are track-relative. Stretch factors 0.25–4 alter duration
while granular overlap/add approximates pitch preservation. `pitch -24..24`
shifts pitch with duration retained. These are basic algorithms and may create
artifacts on transients or extreme changes. Fades are linear. Normalize scales
to 0.98 using the source asset's whole-file peak, not the trimmed segment's peak.
Crossfade places the second of the last two clips over the first's end and sets
matching fade lengths; explicitly trim the first clip before using it.

## Effects

Effects process in written order, one stateful chain per clip/track/master.
Use `clip-effect EFFECT ...`, bare effects inside tracks/instruments, and
`master EFFECT ...` globally. All operate identically in playback and export.

| Syntax | Parameters |
|---|---|
| `eq LOW MID HIGH` | Three broad bands, gains -24..24 dB |
| `filter HZ` | One-pole low-pass, 20..20000 Hz |
| `compress THRESHOLD RATIO` | Threshold -90..0 dB, ratio 1..100 |
| `limit DB` | Instantaneous peak ceiling, -100..0 dB |
| `gate DB` | Smoothed amplitude gate |
| `denoise DB` | Basic amplitude-dependent attenuation; no learned noise profile |
| `delay SECONDS FEEDBACK WET` | Delay (0,4], feedback magnitude <1, wet 0..1 |
| `reverb WET` | Four short feedback combs, wet 0..1 |

## Console commands

Use the separate prompt for operations; these are not saved as composition text.
An optional leading `:` is accepted.

| Commands | Operation |
|---|---|
| `eval`, `play`, `stop` | Compile/validate, play or schedule replacement, stop |
| `new "DIR"`, `save`, `save-as "DIR"`, `open "DIR"` | Project operations |
| `import "FILE"` | Validate and copy an external WAV/FLAC to project assets |
| `render "NAME.flac"` / `render "NAME.wav"` | Export to project `exports/`; no overwrite |
| `convert "samples/FILE.wav" "OUTPUT.flac" [RATE BITS]` | Convert into exports; rate/depth default to project settings |
| `devices`, `input "NAME"`, `output "NAME"` | Enumerate/select native audio devices |
| `channels 1` / `channels 2` | Recording channel count |
| `monitor on` / `monitor off` | Input monitoring, default off |
| `record NAME flac` / `record NAME wav` | Start a new hardware take |
| `record-stop` | Stop input immediately; finalize asynchronously |
| `voice-new "voices/nova" Nova` | Create an empty, editable bank manifest |
| `voice-check "voices/nova"` | Validate bank metadata and decode all samples |
| `undo`, `redo` | Evaluation/import snapshots; text edits also have editor undo |
| `find "TEXT"`, F3 | Search and select successive matches |
| `tracks`, `console` | Hide/show panels |
| `recover-draft` | Load the recovery draft into the editor |
| `help` | List console commands |

Live updates are prepared and their effect history is rendered on a background
worker. They target a future bar after the existing bounded output queue. If an
update misses its prepared boundary, original playback continues and the console
reports it. Sample-rate changes require stopping playback. Live replacement is
not a crossfade between arrangements and may click if the new waveform differs
at the boundary. `bars` must extend beyond the chosen live-update boundary.

## Voicebank Studio and vocal presets

Enter `voicebank help` in the prompt to create, record, analyze, label and test
voices inside Songbird. Follow [Creating Your First Voicebank](FIRST_VOICEBANK.md)
or consult the [complete Studio reference](VOICEBANKS.md).

A singer track can load a portable TOML performance preset directly:

```text
track backing {
    vocal-preset "voices/nova/preset-Soft-Harmony.toml"
    lyrics "me see"
    notes "A3:1 B3:1"
    gain -12db
    formant 0.95
    dynamics 0.8
    vocal-pitch -0.1
    harmony "0 7"
}
```

The preset supplies the bank/register/expression and other vocal controls. Explicit
track controls override those settings; track effects follow preset effects.
`harmony` is a quoted list of semitone offsets and expands to separate singers,
all of which need bank pitch/profile coverage. `formant` ranges from 0.5 to 2,
`dynamics` from 0 to 2, and `vocal-pitch` from -24 to 24 semitones. These controls
also work on ordinary `use voice NAME` tracks. The normal resource limits apply.

## Language selection

Score, voice and track blocks accept `language TAG`, `inventory PROFILE`, and
`provider cmudict|dictionary|rules|manual` (choose one provider). Explicit track
settings precede voice settings; physical bank inventories still constrain samples.
Unannotated legacy banks remain English. Configure new-bank defaults separately
with `language project TAG` and `language default TAG` in the console.

```text
language la
voice latin { bank "default:la"; language la }
track greeting { use voice latin; notes "C4:1 D4:1"; lyrics "salve" }
```

`default:TAG` generates a labeled procedural bank when preparing a used voice.
Punctuation records phrase marks, without allocating notes or adding pauses.
Write rests explicitly. See [language packs](LANGUAGES.md) and
[provider/phrasing research](LANGUAGE_RESEARCH.md).

## Mini-notation and interpolation

`notation mini`, `cycle BEATS`, `cycles COUNT` and `seed INTEGER` opt a track into
the [mini frontend](MINI_NOTATION.md). Legacy notation stays the default.
`automate PARAM "TIME=VALUE ..." [linear|step]` accepts gain, pan, bend and filter;
filter requires a track filter effect. See [export and transport additions](DAW_EXPANSION.md).

## Musical maps and timeline edits

Tempo/meter maps, markers and repeated sections use the syntax in
[TIMELINE.md](TIMELINE.md). Clip selection, musical splitting/trimming, snap and
effect/vocal automation are documented in [EDITING.md](EDITING.md). Recorded-source
trim coordinates remain independent of musical time. Transport and punch/loop
recording commands are in [RECORDING.md](RECORDING.md).

## Optional mixer routing and analysis

Tracks can declare `route NODE`, `send NODE GAIN [pre|post]`, `drum-only LABEL`
and one-based `bypass INDEX`. Project-level `channel NAME { source TRACK ... }`
and `bus NAME { ... }` blocks add grouping, nested routing, sidechain detectors
and new native EQ/dynamics inserts. See [the complete syntax and signal flow](MIXING.md)
for ranges, automation, mute/solo, limits and stem behavior. Mini still contains
patterns only; these mix directives remain native Songbird code.

Prompt `analyze` performs full-render offline sample-peak/RMS/clipping/correlation
and gain-reduction analysis in the background. `plugins` reports actual hosting
compatibility; no foreign plugin backend is operational. These are prompt commands,
not lines to insert into a composition. See [plugin status](PLUGINS.md).

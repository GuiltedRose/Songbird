# Mini-notation

Mini-notation is an opt-in frontend to Songbird's existing renderer. It adds no
JavaScript runtime, sample service or second audio engine. Open `help mini` offline.

```text
track melody {
    synth triangle
    notation mini
    cycle 4
    cycles 4
    seed 42
    notes "[c4 [e4 g4]] <g4 a4>"
    gain 0.15
}
```

`cycle` is a duration in quarter-note beats, independent of meter (default 4).
`cycles` selects a finite number of cycles (default 1). `loop` instead evaluates
successive cycles to the arrangement end; alternation/random choices progress.
Cycles are anchored to track start, including section offset. Track `seed` defaults
to zero; set distinct seeds to decorrelate tracks. Saving/reopening preserves it.
Exports render the arrangement once, including explicitly scheduled pattern loops.

| Expression | Meaning |
|---|---|
| `c4 e4 g4` | Sequential equal divisions of a cycle |
| `c4 ~ e4` | A silent division; `-` also rests |
| `[c4 [e4 g4] a4]` | Nested subdivisions |
| `c4,e4,g4` | Parallel pitches |
| `<c4 e4 g4>` | Successive choices across three cycles |
| `[c4 e4]*2` | Twice as fast |
| `[c4 e4]/2` | Half as fast |
| `c4!3 e4` | Three replicated entries, then e4 |
| `c4@3 e4` | Duration weights 3:1 |
| `kick(3,8)` | Three Euclidean pulses in eight steps |
| `hat(5,8,2)` | Euclidean rhythm rotated right two steps |
| `{kick snare, hat*3}%8` | Align branches to an eight-step cycle |
| `{c4 e4 g4, c3 g3}` | Default polymeter length is the LCM (six steps) |
| `hat?0.1` | Remove events with probability 0.1 |
| `c4\|e4\|g4` | Seeded random choice per local cycle |

Postfix operators bind to the preceding atom/group and apply left to right.
Whitespace makes a sequence; `|` chooses among sequences; commas stack branches.
Bracket grouping overrides this. For `{...}`, top-level commas separate meters;
weighted branches need explicit `%length` when their step counts are fractional.
Euclidean distribution uses Bjorklund grouping, rotated to an initial onset before
the requested right rotation. These are explicit Songbird decisions where upstream
notation/constructor documentation leaves details open.

Semantics were checked against the official [mini-notation reference](https://strudel.cc/learn/mini-notation/)
and [pattern constructors](https://strudel.cc/learn/factories/). Songbird implements
this documented subset, not the JavaScript Pattern API or all Strudel syntax.
Decimal speed factors are accepted. Negative speed, underscore elongation,
general parameter-valued operators and arbitrary JS expressions are not supported.
The limited sound-constructor/control frontend documented below is available.

## Instruments, singers and editing

Pitches require octaves. Drum names `kick`, `snare`, `hat`, `clap` use existing
synthesis. Add `drum hit "samples/hit.wav"` to trigger a local sample as `hit`.
Pitched sample instruments, transposition, effects and automation use the same
track settings as legacy notation. Unknown sample labels report the track name.
Singer patterns must be monophonic and nonoverlapping; rests allocate no lyric
syllable. Supply one syllable/melisma per emitted pitched event across **all**
scheduled cycles. Parallel singer notes produce an error; split them into tracks.
Random removal can change the required lyric count and therefore fail alignment.

Editor Ctrl+Space completes directive prefixes (`nota`, `cyc`, `see`) and rhythm
prefixes (`kick`, `hat`). Pattern operators are highlighted in quoted strings.
Evaluation diagnostics include track, expression and parser byte offset. Parsing,
asset decoding and event scheduling stay on the preparation worker.

Limits: 64 KiB expression, 2048 AST nodes, 24 nested groups/postfix operations,
1024 sequence steps, 128 replications, 65536 queried cycles/events and 262144
query work units. Existing project event/track limits also apply. Shared immutable
AST nodes avoid exponential cloning; excessive combinations fail before playback.
Events use f64 quarter-note positions, then the existing sample-indexed renderer.
See [musical time](TIMELINE.md) for tempo/meter maps and sample-frame conversion.

## Migrate an existing pattern

Leave old scores unchanged to retain their event timing. `notation legacy` is the
default. To migrate `[C4,E4,G4]:2 (C4 E4)*2`, explicitly select mini and rewrite
chords as comma stacks and durations as cycle subdivisions/weights. Legacy `:2`
is a two-beat duration; mini `@2` is a relative weight, so it is not a textual
replacement. Compare compiled timing before replacing a working arrangement.

## Example session

From the repository root (Rust and platform audio/graphics development dependencies
are listed in [platforms](PLATFORMS.md)):

```sh
cargo run --locked --offline --example mini_session -- /tmp/songbird-mini-session
cargo run --locked --offline -- check /tmp/songbird-mini-session
cargo run --locked --offline -- /tmp/songbird-mini-session
```

Choose an unused writable destination; on Windows substitute a Windows path. The
example generates its own original audio fixture, a labeled procedural Latin bank,
and FLAC/WAV mixes plus a region export. It combines mini drums, nested keys,
cycle-alternating bass, sections, an imported fixture, singing, automation and
native effects. No microphone or human voicebank is involved. It also exercises tempo changes, a ramp, meter changes and markers. Physical
punch recording remains an operator check; see [recording](RECORDING.md).

## Familiar sound constructors

You can keep native composition statements and Mini patterns together, and use a
limited familiar sound-expression spelling without running JavaScript:

```text
tempo 140
track drums {
    s("bd*2,oh*4").end("<.1 .2 .5 1>").fast(2)
    gain -12db
}
track melody {
    synth triangle
    notation mini
    notes "c4 <e4 g4>"
    cycle 4
    cycles 4
}
track native_line {
    synth sine
    notes "C3:2 G3:2"
    gain -15db
}
```

Constructors go on their own line inside a native track. Native routing, effects,
gain, instrument definitions, arrangement and exports remain available. A bare
standalone `s(...)` or `$: s(...)` creates an automatically named looping percussion
track for the arrangement. Inside an explicit track, `cycles`/`loop` retain their
normal meaning. For multiple cycles in a non-looping track, declare `cycles`.
`sound(...)` is an alias of `s(...)`; single/double quoted pattern strings work.
Chained calls can continue on following lines beginning with `.`. Standalone
constructors also work inside native sections. Use an explicitly named track when
other composition code needs a stable routing/source name. Native statements
and Mini patterns remain supported; this does not evaluate arbitrary JavaScript.

Supported chained methods:

| Method | Meaning |
|---|---|
| `.fast(NUMBER)` | Speed up the entire preceding expression; equivalent to grouping and `*NUMBER` |
| `.slow(NUMBER)` | Slow down the preceding expression with `/NUMBER` |
| `.end(NUMBER_OR_PATTERN)` | Play a fraction of the local source, from 0 to 1 |
| `.dec(SECONDS_OR_PATTERN)` / `.decay(...)` | Decay from the initial peak toward sustain over the specified seconds |
| `.sustain(LEVEL_OR_PATTERN)` / `.sus(...)` | Sustain level from 0 to 1 for the decay envelope |

For example:

```text
$: s("[bd <hh oh>]*2").dec(.4)
```

The [sound library](SOUNDS.md) lists all supported shorthand and distinct native
percussion. Explicit local `drum NAME FILE` mappings take priority over aliases.
Existing full identifiers such as `kick`, `snare`, `hat` and `clap` still work.

Numeric end/decay/sustain patterns use the same bounded Mini parser, including
alternation, subdivisions, repetitions and seeded choices. They must supply one
numeric value at every sound onset; ambiguous overlapping control values or
missing values are errors. End/sustain ranges are 0–1, decay 0–10 seconds, scalar
speed factors (0,1024]. Chain order matters: a subsequent fast/slow also transforms
previous control patterns; a control added afterward keeps its own cycle timing.
Unsupported functions and arithmetic expressions produce errors, not partial
JavaScript execution.

These constructors currently trigger percussion/local drum samples. Continue to
use native synth/notes directives for pitched instruments and singing. They do
not implement the complete Strudel Pattern API. The familiar spellings follow
[Strudel time modifiers](https://strudel.cc/learn/time-modifiers/),
[sampler end](https://strudel.cc/learn/samples/#end), and
[decay/sustain controls](https://strudel.cc/learn/effects/#decay); native audio
sources and envelope defaults remain Songbird's own.

Legacy bare native/Mini patterns keep their previous event-duration gating. The
opt-in sound constructor plays each source tail independently of step duration:
end is a fraction of the actual local file, or a procedural voice's finite natural
tail (its slowest envelope's -80 dB point). Percussion sustain defaults to zero;
a declared decay therefore reaches silence unless sustain is raised. Short edge
fades (0.5 ms attack, 2 ms release) reduce truncation clicks. Tails are bounded to
30 seconds, and preparation rejects patterns with more than 64 event candidates
inside their longest-tail window. Shorten end/decay or reduce speed on overload.
All expansion, aliases and control patterns resolve during preparation, outside
CPAL. Source files, saved compositions and existing Mini syntax are preserved.

A complete [mixed example](../examples/shorthand-session/README.md) is included for
operator playback/render checks; it has not been hardware-validated by the implementer.

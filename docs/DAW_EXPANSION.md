# DAW workflow

Songbird keeps composition text as the arrangement editor and uses one native
renderer for playback, region exports and stems. [Mini-notation](MINI_NOTATION.md)
is opt-in; existing notation remains supported. See [musical time](TIMELINE.md),
[recording and calibration](RECORDING.md), and [editing/mixing](EDITING.md).
These guides are also available offline as `help daw`, `help timeline`,
`help recording`, and `help editing`.

## Finished-song walkthrough

With the Rust toolchain and platform prerequisites in [platforms](PLATFORMS.md):

```sh
cargo run --locked --offline --example mini_session -- /tmp/songbird-full-session
cargo run --locked --offline -- check /tmp/songbird-full-session
cargo run --locked --offline -- /tmp/songbird-full-session
```

Use a new writable directory; substitute a Windows path on Windows. The example
creates original generated audio and a labeled procedural Latin bank. Its song
includes mini drums, nested subdivisions, alternating bass/chords, sections,
120→140 BPM, a linear tempo ramp, 6/8 meter, a marker, an imported fixture,
synthetic singing, effects and automation. It exports FLAC, WAV, a region and
post-fader stems. It does not contain human recordings or claim human singing.

In the application:

```text
seek @chorus
play
pause
resume
range @chorus bar:5:1
loop on
loop off
range clear
restart
render mix.flac
render-region chorus.wav @chorus bar:5:1
stems isolated-tracks
```

Adjust text and Evaluate. A failed preparation retains the valid old playback.
Ordinary live edits are prepared/prerolled on the worker and swapped at a bar
boundary. Loop edits prepare a replacement and restart the selected range after
successful preparation. Seeking likewise prepares effect history on the worker
before replacing playback. Stream restart can cause a brief hardware-dependent
gap; this is not gapless live routing. Internal loop and live-renderer transitions
use a 128-frame fade to reduce discontinuities.

For recording, follow [the punch tutorial](RECORDING.md). All takes are immutable
media; comping creates trimmed clips referencing those media. Stop recording
before changing transport or applying a loop replacement.

## Export rules

GUI output names go in project `exports/`; CLI paths may be absolute. Existing
files/directories are rejected. Region endpoints are half-open and rounded to
sample frames. Effects preroll from arrangement start. Master effects are included
in mixes; stems are isolated **post-fader, post-track-effects**, without master
effects, ignoring mute/solo so every channel can be exported. Preset harmonies
remain in their parent singer stem. Default export renders the arrangement once;
transport loops and count-in never print into it. Allocate extra arrangement bars
for tails, or choose an explicit longer end. There is no automatic tail detector.

```sh
songbird render PROJECT mix.flac
songbird render-region PROJECT excerpt.wav @chorus bar:5:1 48000 24
songbird stems PROJECT NEW_DIRECTORY
```

FLAC is the default convention; WAV is equally supported. Depth is 16/24 bit;
rate uses the supported project rates. Exports report frames, peak and clipped
samples. Disk preflight uses a conservative uncompressed-size estimate; failures
can leave partial new files for inspection. Retrying requires a new name. A stem
set is not an atomic multi-file transaction.

## Shortcuts and navigation

| Shortcut | Operation |
|---|---|
| Ctrl+P | Play/stop |
| Ctrl+Shift+Space | Pause/resume |
| Ctrl+Home | Arrangement start (clears selected range/loop) |
| Ctrl+G / Ctrl+J | Focus seek / marker-jump command |
| Ctrl+R | Record from start / stop recording |
| Ctrl+Shift+L | Toggle selected-range looping |
| Ctrl+Shift+E | Export a uniquely named FLAC |
| Ctrl+Enter | Evaluate |
| Ctrl+S | Save |
| Ctrl+L / Ctrl+E | Focus prompt / editor |
| F1 | Offline help |

Ctrl+Space completes mini-notation prefixes in the editor. Marker buttons show the
first eight markers; `jump NAME` reaches any marker. Use `marker NAME` to add a
marker at the current audible position. `range START END` defines loop points;
`range clear` removes them. Cursor/range/loop preferences are saved in
`transport.toml`, including Save As. Arrangement remains textual; there is no
piano-roll, drag-edit timeline or plugin-hosting framework.

## Bounds, validation and operation

Tempo/meter maps allow 256 changes each. Expanded arrangements allow 128 tracks,
16 singer tracks and the existing event/effect limits. Checkpoint effect buffers
are limited to 64 MiB. Transport loops require at least 128 frames. Recording is
limited to one hour and 1024 loop spans; a missed input-side loop boundary fails
capture finalization with its original WAV retained. See [storage recovery](EDITING.md).

```sh
cargo fmt --check
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo build --locked --offline --release
```

No service activation is required. Keep the prior executable and back up projects
and config before replacement. Roll back with the prior executable and backup;
older executables cannot parse new notation/maps. Source generations and original
recordings are preserved. Never delete recordings or voicebank samples to free a
PCM-cache quota.

The Linux automated suite checks analytical clock conversion, rendered onset
frames, rewind/loop effect state, deterministic mini scheduling, codec/region PCM,
non-destructive take indices, comp clips, storage quotas and recovery. Recording
gates/count-in tests are simulations. Physical microphones, monitoring, device
clock drift, native Windows/macOS operation and listening quality still require
operator checks. CI configuration is provided; no remote runner was dispatched.

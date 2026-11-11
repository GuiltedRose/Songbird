# Operator guide

Songbird is a desktop process, not a daemon. No background service or automatic
microphone activation is installed. No Freya, wallet, transaction, training,
network-budget, or live-order files are accessed by this project.

## Prerequisites and validation

Use a Linux desktop session with working ALSA and X11/Wayland/OpenGL. PipeWire may
serve an ALSA-compatible default device through the distribution's ALSA plugin;
this build uses CPAL's ALSA backend, not a native PipeWire backend. Install that
plugin if your distribution needs it. Confirm hardware access as your normal
user; do not run the application as root or change device permissions globally.

From the repository root:

```sh
rustc --version
pkg-config --modversion alsa
cargo fetch --locked
cargo fmt --check
cargo test --locked --offline
cargo build --release --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
./target/release/songbird help
```

For an air-gapped host, transfer the source and lockfile and a Cargo vendor bundle
prepared on a connected machine with `cargo vendor --locked vendor`; use the
source-replacement configuration Cargo prints. System packages must also exist
on the offline host. The executable itself needs no Cargo registry or network.

Generate a fresh fixture destination; every output name below must be unused:

```sh
cargo run --locked --offline --example make_project -- /tmp/songbird-operator-example
./target/release/songbird check /tmp/songbird-operator-example
./target/release/songbird voice-check /tmp/songbird-operator-example voices/analytic
./target/release/songbird render /tmp/songbird-operator-example /tmp/songbird-operator-example/exports/check.flac
./target/release/songbird render /tmp/songbird-operator-example /tmp/songbird-operator-example/exports/check.wav
flac --test /tmp/songbird-operator-example/exports/check.flac
./target/release/songbird gui-smoke /tmp/songbird-operator-example
./target/release/songbird /tmp/songbird-operator-example
```

`flac` is an optional independent validation tool, not a runtime dependency.
For X11 testing on a host with stale Wayland environment variables use:

```sh
env -u WAYLAND_DISPLAY ./target/release/songbird gui-smoke /tmp/songbird-operator-example
```

The smoke command closes after three GUI updates. It does not test interactive
keyboard behavior, play audio, activate a microphone, or establish idle CPU usage.

## Device and recording acceptance

In the GUI prompt:

```text
:devices
:input "EXACT INPUT NAME FROM devices"
:output "EXACT OUTPUT NAME FROM devices"
:channels 1
:monitor off
:play
:record opening flac
:record-stop
```

`record NAME` defaults to FLAC; specify `wav` as a third token to choose WAV.
Record several seconds of actual speech. Stop recording before stopping playback
if you need a known musical anchor. Wait for the take's saved path. Import it into
a track, position it explicitly, save, and reopen. Repeat with stereo and WAV.
Confirm both channels, input/output meters, audible content, clipping counters,
and the absence of dropped input frames or output underruns. Record two takes
with the same name and confirm distinct paths and unchanged first-take bytes.

Recording currently captures a standalone take; it does not automatically insert
a latency-compensated clip at the playhead. Use `at`/`place at` and manual timing
correction. Independent input/output device clocks can drift. Sample-accurate
hardware capture synchronization and measured latency compensation are acceptance
gaps, even though offline tracks use one sample-indexed transport.

Monitoring uses a second native output stream. The selected output must support
concurrent streams with playback (normally provided by a desktop mixer). Set
`monitor on` before starting the next recording, using headphones and a safe
hardware input level. Monitoring is uncalibrated and has bounded queue latency;
there is no monitoring gain control beyond your device/system mixer.

Unsupported rates/channels/formats and missing devices produce errors. Supported
device sample formats are f32, f64, signed 16/32-bit and unsigned 16-bit. Change
project `rate` to a device-supported value rather than silently changing pitch.
Device errors stop the affected stream; recording preserves its WAV. Stop, choose
an available device, and start a new take. No automatic reconnect or overwrite.
Device selection and monitoring settings last for the current application session.

## Human singing acceptance

Create an authorized English voicebank using VOICEBANKS.md. Cover all phonemes,
styles, registers, and pitch ranges in the intended phrase. Validate its files,
render a phrase and its melismas, and listen for intelligibility, transitions,
pitch accuracy, formant stability, and artifacts at multiple sample rates.
Compare playback against WAV and FLAC exports. Keep original material unchanged.
The analytic vowel fixture cannot pass this human listening requirement.

For the complete acceptance arrangement, combine a real captured voiceover,
real singing bank, drums, bass, melody, both imported formats, clip edits,
automation, and effects. Save, close, reopen, export both formats and compare
sample frames. Repeat with the microphone disconnected mid-take, output-device
loss, disk exhaustion on a disposable filesystem, and sustained normal playback.
These fault/hardware checks were not executed in the development environment.

## Monitoring and performance

The transport shows output/input peak dBFS; recording shows clipping and dropped
input frames; playback shows underrun callback count and consumed frames. A
nonzero dropped-frame count rejects a take as complete and reports its preserved
partial file. Export reports peak amplitude, full-scale exceedances, total time,
and throughput. Keep console errors with the project and device/rate information.
No telemetry or background upload occurs.

```sh
stat -c '%s bytes' target/release/songbird
/usr/bin/time -v ./target/release/songbird help
./target/release/songbird bench /tmp/songbird-operator-example /tmp/songbird-operator-example/exports/benchmark.wav
```

`help` measures CLI initialization, not GUI startup or idle memory. To measure
idle GUI CPU/RSS use your process monitor or `pidstat -p PID 1 30` after launching
and waiting for compilation/evaluation to finish. Measure native startup to the
first displayed frame and real round-trip latency with an audio loopback capture;
record rate, buffer configuration, server/device, hardware, and load. Queue
capacities are not latency measurements. Repeat at the intended project size.

## Project safety and recovery

`project.toml` selects an immutable composition generation. Saving writes and
syncs the generation, keeps `project.previous.toml`, atomically replaces the
manifest, then updates the editable `composition.sb` mirror. Open uses the
manifest's selected generation. Old generations are deliberately retained.
External edits to the mirror alone do not change the selected generation: edit
in Songbird, or change the manifest's `composition` to `composition.sb` after
backing up the project. No autosave rewrites original audio.

Dirty editor text is written to `recovery.sb` about every five seconds while the
background worker is available, and on normal close. An export can delay periodic
draft writes. `recover-draft` loads that text; evaluate and save deliberately.
Recovery drafts may contain invalid or incomplete code. Save before long renders.
The editor is temporarily read-only during worker operations so snapshot-based
open/save/import operations cannot discard concurrent edits.

With the application closed:

```sh
./target/release/songbird recover /absolute/path/to/project
./target/release/songbird check /absolute/path/to/project
./target/release/songbird recover-take /absolute/path/to/interrupted-take.wav
```

`recover` selects the previous manifest; it leaves audio and generations intact.
`recover-take` repairs RIFF/data lengths into a uniquely named new WAV, preserving
the interrupted file. It supports Songbird PCM/extended-PCM WAV below 4 GiB and
ignores an incomplete final sample frame. It is not a general repair tool for
arbitrarily damaged WAVs. FLAC takes keep their capture WAV for this purpose.
File-write errors preserve partial outputs, but partial files must not be treated
as completed renders or recordings. Export to another unused name after repair.

Missing assets produce an explicit path error. Restore the original relative
path from backup or edit the import/bank entry. There is no automatic relinking.
Save As requires a new destination outside the source project with an existing
parent. It copies referenced assets plus all recordings/samples/voice trees,
including unused takes; it omits regenerable cache and past exports. Referenced
asset symlinks may resolve only inside the project; copied asset trees refuse
symlinks. Original files remain in place, including on a failed copy.

Cache PCM files are temporary and normally removed on asset drop. After a crash,
with Songbird closed, files under project `cache/` can be deleted and regenerated.
Do not delete recordings, samples, voicebanks, source generations, or manifests.
Use an independent backup for user media; undo is not a media backup.

## Optional per-user installation and activation

These are operator commands; installation/activation was not executed. Do this
only after the acceptance checks above. Preserve a preexisting non-symlink binary
instead of replacing it. Keep old release directories and all project folders.

```sh
songbird_version=0.1.0
songbird_release="$HOME/.local/share/songbird/releases/$songbird_version"
mkdir -p "$songbird_release" "$HOME/.local/bin" "$HOME/.local/share/applications"
install -m 755 target/release/songbird "$songbird_release/songbird"
install -m 644 LICENSE "$songbird_release/LICENSE"
install -m 644 data/CMUDICT-LICENSE "$songbird_release/CMUDICT-LICENSE"
if [ -e "$HOME/.local/bin/songbird" ] && [ ! -L "$HOME/.local/bin/songbird" ]; then
    echo 'Preserve the existing regular binary before activation.'
else
    if [ -L "$HOME/.local/bin/songbird" ]; then
        readlink "$HOME/.local/bin/songbird" > "$songbird_release/previous-target.txt"
    fi
    ln -sfn "$songbird_release/songbird" "$HOME/.local/bin/songbird"
    install -m 644 packaging/songbird.desktop "$HOME/.local/share/applications/songbird.desktop"
fi
```

Use a new release-directory name for subsequent versions. Ensure `$HOME/.local/bin`
is in your desktop session PATH, or launch the absolute binary with an explicit
project path. A desktop launcher starts a default `songbird-project` in its working
directory. No service is enabled and no project migration is performed.

Rollback, with Songbird closed, using the release directory activated above:

```sh
songbird_release="$HOME/.local/share/songbird/releases/0.1.0"
if [ -f "$songbird_release/previous-target.txt" ]; then
    songbird_previous=$(cat "$songbird_release/previous-target.txt")
    ln -sfn "$songbird_previous" "$HOME/.local/bin/songbird"
else
    rm "$HOME/.local/bin/songbird"
    rm -f "$HOME/.local/share/applications/songbird.desktop"
fi
```

Rollback changes only the launcher. Keep the release directories, project data,
recorded takes, and immutable save generations. Version 1 project metadata is
validated on open; unsupported versions fail instead of performing migrations.

## Voicebank Studio acceptance

Use [Creating Your First Voicebank](FIRST_VOICEBANK.md) for the integrated
recording/preparation workflow and [the Studio reference](VOICEBANKS.md) for all
commands. To validate the supplied generated example without hardware:

```sh
./target/release/songbird check examples/voicebank-studio
./target/release/songbird voice-check examples/voicebank-studio voices/studio-synthetic
./target/release/songbird gui-smoke examples/voicebank-studio voices/studio-synthetic
./target/release/songbird render examples/voicebank-studio /absolute/new/studio-check.flac
```

A three-frame Studio smoke is not an interactive recording or quality check.
For hardware acceptance, record M/IY/S (or a complete short phrase's required
phones) using your microphone. Check alternate takes, clipping/quiet warnings,
raw/prepared replay, manual timing/root correction, publication, lyric/pitch/style
coverage, a sung preview, then a harmony preset in a saved composition. Export
WAV/FLAC, reopen/reimport the bank in another project, and listen for intelligibility,
pitch stability, consonants and transition artifacts. Repeat across intended
registers/styles and device rates. Do not use the generated example as a substitute
for these checks. Interrupted recordings remain in the bank's recordings folder;
capture receipts retain their target/profile/path for recovery and manual import.

## Mini-notation and export additions

See [mini-notation](MINI_NOTATION.md) for the opt-in syntax, migration and a generated
example session. See [DAW delivery status](DAW_EXPANSION.md) for pause/resume, region
and stem commands, validation/rollback, and documented hardware/resource limits.

## GUI fix validation and rollback

This pass was source-reviewed and formatted only. Tests, compilation, GUI, input
capture, device disconnection, playback timing, and underruns still require the
operator's validation. No service or host configuration change is needed: launch
Songbird as your desktop user inside niri/Wayland, with the prerequisites above.
On CachyOS, the required development/runtime components include Rust/Cargo, a C
compiler, pkgconf, alsa-lib, libxkbcommon, Wayland/X11 libraries and Mesa/OpenGL.
Use your existing working audio setup and Rust toolchain (README: Rust 1.97.1).

Before building, close Songbird and keep an executable/config backup. From the
repository root, in the same shell (the backup directory is printed):

```sh
songbird_backup=$(mktemp -d "$HOME/songbird-before-gui-fixes.XXXXXX")
songbird_config=${SONGBIRD_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/songbird/config.toml}
if [ -f target/release/songbird ]; then cp -p target/release/songbird "$songbird_backup/songbird"; fi
if [ -f "$songbird_config" ]; then cp -p "$songbird_config" "$songbird_backup/config.toml"; fi
printf '%s\n' "$songbird_backup"
# Fetch only if dependencies are not already cached (requires network):
cargo fetch --locked
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
# Desktop GUI smoke: opens and closes after three frames, with no capture/playback.
./target/release/songbird gui-smoke examples/acceptance
# Activation: run the newly built desktop executable explicitly.
./target/release/songbird examples/acceptance
```

For an offline machine append `--offline` to Cargo commands that resolve packages
and skip fetch after transferring cached/vendor dependencies. Run the smoke and
activation commands from the graphical desktop session, not a headless SSH shell.
Use a copy of a personal project for hardware checks. Source media, user projects,
recordings, presets and voicebanks are not changed by these build commands.

Check these interactions before accepting the release:

1. Type continuously for at least 15 seconds (across draft saves); edit/save theme
   colors repeatedly. Verify unsaved text, cursor/selection, editor focus, scroll,
   undo/redo and font loading remain stable. Invalid TOML must keep the last theme;
   a corrected save must recover. Touch/save unchanged TOML: no panel/font resets.
2. Refresh/select devices, restart the app and verify selections return while the
   microphone remains off. Disconnect a named device; verify the unavailable label
   after Refresh and an actionable error on use. Reconnect and refresh to recover.
3. Record short mono/stereo WAV/FLAC takes with monitoring off, then explicitly on
   with headphones. Stop/finalize, inspect the preserved take and placement, replay
   it, and verify asset peaks. Try an unsupported rate/channel combination: expect
   an error, no crash. Confirm controls prevent duplicate/overlapping recordings.
4. Import WAV/FLAC; verify asset peaks before evaluation. Play synth-only audio at
   140 BPM; confirm changing master PCM peaks, individual track envelopes, waveform
   color and collapse controls. Compare underrun counters with displays open/closed
   over sustained playback; verify pause/resume, loop and live evaluation still work.
   Master shows callback-bound PCM; this cannot prove downstream hardware output.

If the new build fails, close it normally so takes finish. Launch the backed-up
executable (if one existed) and optionally restore the backed-up user config:

```sh
# Use the printed backup path if this is a new shell.
# songbird_backup="$HOME/songbird-before-gui-fixes.REPLACE_ME"
if [ -f "$songbird_backup/config.toml" ]; then cp -p "$songbird_backup/config.toml" "$songbird_config"; fi
"$songbird_backup/songbird" examples/acceptance
```

If there was no previous executable, build your known-good source revision in a
separate checkout and launch it; retain this checkout and all user data. If the
config did not previously exist, keep it or move it into the backup directory
instead of deleting it. Never remove projects, recordings, cache leases in use,
voicebanks, or take indexes as part of application rollback. For interrupted
recordings use the existing `recover-take` workflow in this guide.

## Pitch coach acceptance

No additional packages, permissions, service activation, or network access are
needed beyond the existing audio/desktop prerequisites. Close Songbird before
building and installing. Run from the checkout:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
./target/release/songbird gui-smoke examples/voicebank-studio
# Preserve the currently installed executable before updating it.
cp -p "$HOME/.local/bin/songbird" "$HOME/.local/bin/songbird.before-pitch-coach"
install -m 755 target/release/songbird "$HOME/.local/bin/songbird"
cmp target/release/songbird "$HOME/.local/bin/songbird"
"$HOME/.local/bin/songbird" examples/voicebank-studio
```

If Songbird was never installed, skip the executable backup and launch directly
from `target/release/songbird`. The new tuner/parser tests are written but were
not executed during implementation. Hardware, latency, tracking quality, and
sustained playback/recording performance remain operator checks:

1. Open a test bank, enable the live tuner and explicitly record a short held vowel.
   Confirm detected note/Hz, cent direction, target selection, levels, and history.
   Use an independently known reference near 220/440 Hz; check several pitches and
   your actual device rates, including 44.1/48 kHz. Expect no note for silence or
   unvoiced consonants and a warning/withheld pitch for clipping. Check octave errors.
2. Stop/finalize. Confirm input stops, the original take remains playable, and the
   measured root can be reviewed. Set a manual root, run `voicebank analyze`, and
   verify it stays unchanged. Click **Apply measured root** only when satisfied;
   verify trims/range/audio and published bank samples remain unchanged until Use.
3. Record with the tuner enabled/disabled during backing playback. Compare recording
   drops, output underruns, and captured audio quality. Check Stop, device loss,
   repeated short captures, quiet input, noise, and target changes. A skipped-tuner
   counter must not suppress a genuine recording overrun or device error.
4. Repeat on each supported desktop platform before claiming cross-platform tuner
   acceptance. Earlier build/audio confirmations predate this feature.

To roll back, close Songbird normally, then restore only the saved executable:

```sh
install -m 755 "$HOME/.local/bin/songbird.before-pitch-coach" "$HOME/.local/bin/songbird"
```

All recordings and existing bank metadata remain in place; there is no schema
migration or automatic publication to undo. Root edits made explicitly by the
operator remain saved, as with the existing root-edit command.

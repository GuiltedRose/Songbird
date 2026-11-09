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
cargo clippy --locked --offline --all-targets
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

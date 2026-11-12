# Routing expansion status — 2026-11-12

The complete mixing/plugin/voicebank expansion request is **unfinished**. This
increment has source changes and new regression tests, with no implementer-run
build, test, reference render, GUI, hardware or plugin acceptance. Formatting and
whitespace review alone do not establish buildability or production readiness.

| Milestone | Current source implementation | Outstanding |
|---|---|---|
| 1: mixer | Typed prepared DAG, optional explicit channels, source grouping, nested buses, sends, cycles, checkpoints and meters | Operator regression/PCM/GUI acceptance; bus stems |
| 2: instruments | Shared routes for all track types, drum-event isolation, node gain/pan/effect automation and bypass | Automatic per-instrument splitting within a multi-source track; acceptance |
| 3: DSP | RBJ EQ bands, timed compressor/gate, width and graph sidechains; old effects retained | Sidechain detector filtering/live external ports, improved reverb/limiter, optional multiband; objective acceptance |
| 4: analysis | Offline master/track/node sample peaks, RMS, clipping, correlation and insert gain reduction | Integrated/momentary/short-term LUFS, LRA, true peak, standards conformance, optional spectrum |
| 5: plugins | Format-neutral control/state/MIDI-Learn contracts and mock-backed tests | ALL VST3/CLAP/LV2 binary adapters, discovery/scanning, native/original GUI integration, OS MIDI I/O, plugin routing/PDC, persistence UI and crash isolation |
| 6: voicebanks | Separate phonetics overview and five exact-inventory guides, language-specific recording/checklists and offline Help | Operator review of links/rendering, pronunciation/audio quality and real-bank acceptance |

See [mixing](MIXING.md), [plugin compatibility](PLUGINS.md),
[implementation audit/plan](EXPANSION_PLAN.md) and [operator commands](OPERATIONS.md#routing-expansion-validation-and-rollback).
No statement below about older executed checks applies to these new changes.

---

# Verification and acceptance status

2026-11-09. This repository contains an integrated implementation, not a certified
production release. **The specification's complete definition of done has not yet
been demonstrated.** Real audio hardware and an authorized human English voicebank
are still needed for acceptance; several operational/quality limitations below
also need consideration before relying on it for production work.

## Executed successfully

- `cargo test --offline --locked`: **66 tests passed**, no ignored/failed tests.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: passed.
- `cargo build --release --offline --locked`: passed.
- `cargo fmt --check`: passed.
- Native release GUI smoke: **three updates and clean exit**, using X11 outside
  the filesystem/network sandbox. In-sandbox X11/Wayland connections failed;
  Wayland itself was not verified. This was not an interactive UX acceptance run.
- Generated the six-track [integrated example](../examples/acceptance/README.md),
  opened its saved manifest, decoded its assets, and rendered actual WAV/FLAC.
- Independent reference `flac --test --silent` passed the final master, including
  its PCM checksum. MD5: `648a9d82f6886cd6f867189f3b216d73`.
- Whitespace diff checks passed. No install, activation, deployment, push, service
  startup, or remote repository creation was performed.

Tests inspect audio samples and cover parsing/errors, note names, nested patterns,
repetition, definitions/sections, musical timing, loops, live preroll and effect
history, deterministic oscillators/drums, shared-renderer block-size consistency,
mixing, panning, muting/soloing, automation, effects, non-destructive clip edits,
WAV/FLAC mono/stereo 16/24-bit round trips (including tiny final FLAC blocks),
resampling/conversion, transient encoding bounds/checksums, missing/traversing
assets, bank validation, pronunciation lookup, syllable/note alignment, analytic
vocal output/determinism, save/reopen/recovery, unused-take preservation during
Save As, existing-source protection, and interrupted WAV repair.

The larger example exposed a flacenc 0.5.1 predictive representation exceeding
PCM size by orders of magnitude on two transient frames. Encoding now checks
frame size before bitstream allocation and falls back to bounded verbatim FLAC.
A committed audio regression reproduces the issue and verifies PCM identity.
Final STREAMINFO includes actual frame-size bounds, sample count, and PCM MD5.

## Measured release results

The table below is the earlier pre-Studio baseline; its binary size, RSS and render
times are not measurements of the new Studio feature.

Rust 1.97.1, ALSA development library 1.2.16.1; Linux execution environment on
Intel Core i7-8750H @ 2.20 GHz. Single observations, not a controlled benchmark.

| Measurement | Observed result |
|---|---:|
| Stripped release executable | 11,125,120 bytes (10.61 MiB) |
| CLI `help` process elapsed time | 0.00 s at `/usr/bin/time` precision |
| CLI `help` maximum RSS | 3,928 KiB |
| Three-frame GUI smoke process lifetime | 0.21 s |
| GUI smoke maximum RSS | 74,368 KiB |
| Example duration | 658,286 frames / 48 kHz, about 13.714 s |
| Example WAV total preparation/render/write time | 1.262 s (10.87x duration) |
| Example FLAC total preparation/render/write time | 1.421 s (9.65x duration) |
| Example FLAC size | 1,909,479 bytes |
| Example peak before PCM conversion | 0.278367 |
| Example samples exceeding full scale | 0 |

GUI smoke lifetime is **not** first-frame startup latency. Maximum RSS during
startup is **not** idle memory. Idle CPU, idle GUI RSS, first-frame startup time,
physical audio latency, recording stability, and sustained normal playback were
not measured. Queue capacities are documented but are not latency measurements.
The render benchmark contains analytic singing, imported clips, and instruments;
it does not predict performance for large human banks or long projects.

## Remaining acceptance and implementation limits

| Area | Current evidence and remaining work |
|---|---|
| Physical input/output | Actual CPAL stream/queue/writer implementation exists; `/dev/snd` is absent here. Microphone/line input, stereo channels, overdubbing, monitoring, device loss, and sustained playback need compatible hardware tests. |
| Capture synchronization | Imported/arranged audio shares a sample-indexed transport. Timeline takes use output-clock gating and automatic clip placement with manual latency compensation. Automatic round-trip measurement, synchronized device clocks and physical alignment validation remain outstanding. |
| Monitoring | Uses a second output stream, requires host concurrent-stream support; uncalibrated latency. No monitored-output gain control. |
| Human English singing | Dictionary, bank selection, syllable alignment, consonant playback, vowel PSOLA, vibrato, bends, smoothing and dynamics are implemented. No authorized human bank was supplied. Intelligibility, pitch accuracy, formant quality and useful register range remain unverified. |
| Vocal character creation | Integrated Studio commands/panel support guided recording, take import/selection, waveform/timing edits, local pitch/onset/voicing suggestions, publication, coverage checks, dictionaries, presets, preview, and portable reuse. Suggestions need manual inspection; phoneme identity comes from the chosen target. |
| Synthesis quality | Analytic vowels and mixed audio are audible and deterministic. Root-grid PSOLA is basic, not evidence of natural English coarticulation or mastering-grade output. |
| Editing | Text operations are functional; waveform display assists inspection. Group labels are metadata, not bulk-edit selectors; there are no mouse waveform handles, automatic relinking, or spectral/noise-profile editor. |
| Stretch/effects | Granular stretching/pitch shift and simple native effects are implemented. Extreme shifts and transients can produce artifacts. Denoise is amplitude suppression rather than profile-based spectral subtraction. |
| Live changes | Prepared, quantized replacement is implemented and its preroll tested offline. Real-time boundary behavior needs hardware testing. Replacement is not crossfaded and may click; a missed boundary keeps the original mix and reports the failure. |
| Tempo/signature | Global settings and live reevaluation are supported; within-song instant/ramped tempo and bar-boundary meter maps are now implemented; see the latest DAW entry below. |
| Large sessions | PCM pages/queues and aggregate cache reservations are bounded; orphan-only eviction and native disk preflight are implemented. Quota/failure fixtures pass; sustained hardware and real disk-exhaustion/power-loss testing remain outstanding. |
| Recovery | Atomic generations, prior manifests, drafts, take preservation, and PCM WAV repair are implemented/tested. An export can delay periodic drafts; original media still needs operator backups. |
| GUI workflow | Native window creation/rendering passed. Keyboard completion/history/search, recording controls and a full creative session need manual interaction acceptance. |

The original analytic bank covers only AA; the new Studio example contains
explicitly generated M/IY/S material and a harmony preset. Neither is a complete
human English voicebank or proof of intelligible natural singing. No synthesized
signal is presented as microphone capture. Full singing acceptance still requires
authorized human recordings, listening checks and compatible physical audio devices.
No such recordings were supplied for this feature.

Follow [OPERATIONS.md](OPERATIONS.md) for exact prerequisites, validation,
hardware acceptance, optional installation/activation, recovery, and rollback.
Full production acceptance remains outstanding.

## Native platform follow-up

Windows/macOS config discovery, portable composition paths, Windows save handling,
a Windows launcher and macOS microphone/app metadata have been added. The desktop
GitHub Actions matrix is prepared for native builds/tests/artifacts on both hosts.
It has not been pushed or dispatched here. Linux checks do not establish Windows
or macOS compatibility; native CI results, GUI interaction, microphone permission,
recording/playback, and device-loss checks are pending. Appearance tests now also
cover Windows/macOS config resolution and live theme reload.

Unix saves retain parent-directory fsync. Windows saves sync and close the staged
file before rename; directory fsync is unavailable through this implementation.
Windows power-loss durability therefore needs separate validation.

## Voicebank Studio verification

The added automated cases exercise creation/settings, actual PCM import and source
preservation, capture-context receipts (without hardware verification), pitch and
silence/onset analysis, clipping/quiet/unvoiced diagnostics, manual corrections,
coverage/profile/range failures, English pronunciations/ambiguities, syllable
samples/melismas, metadata updates, legacy bank editing, portable presets/harmonies,
save/export/reimport, corrupted/missing assets, extreme numeric input rejection,
and deterministic multi-phoneme previews through the shared mixer. The generated
example also mixes its preset-driven voice with drums/bass.

Analysis suggestions do not recognize a recorded phoneme or validate a voice's
identity/intelligibility. A capture-start receipt does not certify completion;
take registration follows successful recorder finalization. Source files and
previous metadata remain when the preferred take changes. Bank/studio manifests
are independently atomic; simultaneous crash recovery is not a multi-file transaction.

The feature's real-microphone and intelligible-human-phrase acceptance steps remain
operator checks. Existing Windows/macOS CI jobs include the new tests and package
the Studio example, but were not dispatched here.

Final Studio release checks on the same Linux host passed: native three-frame
Studio-panel smoke, example project/voicebank validation, and an independently
verified FLAC export (`flac --test --silent`). The 8-second, four-track example
(two harmony singers plus drums/bass) rendered 384,000 frames at 48 kHz in
1.826 seconds (4.38x duration), peak 0.148476, with zero samples over full scale.
The FLAC was 1,431,278 bytes; the release executable was 11,538,872 bytes. These
are single-run measurements of generated fixture material, not hardware latency
or human singing quality measurements. No build/activation/rollout on the operator's
monolith, publishing, or remote workflow dispatch was performed.

## Multilingual and procedural increment

Implemented native locale selection, layered UI catalogs, independent language
preferences, discoverable pronunciation profiles/TSV dictionaries, procedural
voicebanks, inventory-aware Studio and offline English language guides, including
Latin voicebanks. Punctuation is separate phrase metadata; note timing remains
explicit. This increment is untested; previous validation counts do not cover it.
No human singing or native Windows/macOS validation was performed. See
[operator checks](LANGUAGES.md#punctuation-and-validation).

## Historical DAW milestone: mini-notation subset (superseded below)

Implemented opt-in cycle-aware mini-notation, deterministic seeds, bounded AST
queries, sample/drum/singer integration, editor assistance, pause/resume, step and
filter automation, region export with preroll, and isolated post-fader stems.
The broader DAW specification remains incomplete; see [delivery status and remaining
work](DAW_EXPANSION.md). In particular there are no tempo/meter maps, marker-aware
transport, punch/count-in recording, aggregate cache quotas or complete timeline UI.

The baseline lockfile/package-version mismatch, Latin inventory-ID validation,
and Studio guidance wording regressions were repaired. The full 80-test suite
passed, followed by the additional pause queue/clock test. Focused mini/render
regressions cover scheduling, resource rejection, PCM equivalence, save/reopen,
region preroll, stems and preservation. Strict all-target Clippy passed. The
original generated example exported 128,000 frames per full FLAC/WAV mix at 16 kHz
and a 64,000-frame region; no human recordings or hardware streams were used.
These checks do not validate the incomplete systems or native Windows/macOS audio.

Final Linux release build and CLI asset check passed. Release benchmark of the
8-second mini example: 128,000 frames at 16 kHz, peak 0.197392, zero over-full-scale
samples, 14.2 ms preparation and 277 ms total (28.87x duration). This is one local
synthetic workload, not a hardware latency measurement or a general performance
guarantee. No remote runner, installation, activation or rollout was performed.

## DAW workflow completion increment

Added the analytic musical clock (tempo jumps/linear ramps and bar-boundary meter
maps), markers/repeated sections, frame-accurate event mapping, bidirectional
prerolled seeking, checkpointed range loops, current musical position display,
transport preferences, input-clock gates, count-in/metronome/pre-roll, punch/loop
recording, numbered non-destructive loop spans and take comping/replay. Added clip
IDs/selection/relinking, musical split/trim, snap/group operations, scalar effect
and vocal automation, per-channel peak feedback, 128-frame transition fades, cache
reservations/orphan eviction and native disk preflight. Guides are embedded offline.

The full 92-test Linux suite passes, including original workstation/voicebank
regressions and new clock/onset-frame, gate/count-in, loop-span, comp, quota and
transport-persistence fixtures. Strict all-target Clippy and release build pass.
The expanded generated song renders 210,051 frames per FLAC/WAV mix at 16 kHz,
peak 0.19739243 and zero clipped samples, plus a 64,000-frame region and five
isolated stems. Its clock includes 120→140 BPM, a linear ramp, 6/8 and a marker;
its vocals and imported clip are original procedural fixtures.

No claim of physical microphone accuracy, synchronized hardware clocks, natural
human singing or native Windows/macOS operation is made. These require operator
acceptance. Loop editing/seeking use prepared stream restarts and may have short
hardware gaps. Source/lease/recording data are preserved; no rollout or remote
workflow dispatch occurred. See [workflow and limits](DAW_EXPANSION.md).

Final generated-session acceptance additionally compared every decoded FLAC/WAV
sample with a fresh renderer after project reopen: 210,051 frames matched within
24-bit quantization tolerance. The release CLI asset check passed. One release
benchmark prepared in 12.8 ms and rendered in 319 ms total (41.19x duration), peak
0.197392 and zero clipped samples; this is a synthetic single-workload measurement.
The current native GUI smoke attempt could not connect to an X server in this
execution environment. GUI interaction and hardware acceptance therefore remain
unverified for this increment; the display failure is not a demonstrated code
failure. No Windows/macOS runner or rollout was executed.

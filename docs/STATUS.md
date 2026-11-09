# Verification and acceptance status

2026-11-09. This repository contains an integrated implementation, not a certified
production release. **The specification's complete definition of done has not yet
been demonstrated.** Real audio hardware and an authorized human English voicebank
are still needed for acceptance; several operational/quality limitations below
also need consideration before relying on it for production work.

## Executed successfully

- `cargo test --offline --locked`: **33 tests passed**, no ignored/failed tests.
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
| Capture synchronization | Imported/arranged audio shares a sample-indexed transport. New takes need explicit placement; automatic input-latency compensation and shared-clock capture alignment are not implemented. |
| Monitoring | Uses a second output stream, requires host concurrent-stream support; uncalibrated latency. No monitored-output gain control. |
| Human English singing | Dictionary, bank selection, syllable alignment, consonant playback, vowel PSOLA, vibrato, bends, smoothing and dynamics are implemented. No authorized human bank was supplied. Intelligibility, pitch accuracy, formant quality and useful register range remain unverified. |
| Vocal character creation | Portable TOML banks and sample labels are supported. Root pitch, timing, phoneme/style/register labels and dictionary edits are manual. There is no automated bank analysis or specialized bank-labeling GUI. |
| Synthesis quality | Analytic vowels and mixed audio are audible and deterministic. Root-grid PSOLA is basic, not evidence of natural English coarticulation or mastering-grade output. |
| Editing | Text operations are functional; waveform display assists inspection. Group labels are metadata, not bulk-edit selectors; there are no mouse waveform handles, automatic relinking, or spectral/noise-profile editor. |
| Stretch/effects | Granular stretching/pitch shift and simple native effects are implemented. Extreme shifts and transients can produce artifacts. Denoise is amplitude suppression rather than profile-based spectral subtraction. |
| Live changes | Prepared, quantized replacement is implemented and its preroll tested offline. Real-time boundary behavior needs hardware testing. Replacement is not crossfaded and may click; a missed boundary keeps the original mix and reports the failure. |
| Tempo/signature | Global settings and live reevaluation are supported; no within-song tempo map. |
| Large sessions | PCM pages and audio queues are bounded, but total disk cache has no quota/eviction or low-disk preflight. Stress, disk exhaustion and power-loss testing remain outstanding. |
| Recovery | Atomic generations, prior manifests, drafts, take preservation, and PCM WAV repair are implemented/tested. An export can delay periodic drafts; original media still needs operator backups. |
| GUI workflow | Native window creation/rendering passed. Keyboard completion/history/search, recording controls and a full creative session need manual interaction acceptance. |

The included voicebank covers only an analytic AA vowel. It cannot synthesize
arbitrary English lyrics. It is openly labeled as a test/example instrument;
no synthesized signal is presented as a human microphone recording. A complete
15-item acceptance arrangement requires real human recordings and a suitably
covered bank, followed by save/reopen and dual-format export comparisons.

Follow [OPERATIONS.md](OPERATIONS.md) for exact prerequisites, validation,
hardware acceptance, optional installation/activation, recovery, and rollback.
Full production acceptance remains outstanding.

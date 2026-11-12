# Architecture

One Rust crate; one native egui/eframe OpenGL application. Core modules do not
require GUI state. `main` also exposes offline CLI operations.

- `language`: bounded lexer, validated composition model, recursive pattern
  expansion, musical timestamps, reusable patterns/instruments and sections.
- `codec`: streaming WAV/FLAC decode to temporary file-backed PCM, paged reads,
  windowed-sinc resampling, streaming encoders, unique files, path checks, recovery.
- `dsp`: deterministic synthesis, ADSR, pitch/volume controls, filters and effects.
- `studio`: guided bank creation, immutable takes/capture receipts, editable metadata,
  coverage, portable performance presets, and preview preparation.
- `voice_analysis`: bounded local silence/level/YIN pitch and stable-region suggestions.
- `voice`: portable bank validation, embedded English dictionary lookup, syllable
  alignment, recorded-phone playback and pitch-synchronous vowel overlap/add.
- `engine`: one sample-indexed multitrack renderer, shared by playback and export.
- `audio`: CPAL device abstraction, lock-free SPSC queues, hardware streams,
  recording writer, meters, device failure flags and quantized replacement.
- `project`: atomic, versioned manifests, immutable composition generations,
  recovery drafts, asset copies, Save As and evaluation history.
- `gui`: editor, highlighting, prompt/completion/history, track/asset overview,
  waveform peaks, diagnostics, background jobs, transport and meters.

## Threads and buffers

A GUI background worker performs preparation, import, saves, exports and bank
validation. Each playback has a feeder rendering to a fixed-capacity SPSC audio
ring. The native output callback only consumes frames, converts/clamps sample
formats, and updates atomics. Each recording has an input callback feeding a
second ring and a writer thread. FLAC compression follows capture finalization
on that writer. Monitoring uses another ring/output stream. Encoding is in Rust;
no `flac` subprocess is used by the application.

Audio callbacks have no explicit locks, heap allocation, file operations, codec
work, GUI calls, network access, or voice analysis. Queue elements are fixed-size
stereo float arrays. Expensive work runs outside callbacks; missing output yields
silence and an underrun counter, not fabricated source material or a successful
recording label. Dropped input rejects take completion. Hardware errors set flags
and stop work; partial capture files remain recoverable. Queue capacities:

| Queue | Capacity |
|---|---:|
| Playback PCM | 16384 stereo frames |
| Recording PCM | 32768 stereo frames |
| Monitoring PCM | 8192 stereo frames |
| GUI worker jobs/results | 2 each |
| Pending prepared live renderer | 1 |

Output starts after up to 4096 prefilled frames. These capacities favor robustness,
not low monitoring latency. The hardware buffer size is device-default; these
values must not be reported as measured latency. Input/output callbacks and host
streams have independent timing. Capture gating follows the audible frame clock;
manual placement compensation is available, while hardware clock synchronization
and automatic latency measurement remain unimplemented.

Live preparation reconstructs effects up to a future bar on the GUI worker.
The playback feeder swaps already-prepared state at that sample index; it does
not decode assets or fast-forward the replacement there. A missed deadline keeps
old playback and reports failure. Preparation has a bounded retry count. A new
sample rate requires a new stream. Worker failure is surfaced and assets are
owned/dropped outside callbacks. No audio-DSP memory is dropped by the callback.

## Storage and limits

Internal processing is f32 stereo PCM. Source mono is duplicated for mixing;
export is stereo, recording is selectable mono/stereo. WAV/FLAC integer output
supports 16/24 bit. FLAC writes 4096-frame encoded blocks and finalizes STREAMINFO
sample count and PCM MD5. It does not retain an entire compressed song in RAM.
Cache PCM uses float little-endian samples, with at most one 8192-frame page per
active asset plus reduced waveform peaks. WAV float32 and integer PCM, and FLAC
mono/stereo at 8–96 kHz are accepted. Composition rates are a documented subset.
WAV RIFF size is limited by the WAV library/format; RF64 is not implemented.

Limits: 1 MiB composition and voicebank manifests, 64 KiB project manifest,
128 tracks, 512 clip/drum assets, 16 singer tracks, 256 total effects, 65536
expanded events per track and 131072 per project, 16 recursive pattern levels,
128 repetitions, up to 4096 bars but at most one hour of arrangement time.
Voicebanks allow up to 2048 sample entries each. Individual decoding is bounded
at 691200000 interleaved samples (one hour at stereo 96 kHz); at lower rates this
sample-count cap permits longer files. Disk-cache reservations are aggregate and
bounded. Native free-space preflight and orphan-only PCM eviction are implemented
in `storage`; active leases, original media and voicebank samples are never evicted.

Source paths are project-relative; canonical containment rejects traversal and
escaping symlinks. Output creation uses create-new; imported audio is copied into
unique project assets. Saves retain old generations and takes. Projects/scripts
are data, not executables. Runtime uses no cloud APIs or network services.
Path checks do not provide an OS sandbox against another process racing filesystem
symlinks. Do not share writable project directories with hostile concurrent actors.

## Quality bounds

Effects are basic native DSP. Granular clip stretching/pitch shifting and root-grid
vocal PSOLA require listening validation; neither is a claim of mastering-grade
processing. The denoiser is amplitude attenuation, not spectral-profile removal.
Lyrics use dictionary lookup and heuristic syllabification, not a trained language
model. No human training data or model-qualification machinery is introduced.

Voicebank Studio adds no dependencies or service processes. Preparation/analysis
and file mutations use the existing GUI worker; a small capture-start receipt is
written on the control thread after stream creation. Audio callbacks remain free
of analysis, encoding and file operations. Studio limits take duration to 30 s
and decoded input to 5,760,000 interleaved samples. Pitch uses at most 64 windows;
suggestions are optional metadata, not phoneme recognition. Preset harmony expands
within existing track/singer/effect limits. Vocal units are indexed by start time
and cumulative end time so inactive phonemes are not scanned on every sample.

## Mini-notation increment

`mini` compiles bounded immutable ASTs and queries musical cycle ranges during
preparation. Track seeds and deterministic coordinate hashes govern random events.
`engine` consumes the same Event representation as legacy patterns. `export` uses
that renderer for region preroll and post-fader isolated stems. Pause gates queue
consumption without dropping queued PCM or advancing the audible frame counter.
`timeline` integrates tempo segments analytically and maps meter-aware bars,
beats, seconds and frames. Prepared schedules convert event boundaries once.
Renderer checkpoints retain oscillator/vocal/effect state for rewind and loops.
`recording` supplies an atomic capture gate and bounded loop-span metadata;
`storage` uses native free-space checks, cross-process cache reservations and
orphan-only eviction. See [workflow limits](DAW_EXPANSION.md).

## Prepared native routing and staged plugin contracts

`mixer` compiles explicit source ownership, typed destinations, send taps and
sidechain dependencies into a bounded deterministic DAG during Renderer preparation.
Existing tracks stay automatic channels. Node control automation resolves insert
indices before processing; fixed frame arrays carry signals without per-frame
allocation. Renderer checkpoints include bus DSP; meter history is not rewound
with DSP. The old sum path remains when no new routing feature is used.
`analysis` measures the shared renderer offline; it does not approximate LUFS or
true peak. `plugins` defines format-neutral control/state contracts with mock
coverage; it has no binary backend or audio-graph integration. See [current
limitations](STATUS.md) before describing any of this increment as validated.

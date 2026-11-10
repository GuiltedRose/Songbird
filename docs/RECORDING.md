# Timeline recording, calibration and takes

Select input/output devices in **Audio devices & recording**, or use
`devices`, `input NAME`, `output NAME` (`default` selects System Default); set
`channels 1|2`. Use headphones before `monitor on` to avoid acoustic feedback.
Monitoring is explicit and remains audible during count-in/pre-roll. The click is
mixed into output, not digitally into the input recording; microphones can still
pick it up from speakers.

```text
count-in 1
pre-roll 1
metronome on
latency 0
record-at verse @chorus bar:5:1
```

`record-at NAME START [END]` starts arrangement playback, adds a count-in, plays
pre-roll where available, and gates input between punch-in and punch-out. End
defaults to arrangement end. Count-in is heard before pre-roll; its musical timing
uses the clock preceding that position (before beat zero it uses the initial BPM).
The audible arrangement clock does not advance during count-in. Pre-roll starts
at an earlier bar boundary, clamped to arrangement start. Capture automatically
finalizes at punch-out; `record-stop` can finish early. An input device that arms
after punch-in is rejected: increase count-in/pre-roll rather than accepting a
silently shortened take.

`record NAME [flac|wav]` remains available for immediate recording. It records a
unique source take and estimates placement from the current transport/cursor.
For overdubs use `record-at` instead: immediate input-device startup adds an
unmeasured alignment delay. Timeline finalization indexes the take and appends an
import track automatically. `save` persists the changed composition.

## Input latency calibration

`latency MS` is a manual **placement compensation**, not a claim of synchronized
host input/output clocks. Positive values place recorded audio earlier; negative
values place it later. Placement clamps at arrangement zero. Send a known click
through a physical loopback or record it with your intended microphone setup,
inspect the offset against the intended click position, and enter that offset.
Repeat at the song's sample rate and device/buffer settings. Device callback
scheduling, independent clocks and drift remain hardware dependent. No automatic
round-trip measurement or CPAL timestamp clock synchronization is implemented.

The gate follows the audible output frame counter. Input callback blocks can
straddle that counter's updates, so boundaries have hardware-buffer uncertainty;
this is not sample-exact cross-device punch recording. Verify onset/end alignment
and adjust clip source trim/placement after a real test take. Long sessions may
need separate drift correction; manual latency is a constant offset only.

## Loop takes and comping

```text
range @chorus bar:5:1
record-loop chorus
record-stop
takes
take-replay 2
take-use 2 @chorus
take-use 3 6s 0.2s 0.8s
```

Loop recording keeps one continuous original WAV (and an optional FLAC copy) plus
a `.loops.toml` span index. `recordings/takes.toml` numbers passes without replacing
media. Only the first pass is placed automatically; select another using
`take-use ID [AT [SOURCE_START SOURCE_END]]`. Existing clips remain until you edit,
mute or delete their references. Build a comp with several trimmed take-use clips
at chosen positions and use clip fades/crossfades to join them. Reusing a take never
rewrites its original media. No automatic best-take ranking or destructive comp is
performed. Explicit trims refer to the continuous original file, not a pass-local
zero.

An input callback that misses entire loop boundaries rejects finalization and
preserves the WAV; use longer loops or smaller hardware buffers. Up to 1024 spans
and 4096 indexed takes are allowed. Buffer overflow/device loss also reports an
error and preserves partial audio. Recover interrupted WAVs using `songbird
recover-take FILE.wav`. There is no automatic deletion of unused original takes.

Settings persist in the normal config under `[recording]`: `latency_ms`,
`count_in_bars`, `pre_roll_bars`, `metronome`. Count-in/pre-roll are bounded to
0..8 bars, latency to ±2000 ms. Pause preserves the transport clock; gated capture
waits during pause, while monitoring remains active. Stop recording before seeking,
changing ranges or restarting looping playback.

## Configuration settings

These settings belong in the user `config.toml`, not the composition. GUI audio
preferences are saved atomically, preserving other values; invalid TOML is never
overwritten. TOML comments/formatting may be normalized when preferences are saved.
Monitoring is deliberately session-only. Device changes affect future streams;
active capture/playback is not restarted by a config save.

| Setting | Meaning |
| --- | --- |
| `voicebank.default_language = "auto"` | Default singing language when the composition/project does not override it. Auto follows language detection with English fallback; existing banks retain their metadata. |
| `voicebank.default_inventory = "auto"` | Default phoneme inventory (the set of singing sounds). Auto uses the selected language profile. Explicit inventories must match available profiles. |
| `recording.latency_ms = 0.0` | Manual clip placement correction in milliseconds, from -2000 to +2000. Positive moves clips earlier, clamped at zero. Measure on your hardware; this does not synchronize device clocks. |
| `recording.count_in_bars = 1` | 0–8 bars counted before timeline recording/pre-roll. Immediate GUI Record and `record` start capture directly. |
| `recording.pre_roll_bars = 0` | 0–8 arrangement bars played before punch-in, clamped to arrangement start. Input is gated until punch-in. |
| `recording.metronome = false` | Enable the timeline output click. It is not digitally recorded, but a microphone can pick it up from speakers. |
| `storage.cache_bytes = 4294967296` | 4 GiB limit for regenerable decoded PCM caches, including active reservations. Source media and recordings are never cache-evicted. |
| `storage.minimum_free_bytes = 67108864` | 64 MiB disk headroom required by capture/decoding checks. Insufficient disk space produces an error rather than bypassing the limit. |
| `audio.input`, `audio.output` | Exact CPAL device names. Omitted means System Default. Unavailable named devices produce an error rather than silently switching devices. |
| `audio.channels = 1` | Mono (1) or stereo (2) input; device must support it at the project sample rate. |
| `audio.format = "flac"` | FLAC or WAV preference for immediate recording. FLAC finalization retains the original WAV. |

# Code-first routing and native mixing

This increment is implemented but **not build/test/hardware validated**. Read
[acceptance commands](OPERATIONS.md#routing-expansion-validation-and-rollback)
before using it on production projects. The composition editor remains primary;
no graphical mixer, plugin window, or JavaScript runtime is required.

## Existing tracks remain automatic channels

Synths, Mini patterns, drumkits, imported/recorded clips, samples and singing use
one Renderer. Every track already owns gain, equal-power pan, mute/solo, ordered
inserts and automation. With no new routing declarations, the original summation
path and original DSP algorithms are retained. New mixer stages are opt-in.

```text
track bass {
    synth saw
    notation mini
    notes "c2 ~ g2 bb2"
    cycle 4
    gain -9db
    route music
}
bus music {
    highpass 40 0.707
    bell 250 -2 1.2
    route master
}
master limit -1
```

All graph names are project-global, unique and case-sensitive. `master` is
reserved. New `channel` and `bus` blocks live at project scope, not inside a
`mixer` wrapper or a section. Section track names include their section prefix;
source references must use the resulting complete name.

## Signal flow and controls

Track instruments/clips → existing track inserts → existing equal-power track pan
→ pre-fader send tap → track gain → post-fader send/direct route → explicit channel
or bus inserts → mixer balance → pre-fader send tap → mixer gain → post-fader
send/direct route → master inserts → device/export conversion and clamping.

Existing track center pan remains its original equal-power behavior. New explicit
channel/bus pan is stereo **balance**, unity at zero; a neutral bus does not apply
a second center-pan attenuation. Track gain controls its source; explicit channel
gain controls the sum of its sources; bus gain controls that group's sum. These
are serial stages, not synonyms or hidden replacements.

`send DESTINATION GAIN [pre|post]` is additive to the direct route. Omitted mode is
post-fader. Gain accepts the existing linear or `-12db` spelling; new sends are
bounded to linear 0–4. Pre-fader here means **after inserts/pan, before gain**,
not pre-insert. Muted/solo-excluded tracks emit neither direct signal nor sends.

```text
track lead {
    synth triangle
    notes "C4:1 E4:1 G4:2"
    send ambience -12db post
    route music
}
bus ambience {
    reverb 1
    gain -6db
    route music
}
bus music { route master }
```

Set an auxiliary reverb/delay to wet-only when appropriate: the original source
already follows its direct route. Sends to master are allowed and intentionally
add another copy; they are not automatically deduplicated.

## Explicit channels, grouping and nested buses

```text
track lead_a { synth saw; notes "C4 E4 G4"; gain -12db }
track lead_b { synth triangle; notes "C5 E5 G5"; gain -18db }
channel leads {
    source lead_a
    source lead_b
    gain -3db
    pan -0.2
    send room -15db
    route music
}
bus room { reverb 1; route music }
bus music { compressor -18 2.5 15 100 6 0; route master }
```

A source is a **track name**, not an instrument-definition name or a filename.
Several tracks can feed one explicit channel. A track belongs to at most one
explicit source channel; that replaces its default direct route to master.
An explicit track `route` must agree with its assigned channel, otherwise
preparation rejects the ambiguity. Buses receive inputs through routes/sends;
`source` is reserved for explicit channels. Nested buses provide channel groups.

Channels/buses support gain, pan, mute, solo, inserts, bypass, routing and automation.
A soloed bus/channel admits its upstream track sources; ordinary track solos also
apply. Downstream buses still process admitted signals. Mute removes a node's
output, including sends. Detector inputs follow these same mute/solo choices.

## Independent drum mixing

The simple kit remains unchanged. For separate processing, duplicate its pattern
into dedicated tracks and isolate one event label in each:

```text
track kick {
    instrument drumkit
    notation mini
    pattern "kick snare [hat hat] clap"
    drum-only kick
    gain -9db
    route drums
}
track snare {
    instrument drumkit
    notation mini
    pattern "kick snare [hat hat] clap"
    drum-only snare
    gain -12db
    route drums
}
bus drums { highpass 30 0.707; route master }
```

Add hat/clap or custom sample labels using the same method. `drum-only LABEL`
requires a drumkit and a built-in kick/snare/hat/clap or declared custom drum label.
It suppresses other drum contributions while retaining the original event clock
and loop duration. Do not also play an unfiltered copy unless doubling is intended.
Instrument definitions can still be reused; put routing/mix settings on the track
that uses them. Native and Mini tracks use the same graph.

## New native inserts

Insert order is statement order. These **add** syntax; legacy `eq`, `compress`,
`filter`, `gate`, `denoise`, `delay`, `reverb`, and `limit` retain their algorithms.
Arguments below are plain numbers: frequency in Hz, gain/threshold in dB, times in
milliseconds. Do not append `db` or `ms` inside these effect argument lists.

| Syntax | Meaning |
|---|---|
| `highpass HZ Q` | Second-order high-pass |
| `lowpass HZ Q` | Second-order low-pass |
| `bell HZ DB Q` | Peaking EQ band |
| `lowshelf HZ DB Q` | Low shelf with Q damping |
| `highshelf HZ DB Q` | High shelf with Q damping |
| `compressor THRESHOLD RATIO ATTACK RELEASE KNEE MAKEUP` | Stereo-linked peak detector; attack/release in ms, knee/makeup in dB |
| `noisegate THRESHOLD ATTACK RELEASE HOLD` | Smoothed stereo gate with ms timing |
| `width AMOUNT` | Mid/side width; 0 mono, 1 unchanged, 2 doubled side |
| `bypass INDEX` | Bypass one insert, indexed from 1; multiple declarations allowed |

EQ bands accept 20–20000 Hz, Q 0.1–20 and gains -24–24 dB. Coefficients guard
frequencies near Nyquist and use double-precision state; coefficient changes
smooth over approximately 5 ms. Several bands are several ordered statements.
These follow the [RBJ/W3C EQ cookbook](https://www.w3.org/TR/audio-eq-cookbook/).
Compressor threshold is -90–0 dB, ratio 1–100, attack 0.1–2000 ms, release
1–5000 ms, knee 0–24 dB and makeup -24–24 dB. Gate threshold is -100–0 dB,
attack 0.1–2000 ms, release 1–5000 ms and hold 0–5000 ms.

## Sidechains and automation

```text
track kick { instrument drumkit; pattern "kick - kick -"; gain -6db }
track bass { synth saw; notes "C2:4"; gain -12db }
channel ducked_bass {
    source bass
    sidechain kick
    compressor -24 4 2 120 6 0
    route master
}
```

`sidechain SOURCE` belongs on an explicit channel/bus and can reference a track or
another explicit node. It supplies detector audio to new compressor/noisegate
inserts without adding that audio to the channel. Missing sources and dependency
cycles are rejected, including cycles formed only by sidechain links. This enables
Mini/native kick ducking or vocal-driven music ducking. Live external sidechain
ports and separate detector filtering are not implemented. Route a recorded input
track if that meets the project workflow; this is not a live hardware sidechain.

Gain/pan/filter and one-based `effect.INDEX.ARG` automation are shared with tracks:

```text
bus music {
    bell 250 -2 1.2
    automate gain "0b=0.5 8b=1" linear
    automate effect.1.2 "0b=-2 8b=-6" linear
    route master
}
```

Time is arrangement time on explicit nodes; musical points follow the score clock.
Automation retains existing argument validation, including rejecting live delay
buffer-size automation. Bypass is static per prepared composition, not an automated
plugin switch. Each evaluated graph is prepared outside CPAL and uses the existing
safe renderer replacement boundary; edits are not patched into an active callback.
Adding/removing/reordering explicit nodes requires stopping playback so live meter
identities cannot be mislabeled. Edits within the existing ordered node identities
use the normal prepared replacement boundary.

## Bounds, state, meters and exports

The new graph permits at most 64 explicit nodes, 32 sends per source and 512 route
edges, alongside the existing 128-track, 256-effect and 64 MiB effect-state limits.
Explicit nodes allow at most 32 automation lanes each.
Order is deterministic; all feedback is rejected, even zero-gain cycles. Graph
state participates in seek/loop checkpoints. No graph compilation, allocation,
name lookup or file I/O happens in CPAL. Existing renderer file streaming and
preparation remain on their existing workers.

Implemented native nodes add **zero algorithmic latency frames**: no lookahead or
plugin pipeline is present. Delay/reverb are intentional audio effects. Arbitrary
latency reporting/compensation for plugins remains unimplemented.

The track panel shows live explicit-node sample peaks and over-full-scale frame
counts. Values come from the feeder and can lead audible output by the existing
queue. `analyze` in the prompt, or `songbird analyze PROJECT` in a terminal,
measures the full shared render offline: master sample peak, RMS, over-full-scale
frames, correlation, track/node peaks, and maximum new-compressor insert gain
reduction. Silence/empty-channel correlation is undefined. These are pre-device
clamp measurements. **RMS is not LUFS; sample peak is not true peak.** Integrated,
short-term and momentary LUFS, LRA, oversampled true peak and spectrum are pending.
No BS.1770/EBU R128 compliance is claimed.

Existing stem exports remain isolated post-track-insert/post-fader stems without
master or new bus/channel processing. They explicitly remove graph routes so
other buses/detectors cannot leak into isolated exports. Bus stems and processed
sidechain-aware stems remain future work. Main WAV/FLAC/region renders include
the graph through the shared renderer.

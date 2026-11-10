# Editing, automation, mixing and storage

Text remains the primary editor. Every operation is non-destructive to source
media. Imported clips receive `clip-1`, `clip-2`, … identifiers; override with
`clip-id NAME` and address any imported clip using `select-clip NAME|INDEX`
(one-based indices). Existing last-clip workflows continue to work.

```text
track comp {
    import "recordings/take.wav"
    clip-id lead
    import "recordings/alternate.wav"
    clip-id alternate
    select-clip lead
    trim 0.2s 2s
    place 8b
    fade in 10ms
    group vocals
    snap 0.25b
    select-clip alternate
    place 12b
    group vocals
}
```

`trim`/`crop` and `split` use **source** time. `split-at POSITION` and
`trim-at START END` use timeline positions relative to track start (markers and
bar positions are absolute). Timeline edits are resolved after placement/snap and
other clip directives. They address imported clips selected when the directive
was written; for further editing a generated split half in the same block, use
explicit duplicate/source-trim directives instead. `duplicate POSITION`, `delete`,
`relink PATH`, reverse, normalize, stretch and pitch act on the selected clip.
`crossfade DURATION` joins the last two clips and requires explicit trim. Snap is
in musical beats; `snap off` disables it.

`group-move GROUP OFFSET` and `group-duplicate GROUP OFFSET` operate on all clips
with that group. Offsets are nonnegative elapsed durations; move backward by
editing the clips' placement directly. Originals remain available. A missing relink
path is diagnosed during preparation; valid playback stays active on failure.

## Automation and mixing

```text
track keys {
    synth saw
    notation mini
    notes "c4 e4 g4"
    loop
    gain 0.12
    pan -0.2
    filter 1500
    delay 0.15 0.2 0.15
    automate gain "0b=0.05 8b=0.2" linear
    automate pan "0b=-0.5 4b=0.5" step
    automate filter "0b=500 8b=2500"
    automate effect.2.2 "0b=0.1 8b=0.4"
}
```

Automation supports gain, pan, bend, filter, formant, dynamics, vibrato,
breathiness and vocal-pitch. Vocal controls affect singers. `effect.INDEX.ARG`
uses one-based effect/argument indices in the displayed order. Values are validated
against the existing effect's range. Allocation-changing delay-time automation is
rejected; feedback/wet and other scalar effect arguments are supported. Filter
controls all track filters. Default interpolation is linear in elapsed seconds
between converted control points; step holds the preceding value. Beat points map
through tempo changes before interpolation. There are no additional curve types.

Effects run in source order. Gain/pan, mute and solo remain explicit track settings;
post-fader channel peaks and clipping indicators appear with effect order/arguments
in the track panel, alongside master/input meters. Leave headroom (the example uses
small channel gains) and inspect peak/clipping reports. Existing fixed track/master
effects remain shared by playback and export; there is no arbitrary bus graph.

## Disk and cache recovery

`[storage]` config keys are `cache_bytes` (default 4 GiB) and
`minimum_free_bytes` (default 64 MiB). Each decoded PCM file reserves its bounded
size before decoding. Quotas account for active reservations across processes.
Only orphan `pcm-*.raw` files with **no** `.reserve` lease can be automatically
evicted, oldest first; leased PCM is retained. Source WAV/FLAC, recordings,
voicebanks, dictionaries and project generations are never cache-evicted.

A crash can leave `.reserve` files or `cache/.budget-lock`. Close **all Songbird
instances using that project**, back up the project, then remove only the project's
regenerable `cache/` contents to recover. Never remove reservations while playback
or another instance is using them. Active budget contention reports an error
instead of bypassing the quota. Unknown-length FLAC reserves its decode bound;
convert it to a known-length file if conservative reservation exceeds disk/quota.

Mix/region/stem exports preflight disk space conservatively. Recording checks at
startup and every five captured seconds, with a one-hour ceiling. Disk exhaustion
or device loss preserves new partial output and reports failure; do not treat it
as a completed take/export. Disk checks cannot reserve free space against unrelated
processes writing to the same volume. Use the project/WAV recovery commands in
[operations](OPERATIONS.md), and keep backups of original media.

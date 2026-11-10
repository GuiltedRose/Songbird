# Musical time, tempo, meter and transport

Musical positions use f64 quarter-note beats; display resolution is **960 ticks
per quarter note**. Absolute elapsed seconds and integer sample frames are distinct.
Recorded clip `trim`/`split` source positions remain source seconds, unaffected by
tempo. Scheduling converts musical event boundaries through one analytic clock;
playback/export use the same rounded sample timeline.

```text
tempo 120
signature 4/4
bars 8
marker chorus at 8b
tempo at @chorus 140
tempo ramp 16b 24b 140 120
signature at bar 5 6/8
section chorus at @chorus {
    track bass { notation mini; notes "<c2 g2>"; loop }
}
section refrain at 16b repeat 2 every 3b {
    track accent { notation mini; notes "c4 ~ ~"; cycle 3; gain 0.1 }
}
```

Markers used by tempo/section directives must be declared earlier. Meter changes
are allowed only at numbered bar boundaries; an inside-bar change is rejected.
In 6/8 a bar has three quarter-note beats, while the displayed beat counts six
eighth-note units. Meter never changes BPM; BPM always counts quarter notes.

Tempo jumps are instant. A ramp is **linear BPM as a function of musical beat**,
not linear seconds: the clock integrates 60/BPM and uses its analytic inverse.
Ramp start/end BPM are explicit; the start can itself be a jump. Overlapping ramps,
duplicate changes, nonpositive spans and BPM outside 20..400 are rejected. Linear
is the supported ramp curve. Meter and tempo maps are independent; `bars` uses
the meter map to determine the final musical position.

Track/section `at Nb`, clip `place Nb`, and automation `Nb=VALUE` use this clock.
Track automation/clip placement beats are relative to track start; section repeat
spacing is musical. Seconds stay absolute elapsed durations. Legacy scores without
maps retain their prior placement conversion behavior. Declare initial global
settings before tracks for predictable authoring.

The prompt accepts `8b`, `1.5s`, `250ms`, `00:01:02.250`, `@chorus`, and
`bar:5:2:240` (one-based bar/beat, optional tick). A tick must fit inside the active
notated beat. `seek POSITION` works in either direction. `jump NAME` selects a
marker; `restart`/`home` clears range looping and returns to arrangement start.
The transport shows current tempo/meter, elapsed/total/remaining time and musical
position. A selected range constrains seeking: clear it to seek elsewhere.

`range START END`, `loop on|off`, `pause`, `resume`, `play` and `stop` are separate
from pattern `loop`. Arrangement loops restore a prepared effect/oscillator/vocal
checkpoint; a short fade reduces the boundary click. Their cached state is bounded;
no parsing or preroll happens in audio callbacks. A replacement that cannot prepare
leaves old playback valid. Live loop edits use a prepared stream restart and may
have a short gap. Recorded source time is never stretched automatically by a tempo
map; use explicit clip stretch when desired.

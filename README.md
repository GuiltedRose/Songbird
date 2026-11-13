# Songbird

Songbird is an offline music workstation with native Linux, Windows, and macOS build targets. Write music in a code editor,
record or import audio, mix tracks, and export WAV or FLAC—all in one native app.

## Make music with code

Build a beat, add a bass line, write chords and a melody, then arrange your parts
into a song. Record your own performances alongside them and export the mix.
Your composition is an editable score: you decide what plays, how it sounds, and
when each part enters.

Start with the [complete songwriting walkthrough](docs/MAKING_MUSIC.md). It teaches
the musical decisions behind the code and takes you from a repeating groove to
a sixteen-bar arrangement with an intro, verse, ending, and recorded performance.
The accompanying [Night Walk project](examples/songwriting/composition.sb) is
ready to open:

```sh
./target/release/songbird examples/songwriting
```

Use `target/release/songbird.exe` on Windows. Press **Ctrl+P** to listen,
**Ctrl+E** to edit, and **Ctrl+Enter** to apply changes. The sections below cover
setup and the everyday recording, editing, and export workflow.

Keep these references nearby as you compose:

- [Composition language and commands](docs/LANGUAGE.md)
- [Create your first voicebank](docs/FIRST_VOICEBANK.md)
- [Voicebank commands, vocal presets, and pronunciation overrides](docs/VOICEBANKS.md)
- [English pronunciation dictionary](data/cmudict.dict) and [dictionary guide](data/README.md)

## Build on Linux and open the example

Use Rust 1.97.1 and Cargo, a C linker, `pkg-config`, ALSA development files, and
a Linux desktop with OpenGL and X11 or Wayland.

On Debian or Ubuntu, install the native build dependencies:

```sh
sudo apt install build-essential pkg-config libasound2-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev libgl1-mesa-dev libwayland-dev libxkbcommon-dev
```

From the repository directory:

```sh
cargo fetch --locked
cargo build --release --locked --offline
./target/release/songbird examples/acceptance
```

The first dependency download needs internet. After that, Songbird works offline.
Press **Ctrl+P** to hear the example. It combines drums, bass, melody, imported
audio, and a synthetic vowel voicebank. Press **Ctrl+P** again to stop.

## Windows and macOS

The [desktop workflow](.github/workflows/desktop.yml) builds, tests, and packages
Windows and macOS on GitHub-hosted runners after pushes and pull requests. You can
also run it from **Actions → Desktop builds → Run workflow** once it is on the
default branch. Download the matching artifact from a successful run. The macOS
archive contains `Songbird.app`; Windows includes `Start-Songbird.cmd`.

To build locally, use Rust 1.97.1 with **Visual Studio C++ Build Tools and the
Windows SDK** on Windows, or **Xcode Command Line Tools** on macOS:

```sh
cargo fetch --locked
cargo build --release --locked --offline
```

Launch `target/release/songbird.exe examples/acceptance` on Windows, or
`./target/release/songbird examples/acceptance` on macOS. Native audio uses WASAPI
on Windows and CoreAudio on macOS. The launcher/no-argument app creates or opens
`Music/Songbird` under your user profile/home directory on these platforms.

See the [native platform guide](docs/PLATFORMS.md) for launchers, permissions,
and platform-specific setup.

## Learn the workspace

Songbird has two places to type:

- **Composition editor:** defines the song—tracks, notes, clips, and effects.
- **Command prompt:** performs actions such as playback, recording, saving, and
  exporting. Press **Ctrl+L** to focus it, type a command, and press **Enter**.

The track panel summarizes the composition. The console shows results, errors,
and saved recording paths. The transport shows playback position and audio meters.

| Shortcut | Action |
|---|---|
| Ctrl+E | Focus the composition editor |
| Ctrl+Enter | Evaluate the composition and apply changes |
| Ctrl+P | Play or stop |
| Ctrl+S | Save the project |
| Ctrl+L | Focus the command prompt |
| Ctrl+F / F3 | Search / select the next match |
| Tab in the prompt | Complete a command |
| Up / Down in the prompt | Recall command history |

Use the editor's normal undo/redo shortcuts for text edits. The prompt also has
`undo` and `redo` commands for evaluation and import snapshots.

## Start a new composition

Create a project in a new directory, then open it. Run these in your **shell**:

```sh
./target/release/songbird new "$HOME/Music/first-song"
./target/release/songbird "$HOME/Music/first-song"
```

Replace the contents of the **composition editor** with:

```text
project "First Song"
tempo 120
signature 4/4
bars 4

track drums {
    instrument drumkit
    pattern "kick hat snare hat kick hat snare hat"
    gain -12db
    loop
}

track bass {
    synth saw
    notes "C2:1 C2:1 G2:1 Bb2:1"
    filter 900
    gain -12db
    loop
}

track melody {
    synth triangle
    notes "C4:1 E4:1 G4:1 Bb4:1"
    reverb 0.2
    gain -9db
    loop
}

master limit -1
```

Press **Ctrl+Enter** to check it, then **Ctrl+P** to play. Check the console if
something fails. Press **Ctrl+S** to save.

Listen for the bass anchoring the beat and the higher melody adding movement.
Change one part at a time, evaluate, and listen again:

- `tempo 120` sets beats per minute; `bars 4` sets the song's total length.
- `C4:1` plays C in octave 4 for one quarter-note beat. Without `:duration`, an
  item lasts half a beat.
- `-` is a rest. Try `notes "C4 - E4 G4"`.
- `[C4,E4,G4]:2` plays a chord for two beats.
- `(C4 E4)*2` repeats a sequence twice.
- `loop` repeats a track's pattern through the song's end.
- `gain -12db` lowers volume; `pan -0.5` moves a track toward the left.

Edit a pattern during playback and press **Ctrl+Enter**. Prepared changes take
effect at a future bar boundary. If the song ends before you can apply an edit,
start playback again or increase `bars` for a longer practice session.

## Import and edit audio

In the **command prompt**, import a WAV or FLAC file:

```text
import "/absolute/path/to/dialogue.wav"
```

Songbird copies the file into the project and adds a track to the editor. Use the
relative path it inserts; editing the clip does not modify the original audio.
For a recording at least five seconds long, that track could look like this:

```text
track dialogue {
    import "samples/REPLACE_WITH_IMPORTED_FILENAME.wav"
    trim 0.25 3.5
    place at 8b
    fade in 20ms
    fade out 100ms
    gain -3db
    pan -0.2
}
```

Replace the example filename with the actual imported path. `trim` selects source
seconds; `place at 8b` starts the clip eight beats into the track. You can also use
seconds (`2.5s`), milliseconds (`250ms`), or timestamps (`00:08.250`).

Press **Ctrl+Enter** to apply the edit. Other clip operations include `split`,
`duplicate`, `reverse`, `stretch`, and `pitch`; see the
[editing and effects reference](docs/LANGUAGE.md#non-destructive-clips).

## Record a voiceover

Use the **command prompt** to list devices:

```text
devices
```

Choose names exactly as listed, then start a mono take:

```text
input "YOUR INPUT DEVICE NAME"
output "YOUR OUTPUT DEVICE NAME"
channels 1
monitor off
record opening flac
```

Speak into the selected input, watch the input meter, then enter:

```text
record-stop
```

Wait for the console to report the saved take. Add its project-relative path to
an `import` statement in the editor, just like the clip above. Position it with
`place at` or the track's `at` setting. Takes are not automatically placed or
latency-compensated.

Each recording gets a unique filename. FLAC is the default; `record opening wav`
chooses WAV. FLAC capture retains a recoverable WAV alongside the finished FLAC.
You can record while music plays. For monitoring and stereo capture, see the
[audio setup guide](docs/OPERATIONS.md#device-and-recording-acceptance).

## Build a singer in Voicebank Studio

Create a custom singer from your own recorded material, prepare its samples,
check English phoneme coverage, and audition lyrics without exporting a song.
Open the Studio from the **command prompt**:

```text
voicebank new voices/nova "Nova" english
voicebank register alto
voicebank expression normal
voicebank guide "me see"
voicebank target IY
voicebank record
```

Record a steady “ee,” then enter `record-stop`. Studio shows the take's waveform,
level/clipping diagnostics, and suggested pitch/timing. Use `voicebank replay`,
`stop`, and the metadata commands to refine it; `voicebank use` publishes the
preferred take. Repeat for M and S, then check and preview the phrase:

```text
voicebank coverage "me see" "A3:1 B3:1"
voicebank test "me see" "A3:1 B3:1"
```

Select a real input with `devices` / `input "DEVICE NAME"` before recording.
[Creating Your First Voicebank](docs/FIRST_VOICEBANK.md) walks through microphone
setup, alternate takes, trimming, root pitches, pronunciation, saving/reusing
banks, and creating a portable backing-vocal preset. The
[Studio command reference](docs/VOICEBANKS.md) covers every control.

To explore before recording, open the
[Voicebank Studio example](examples/voicebank-studio/README.md). It combines a
preset-driven singer with drums and bass. Its M/IY/S samples are explicitly
**generated engineering fixtures**, not a human voice or a full English bank.

## Save, reopen, and export

Press **Ctrl+S**, or enter `save` in the prompt. Reopen a song by passing its
**project directory** to Songbird:

```sh
./target/release/songbird "$HOME/Music/first-song"
```

Songbird saves the composition, project metadata, and relative asset references.
Use the app's editor and save command; directly changing the `composition.sb`
mirror does not update the generation selected by the project manifest.

To export the current composition, enter these in the **command prompt**:

```text
render "master.flac"
render "master.wav"
```

Files appear in the project's `exports/` directory. Exports use the song's `bars`
length and include its instruments, clips, vocals, and effects. Existing outputs
are protected: use a new filename for another render. Set `rate 48000` and
`bits 24` in the composition to choose the export sample rate and bit depth.

You can also render a saved project from the **shell**, without opening the GUI:

```sh
./target/release/songbird render "$HOME/Music/first-song" "$HOME/Music/first-song/exports/master-v2.flac"
```

## Resume the last project

Launching `songbird` without a project argument reopens the last successfully
opened or saved GUI project. Opening another project or using Save As updates
the absolute path in the existing user config. An explicit `songbird PROJECT`
argument always takes precedence. Command-line rendering and checks do not
change the remembered project.

Restoration is enabled by default. Disable it in your config with:

```toml
[session]
restore_last_project = false
# last_project = "/absolute/path/to/project" # Maintained automatically.
```

Missing, invalid, or inaccessible remembered projects produce a startup diagnostic
and fall back to the usual default project. Restoration only opens saved project
state, leaving recovery drafts untouched; use `recover-draft` explicitly to
recover edits. It runs before the editor exists and never replaces an unsaved
editor document. Config write failures are reported in the console without
discarding the active project. Other config preferences are retained.

## Native instrument bank (0.9.0)

Songbird includes 20 additional synthesized instruments for folk, country,
jazz, rock, soul, chamber, and cinematic arrangements. Select them with the
existing `instrument NAME` syntax; see the [sound catalogue](docs/SOUNDS.md)
for the complete bank and sound-design details. These are procedural voices,
not recorded acoustic instruments.

Pitched Mini shorthand is also available: `note("c4 <e4 g4>").s("piano")`.
All bank instruments support native Mini note patterns; see
[Mini notation](docs/MINI_NOTATION.md) and `examples/mini-instruments`.

Open `examples/instrument-bank` to audition every voice, or start from
`examples/folk-band`, `examples/jazz-combo`, or `examples/chamber`.
See [0.9.0 release and operator checks](docs/RELEASE_0_9_0.md).

## Customize the theme

Songbird finds its config automatically:

| Platform | Default config file |
|---|---|
| Linux | `$XDG_CONFIG_HOME/songbird/config.toml`, or `~/.config/songbird/config.toml` |
| Windows | `%APPDATA%\Songbird\config.toml` |
| macOS | `~/Library/Application Support/Songbird/config.toml` |

`SONGBIRD_CONFIG` overrides the location on every platform. Windows falls back to
`%USERPROFILE%\AppData\Roaming\Songbird\config.toml` if `APPDATA` is unavailable.
Start with the commented [example config](config.example.toml). On Linux:

```sh
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/songbird"
cp -n config.example.toml "${XDG_CONFIG_HOME:-$HOME/.config}/songbird/config.toml"
```

The copy command preserves an existing config. Edit that file in your text editor
and save: **Songbird automatically reloads appearance changes after a stable save (normally within about one
second)**, including during playback. Invalid edits leave the last working theme
in place and show an error in Songbird's console; fix the file and save again.
No restart is needed for colors, font sizes, or panel visibility.

A minimal config looks like this:

```toml
version = 1

[theme]
base = "light"
accent = "#7c3aed"
waveform = "#0f766e"

[appearance]
font_size = 14.0
editor_font_size = 17.0
show_tracks = true
show_console = true
```

Choose `dark` or `light` as the base. Omitted colors inherit that base; custom
colors use quoted `#RRGGBB` values. You can customize backgrounds, text, selection,
syntax colors, waveforms, and the recording indicator—see the example for all
keys. Fonts accept sizes from 10 to 32. Optional `window_width` and `window_height`
set the initial window size and require a restart. Reloading applies panel visibility only when that setting changes; temporary
`tracks`/`console` toggles survive unrelated theme saves.

The **Theme** dropdown above the transport switches between Light, Dark, and
Bubblegum immediately and saves the selection in the active config. Only the
theme section is replaced; other preferences are retained. To add themes, place
TOML files containing a `[theme]` section in the `themes/` directory beside your
active `config.toml` (or beside `SONGBIRD_CONFIG`). Reopen the dropdown to discover
them. File themes appear with a `(file)` suffix; the current custom palette is
shown as Custom. Invalid files report their errors in the dropdown.

The [Bubblegum theme](themes/bubblegum.toml) provides a pink, blue, and mint
palette. Copy its `[theme]` section over your config's existing `[theme]` section
to keep your other settings. Saved appearance changes reload automatically.

To preview Bubblegum with default settings:

```sh
SONGBIRD_CONFIG="$PWD/themes/bubblegum.toml" ./target/release/songbird examples/acceptance
```

To try a separate theme file without changing your normal config:

```sh
SONGBIRD_CONFIG="/absolute/path/to/theme.toml" ./target/release/songbird examples/acceptance
```

That file is watched too. Changing GUI audio preferences creates or updates the
selected TOML file; startup alone does not create one. A missing
normal config uses defaults; deleting it restores defaults. Invalid startup
configurations (including a missing explicitly selected file) use defaults and
report the error. Only changed settings are applied. Theme changes preserve editor state and do
not evaluate the composition or restart active audio streams. Audio preferences
apply to the next explicitly started stream.

## Help and troubleshooting

Enter `help` in the prompt or run `./target/release/songbird help` in your shell.

- **No sound:** check the console, output device, track gain/mute settings, and
  system mixer. The project rate must be supported by the device.
- **Missing audio file:** restore its relative path or update the `import` statement.
- **Unsaved edits after an interruption:** use `recover-draft`; evaluate and save
  the recovered text. See the recovery guide before repairing interrupted takes.

More documentation:

- [Complete language and command reference](docs/LANGUAGE.md)
- [Custom voicebanks and lyrics](docs/VOICEBANKS.md)
- [Device setup, recovery, installation, and rollback](docs/OPERATIONS.md)
- [Architecture and resource limits](docs/ARCHITECTURE.md)
- [Verification results and remaining limitations](docs/STATUS.md)

## License

Songbird uses the [BSD-3-Clause license](LICENSE). The bundled CMU pronunciation
dictionary has its own [license](data/CMUDICT-LICENSE) and
[provenance](data/README.md).

### Languages and procedural voices

Open `help language` for offline setup, or read [language packs](docs/LANGUAGES.md).
UI language follows system preferences independently of the singing language.
English, German, French, Japanese and Latin voice profiles are bundled; Latin is
voicebank-only. Add more languages with resource files, without editing Rust.
`voicebank default la` creates a labeled synthetic Latin bank; human recordings
are optional. Starter dictionaries and synthetic voices are limited examples.
See [pronunciation and punctuation research](docs/LANGUAGE_RESEARCH.md) for design
decisions and [per-language guides](docs/languages/) for authoring.

### Mini-notation and export workflow

Use `notation mini` inside a track to enable cycle-based patterns. See the
[reference and migration guide](docs/MINI_NOTATION.md), or open `help mini` in
Songbird. Pause/resume, step/filter automation, region exports and isolated stems
are described in [DAW workflow](docs/DAW_EXPANSION.md). The shared timeline supports tempo/meter maps, seeking, loops, punch/loop capture,
count-in, take comping and bounded PCM cache management. See [recording and
calibration](docs/RECORDING.md) for hardware-dependent alignment limits.

## Audio devices, recording, and waveforms

Expand **Audio devices & recording** above the editor. Choose input/output or
**System Default**, mono/stereo input, WAV/FLAC, and a take name; click **Record**,
then **Stop recording**. **Refresh devices** re-enumerates without starting a
stream. Missing saved devices remain marked unavailable; reconnect or select
another device. Unsupported project rates/channels report an error in the console.
Use headphones before enabling monitoring. Monitoring starts off every session;
opening Songbird never activates the microphone. Device/channel/format selections
persist under `[audio]` in the existing user TOML. Text commands remain available;
`input default` and `output default` select System Default.

The Tracks panel contains collapsible **Live waveform** and **Asset waveform
peaks** displays using `theme.waveform`. Master displays actual device-bound PCM
samples; track views show post-fader renderer peak envelopes (up to 128 tracks),
which can lead audible playback by the bounded audio queue. Imported and completed
recordings load asset peaks in the background without requiring evaluation.
Evaluated arrangements show a bounded overview of the first 64 clips; Voicebank
Studio retains its selected-take view. No synthetic animation is used. Samples
are handed off through bounded nonblocking queues; aggregation/drawing occur
outside the audio callback. Slow GUI frames may drop visualization data only.

See [recording settings](docs/RECORDING.md#configuration-settings) for the TOML
fields and [operator validation and rollback](docs/OPERATIONS.md#gui-fix-validation-and-rollback)
for the commands to run before using this change.

The in-app **Help** window renders the bundled Markdown guides with formatted
headings, lists, tables, selectable text, and copyable code blocks. Embedded guide
links stay inside Songbird; documentation remains available offline. See
[formatted help](docs/EDITING.md#formatted-in-app-documentation).

Voicebank Studio also includes an offline **Pitch coach** during explicitly
started bank recording: detected note/Hz, optional target and cents guidance,
level/clipping feedback, and a bounded pitch-stability trace. The selected take's
measured root can be applied explicitly before publishing. See
[live pitch coaching](docs/FIRST_VOICEBANK.md#live-pitch-coach-and-voice-practice).

New to recording or singing software? [Create your first voicebank](docs/FIRST_VOICEBANK.md)
starts from zero: project folders, microphone selection, sound labels, note/Hz and
trim explanations, then three recordings that sing “me see.” Studio's sidebar
provides sound/take dropdowns, original/trimmed replay, re-analysis, and explicit
**Use selected take** publication without replacing the composition editor.

## Optional code-first mixing expansion (unvalidated increment)

[Routing and native mixing](docs/MIXING.md) adds explicit channels, grouped sources,
nested buses, pre/post sends, drum isolation, sidechains, native EQ/dynamics and
node automation through the existing renderer. `analyze` reports actual offline
sample peaks, RMS, clipping, correlation and insert gain reduction. LUFS/LRA/true
peak and binary plugin hosting remain unfinished; the format-neutral plugin API
is not a working VST3/CLAP/LV2 host. Read [status](docs/STATUS.md) and run the
[operator checks](docs/OPERATIONS.md#routing-expansion-validation-and-rollback).
The [hybrid example](examples/mixer-routing/README.md) needs no plugin dependency.

The beginner bank workflow now links directly to [language-specific phonetics](docs/PHONETICS.md),
including exact shipped IDs, broad IPA, examples, dialect notes and recording
checklists for English/German/French/Japanese/Latin. These guides are embedded in
Help and work offline; linked audio references are optional supplements.

Mini/native compositions also accept familiar percussion names (`bd`, `sd`, `hh`,
`oh`, and more) and bounded `s(...)`/`sound(...)` expressions with fast/slow,
sample-end and decay/sustain controls. See [syntax](docs/MINI_NOTATION.md#familiar-sound-constructors)
and the offline Help [Sound library](docs/SOUNDS.md) for the native collection and
local sample mappings. Native Songbird statements remain the main composition,
mixing and routing interface. No JavaScript runtime is introduced.

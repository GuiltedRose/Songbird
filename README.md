# Songbird

Songbird is an offline music workstation for Linux. Write music in a code editor,
record or import audio, mix tracks, and export WAV or FLAC—all in one native app.

## Build and open the example

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

## Make your first song

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

Here is how to change the music:

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

## Use a singing voice

The bundled example's `Analytic` voice sings an “ah” vowel. Open
`examples/acceptance` to see how its `voice`, `lyrics`, and `notes` statements fit
together.

For English lyrics, create a voicebank from recordings you are authorized to use.
A bank contains phoneme samples and a `voicebank.toml` describing their timing,
pitch ranges, registers, and expression styles. Follow the
[voicebank guide](docs/VOICEBANKS.md) to record, label, validate, and use one.
The analytic example cannot sing arbitrary English words.

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

Hardware recording, capture synchronization, and human English singing still
need production acceptance testing. The linked status document distinguishes
implemented behavior from verified results.

## License

Songbird uses the [BSD-3-Clause license](LICENSE). The bundled CMU pronunciation
dictionary has its own [license](data/CMUDICT-LICENSE) and
[provenance](data/README.md).

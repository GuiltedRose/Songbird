# Native platform operation

The same Rust core, editor, codecs and renderer are used on every desktop target.
OS-specific audio, graphics, paths, and filesystem behavior still require native
validation; being Rust alone does not establish compatibility.

## GitHub builds

`.github/workflows/desktop.yml` runs on pushes, pull requests, and manual dispatch.
It installs Rust 1.97.1, checks formatting/Clippy, runs the offline audio/project/
config tests, builds a release executable and checks CLI startup. Windows uses
`windows-latest`; macOS uses `macos-latest`. Artifact names include runner
architecture; the Mac artifact is not a universal Intel/Apple Silicon binary.
Use a native build for any additional architecture you need.

After pushing, open **Actions → Desktop builds** and inspect both jobs. Download
artifacts only from a run whose source revision you intend to use. The workflow
uploads build artifacts for 14 days; it does not publish a release, deploy, sign,
notarize, or enable any service. No runner was dispatched during implementation.
CI checks do not activate microphones or prove hardware audio/GUI behavior.

## Windows

Install Rust's MSVC toolchain plus Visual Studio C++ Build Tools and a Windows SDK.
No ALSA packages are required. From PowerShell in the source checkout:

```powershell
cargo fetch --locked
cargo test --locked --offline
cargo build --release --locked --offline
.\target\release\songbird.exe help
.\target\release\songbird.exe examples/acceptance
```

Alternatively extract the Windows artifact to a writable folder and run
`Start-Songbird.cmd`. It opens `%USERPROFILE%\Music\Songbird`; pass another project
folder as its argument if desired. `songbird.exe` remains a console-capable binary
so its CLI commands and errors work. Installation needs no registry changes or
administrator permissions. Keep older extracted releases for rollback.

Create the theme config without overwriting an existing file:

```powershell
$songbirdConfigDir = Join-Path $env:APPDATA 'Songbird'
New-Item -ItemType Directory -Force $songbirdConfigDir | Out-Null
$songbirdConfigFile = Join-Path $songbirdConfigDir 'config.toml'
if (-not (Test-Path $songbirdConfigFile)) {
    Copy-Item config.example.toml $songbirdConfigFile
}
```

Audio uses CPAL's WASAPI backend. Enable microphone access for desktop apps in
Windows privacy settings if input is denied. In Songbird's prompt use forward
slashes for paths (`import "C:/Users/you/Music/take.wav"`); quoted backslashes
are escape characters in the composition language. Automatically inserted import
and recording paths use forward slashes. Shell arguments accept ordinary native
Windows paths when quoted normally.

Saving syncs and closes staged files before replacing manifests. Parent-directory
fsync remains Unix-only; do not assume identical power-loss durability on Windows.
Original takes and source generations are retained as on Linux.

## macOS

Install Rust and Xcode Command Line Tools. No ALSA packages are required:

```sh
xcode-select --install
cargo fetch --locked
cargo test --locked --offline
cargo build --release --locked --offline
./target/release/songbird examples/acceptance
```

The CI artifact contains a `.tar.gz`; extract it to preserve executable permission,
then open `Songbird.app`. It opens `~/Music/Songbird` by default. You may move the
app to Applications without moving or overwriting your projects. The binary at
`Songbird.app/Contents/MacOS/songbird` also accepts normal CLI arguments.
The app is unsigned/unnotarized; normal macOS approval may be required to open it.

```sh
mkdir -p "$HOME/Library/Application Support/Songbird"
cp -n config.example.toml "$HOME/Library/Application Support/Songbird/config.toml"
```

Native audio uses CoreAudio. The app bundle includes the microphone usage
explanation required for the system permission prompt. Allow the application in
**System Settings → Privacy & Security → Microphone** when recording. A terminal
launch may instead need permission for that terminal. The built-in executable
alone does not supply the bundle's metadata. Permit access only when you intend
to record; Songbird starts capture only through an explicit record command.

## Validate and roll back

On each native host, open the example and check editor input, live theme changes,
waveforms and transport. Select real input/output devices, record WAV and FLAC,
play the takes, import files with spaces in their paths, save twice, reopen,
recover the previous save, and export both formats. Test unplug/device-loss and
monitoring on the intended hardware. Use the hardware checklist in OPERATIONS.md.
Native CI can verify save/reopen and codec behavior but cannot replace these checks.

For rollback, close Songbird and launch the previous extracted executable/app.
Keep project folders, configs, takes and save generations. This packaging has no
installer, automatic update mechanism, data migration, or system service.

## New routing/DSP/plugin expansion

The 2026-11-12 routing/native-DSP source increment has no added system dependency,
but has not been built or validated by the implementer on any OS. Previous user
build/audio confirmations apply to earlier code. VST3/CLAP/LV2 binary hosting,
plugin GUIs, device MIDI and latency compensation remain unavailable on **all**
platforms; there is no universal compatibility claim. See [status](STATUS.md) and
[operator validation](OPERATIONS.md#routing-expansion-validation-and-rollback).

## Application icon

The desktop window embeds `Assets/songbird.png` in the executable. Rebuild and
restart Songbird to use it; no external image file is needed at runtime.
For the Linux launcher, with `songbird` on your PATH, run from the checkout:

```sh
install -Dm644 Assets/songbird.png "${XDG_DATA_HOME:-$HOME/.local/share}/icons/songbird.png"
install -Dm644 packaging/songbird.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/songbird.desktop"
```

To roll back the launcher installation, remove those two installed files.
Projects and preferences are unaffected. Desktop icon behavior varies by platform;
macOS Finder and Windows Explorer executable icons require separate bundle/resource
packaging beyond this window icon.

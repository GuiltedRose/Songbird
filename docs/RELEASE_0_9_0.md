# Songbird 0.9.0

Adds 20 native pitched instrument voices and four audition/ensemble projects.
The bank covers keys, plucked and bowed strings, bass, winds, brass, and mallets.
All voices use the shared playback/export renderer, bounded partial counts,
note-local envelopes, and Nyquist attenuation. No new dependencies or external
assets are needed. Package and macOS bundle versions are now 0.9.0.

Existing composition syntax, project/config versions, percussion, and basic
waveforms are retained. Projects using the new names require 0.9.0. Existing
projects, recordings, voicebanks, training data, and user configuration are not
migrated or deleted. Recent theme selection and active-project restoration
remain available.

## Operator validation and activation

Prerequisites: the existing Rust toolchain and platform audio/windowing build
dependencies; populated Cargo cache for offline commands. Run from the checkout.
Close Songbird first and keep a copy of your current executable and user config.
These commands have not been executed as part of this change.

```sh
mkdir -p /tmp/songbird-rollback
cp -p target/release/songbird /tmp/songbird-rollback/songbird-before-0.9.0
cargo test --locked --offline --lib instruments::tests
cargo test --locked --offline --test workstation synths_are_finite_and_audible
cargo test --locked --offline --test workstation instrument_bank_examples_prepare_without_external_assets
cargo test --locked --offline --test strudel_syntax
cargo build --release --locked --offline
./target/release/songbird check examples/instrument-bank
./target/release/songbird check examples/folk-band
./target/release/songbird check examples/jazz-combo
./target/release/songbird check examples/chamber
./target/release/songbird check examples/mini-instruments
./target/release/songbird examples/instrument-bank
```

Use Play explicitly. Listen through all 20 bars, then audition the three ensemble
projects. Check attacks, sustained notes, chords, high/low registers, pitch bends,
and transport seeks. Watch the console for playback errors and check for dropouts
on the target audio device. Adjust track gain for your arrangement. No listening,
device-load, or acoustic-realism acceptance is implied by automated checks.

To compare an export with playback (requires an unused output filename):

```sh
./target/release/songbird render examples/instrument-bank /tmp/songbird-0.9.0-bank.wav
```

Activate by launching the built executable through your usual launcher, or use
the existing platform packaging workflow. No service or background process is
introduced. Your existing startup restoration preference still applies.

## Rollback

Close Songbird and launch the retained executable:

```sh
/tmp/songbird-rollback/songbird-before-0.9.0 /absolute/path/to/an-existing-project
```

Choose a project using the old instruments; older binaries cannot render the new
instrument names. Keep new arrangements and assets intact for 0.9.0. If your
normal launcher points at `target/release/songbird`, restore that executable
from the retained copy after closing the application. No project-data rollback
or cache deletion is needed.

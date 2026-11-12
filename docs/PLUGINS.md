# Plugin hosting: architecture and compatibility

**No plugin format is operational in this increment.** `plugins` in Songbird's
prompt or `songbird plugins` in a terminal reports the same explicit status.
VST3, CLAP and LV2 ABI adapters, binary discovery/scanning/loading, original GUI
embedding, native plugin widgets/code directives, device MIDI I/O, plugin audio
routing and latency compensation are not implemented. Projects requesting a
`plugin` track directive fail clearly; Songbird does not silently bypass it.

The format-neutral code in `src/plugins.rs` is a foundation, not a universal host.
Its tests use a Rust fixture and do not load a CLAP/VST3/LV2 plugin.

## Shared instance contract implemented at the control layer

Backend adapters have one stable plugin identity and instance identity. Parameter
metadata exposes stable IDs, ranges, write/automation capabilities and values.
Code, automation and MIDI mapping changes address that instance; original-GUI
notifications update the same authoritative parameter cache without echoing back
to the backend. Opening/closing an editor requests an operation on that identity,
not a second audio processor. A parameter the plugin does not expose cannot be
assumed to have headless control.

The control layer validates MIDI Learn CC mappings, typed snapshot identity/version,
parameter ranges, bounded opaque state (16 MiB), mappings and prepared audio block
sizes. It supports mock-backed offline processing/state round trips and clears
output on reported processing errors/nonfinite samples. It is not an FFI crash
sandbox: a native process crash cannot be contained by a Rust Result. No real
plugin-state/preset file UI or automatic project persistence is wired yet.

Adapters must marshal editor requests onto the correct OS UI thread while the
same instance continues DSP. Native controls, plugin GUI callbacks, transport,
automation and timestamped MIDI require a bounded event protocol with defined
ordering. The current contract is not yet a sample-accurate event scheduler,
multibus port negotiator or MIDI-output API; those remain required integration.

## Format evaluation (sources checked 2026-11-12)

| Format | Primary SDK/specification | Integration work and limits |
|---|---|---|
| CLAP | [Official CLAP repository](https://github.com/free-audio/clap), MIT license | Stable C ABI with extensions; implement audio/note ports, parameters, state, GUI, offline and latency extensions, thread checks and rescans. Capability support must be negotiated per plugin. Recommended first adapter. |
| VST3 | [Official Steinberg SDK](https://github.com/steinbergmedia/vst3sdk), current SDK MIT license | Processor/controller lifecycle and synchronization, bus negotiation, events, state, SDK GUI platform handling. Follow applicable license/trademark terms; older SDK licensing differs. |
| LV2 | [Official LV2 specification](https://lv2plug.in/), permissive specification/code | Resolve RDF metadata/features and extensions; state, worker, UI and atom/MIDI support need per-plugin capability checks. Platform UI toolkits vary. |

All three are intended for cross-platform hosting; a format's platform support
is not proof of Songbird support. Native Windows/macOS/Linux architectures, bundle
layouts, GUI window APIs and plugin dependencies require separate acceptance.
No new SDK, plugin binary, service, device permission or network download is
installed by this increment. A physical guitar/microphone mixer continues to use
Songbird's native audio devices; it does not need plugin software.

## Remaining implementation and acceptance sequence

1. Select SDK revisions and preserve license notices; provide buildable open-source
   reference effect and instrument fixtures so users need not buy plugins.
2. Add an isolated scanner helper with bounded IPC, per-plugin timeout/quarantine,
   architecture/format diagnostics and no scanning in playback. Retain state for
   missing plugins instead of replacing audio or deleting snapshots.
3. Implement and test CLAP audio, instrument notes, state/presets, generic controls,
   code parameter declarations and automation. Connect one instance to the graph.
4. Add main-thread original GUI creation/reopening and callback synchronization,
   then OS MIDI input/output, MIDI Learn and MIDI mapping persistence.
5. Add reported latency, bounded compensation buffers and routing-branch alignment;
   latency changes require prepared graph rebuilds. Do not pretend all plugins
   have zero latency or deterministic offline behavior.
6. Add isolated processing workers where feasible, crash recovery, offline state
   restoration, and per-plugin/OS compatibility reports. A timeout must not block
   CPAL or silently discard a failed effect/instrument.
7. Repeat the conformance and GUI/audio/state suite for VST3 and LV2, then consider
   additional formats individually. Validate GUI and headless limitations explicitly.

This sequence remains unfinished. The broader request's complete production-ready
plugin-hosting definition of done has not been met. Existing build confirmations
on three operating systems predate this increment and do not validate these APIs.

## Ardour architecture reference

The [Ardour source mirror](https://github.com/Ardour/ardour) is an appropriate
open-source design reference. Reviewed on 2026-11-12:

- [PluginInsert](https://github.com/Ardour/ardour/blob/master/libs/ardour/ardour/plugin_insert.h)
  separates insert processing, automation, latency reporting, state and external
  parameter-change notifications.
- [VST3 adapter](https://github.com/Ardour/ardour/blob/master/libs/ardour/ardour/vst3_plugin.h)
  has format-specific parameter/controller/editor responsibilities.
- [LV2 adapter](https://github.com/Ardour/ardour/blob/master/libs/ardour/ardour/lv2_plugin.h)
  tracks parameter ports, editor availability, automation and latency.
- [Plugin UI window](https://github.com/Ardour/ardour/blob/master/gtk2_ardour/plugin_ui.cc)
  selects native editors when supported and handles generic UI responsibilities.

These support the separation used in Songbird's contracts: shared instance state,
format adapter, mixer insert and independently managed UI lifecycle. This review
is not a claim that Ardour code was copied, that Songbird has equivalent hosting,
or that every requested format was verified in Ardour. Ardour's COPYING states
GPL-2.0-or-later with a clarification about separately licensed plugins; Songbird
retains its existing BSD license and independent code. Free versus paid plugin
pricing does not determine the host ABI; plugin-specific dependencies and license
terms still apply. No commercial plugin purchase is needed to create reference
effect/instrument tests with appropriately licensed open-source fixtures.

Audio Units would require a separate macOS-only adapter and validation. Other
formats are considered individually; no additional format is advertised today.

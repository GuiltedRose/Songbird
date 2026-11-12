# Code-first production expansion: audit and staged implementation

## Audit and baseline

The existing parser builds one Score; Mini expands into the same Event schedule
as native notes. Renderer prepares instruments, recorded/imported clips, voicebank
units and ordered Processor chains. Each track already owns gain/pan/mute/solo,
inserts and automation. Playback uses a prepared-renderer feeder; CPAL consumes a
bounded queue. Export, preroll, seek and loops share Renderer and its DSP checkpoints.
Track/master effects currently have zero added algorithmic latency; delay/reverb
are intentional effects, not plugin latency. Stems currently isolate each track.

Preserve the legacy summation path for compositions with no new mixer directives.
Compile a bounded typed DAG before asset decoding/playback. Explicit channels group
post-track sources; buses sum routed channels. Direct routes and pre/post-fader
sends are edges. Reject missing nodes, ambiguous ownership and cycles. Stable
node ordering makes offline renders deterministic. Checkpoint new state alongside
existing DSP so seeking/looping does not leak bus history. New facilities must not
change existing equal-power track pan or effect parameter conventions.

No baseline tests/builds/reference renders were executed by the implementer.
Operator baseline: save a known-good executable, run the existing test/build
commands, and render existing acceptance/garden/mini-session projects into a new
scratch directory with the known-good executable. After building, compare legacy
WAV PCM (not merely container bytes) and seek/loop behavior against those renders.
Retain all projects/media and existing limits. The operator guide contains exact
commands for the new increment.

## Milestones

1. Mixer foundation: typed destinations, automatic track channels, optional
   explicit channel and bus blocks, sources, nested routes, sends, DAG validation,
   graph checkpoints, bounded allocation and isolation of existing stem exports.
2. Instrument routing: all instrument types share track routes; optional drum-event
   isolation produces independently processable drum tracks without changing the
   simple kit. Explicit grouping and mixer automation use the same DSP path.
3. Advanced DSP: additive syntax only, stable filters/dynamics, sidechain dependencies
   in the graph, bypass and gain-reduction metering. Preserve old DSP algorithms.
4. Mastering: implement standards-backed loudness and oversampled true peak only
   with reference fixtures. Never label sample peak as true peak or RMS as LUFS.
5. Plugins: evaluate native ABIs and licensing, then an isolated initial host with
   explicit parameter/latency/error contracts. Do not advertise an unvalidated host.
6. Documentation: real supported phoneme inventories, separate language references,
   verified external supplements and offline Help. Examples must use final syntax.

Each stage requires operator-run compilation, regression tests and acceptance
before production use. Unimplemented stages remain explicitly listed in status;
a plan or API declaration is not an implemented capability.

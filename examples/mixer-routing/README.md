# Native/Mini routing example

All sounds here are native synthetic instruments. There is no plugin or human
voicebank dependency. Kick uses Mini; hats and bass use native notation. Drum
isolation retains the source pattern timing. The bass feeds an explicit channel,
uses the kick as compressor detector, and sends to a wet reverb bus. Drum/reverb
buses and bass meet at music, then master. See [mixing](../../docs/MIXING.md).

This example was written but not rendered/tested by the implementer. Run the
operator validation commands before accepting its audio. Listen for bass ducking
on the kick, and compare with compressor insert 2 bypassed using `bypass 2` inside
bass_strip. Do not add `bypass 2` to another node with fewer than two inserts.

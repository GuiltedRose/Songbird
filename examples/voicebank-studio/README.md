# Voicebank Studio engineering example

Open this project to hear a reusable backing-vocal preset with drums and bass.
Then use the prompt:

```text
voicebank open voices/studio-synthetic
voicebank samples
voicebank sample 2
voicebank replay
stop
voicebank coverage "me see" "A3 B3"
voicebank test "me see" "A3:1 B3:1" "Soft Harmony"
```

All three WAV files are original **generated audio**, dedicated to the public
domain under CC0-1.0. They are not microphone recordings, a human singer, a full
English voicebank, or proof of intelligible/natural English singing. The labels
are explicit generator targets, not the result of automatic phoneme recognition.

M is a low harmonic hum, IY a harmonic signal with vowel-like spectral shaping,
and S deterministic high-passed noise. Root 220 Hz is the generator's fundamental
for M/IY; the unvoiced S entry uses it only as a neutral reference. Samples are
mono 16 kHz / 24-bit PCM, 0.8 s long, with sound from 0.1–0.7 s and edge fades.
Each harmonic signal sums harmonics 1–34 of 220 Hz, weighted by Gaussian bands
and inverse square-root harmonic number. M bands: (250 Hz, 100 Hz width, 1 gain)
and (1000,150,0.12); IY: (300,100,1), (2500,180,0.7), (3200,250,0.4).
S uses an LCG (seed 0x12345; a=1664525; c=1013904223; modulus 2^32), subtracts
a one-pole low-pass with coefficient 0.18, and scales by 0.18. Harmonic signals
are scaled by 0.22. Fades are linear 15 ms in / 30 ms out.

These fixtures exercise multi-label coverage, consonant/vowel routing, presets,
harmony, direct preview, and the shared mixer. Follow docs/FIRST_VOICEBANK.md
with your own authorized recordings for a real singer and human listening checks.

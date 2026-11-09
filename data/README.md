# Offline English pronunciation data

`cmudict.dict` and `CMUDICT-LICENSE` are unmodified from
https://github.com/cmusphinx/cmudict at commit
`74790861f652b15e4ac49015a90074ad62a27690`.

SHA-256:

- cmudict.dict: `81917843c7f44ce2b094ac63873c2c7a4cf802040792c455ba3ca406891c3d22`
- CMUDICT-LICENSE: `bd4ce8e44170a5f9f481310ca85c51de3c4f851a65e679b40e603b143bd3542a`

Copyright (C) 1993-2015 Carnegie Mellon University. All rights reserved.
Redistribution terms are preserved in CMUDICT-LICENSE and must accompany binaries.
The build sorts a copy by byte order (upstream groups variants before possessives).
The sorted dictionary is compiled into Songbird and binary-searched in place, without
loading a heap index or contacting any service. Stress removal and approximate
syllable boundaries happen at lookup; bank overrides retain priority.

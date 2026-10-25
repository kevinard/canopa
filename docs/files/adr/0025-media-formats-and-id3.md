# Media format acceptance and bounded ID3 input

Status: accepted under delegated implementation authority, 2026-10-16.

Quick Look uses the installed GTK/GStreamer decoder backend. Native acceptance
now includes MP3, FLAC, Opus, AAC/M4A and H.264/AAC MP4 alongside WAV and
VP8/Vorbis WebM. Audio fixtures have different tones before and after a known
position. Seeking must change the actual recorded tone, and Home must restore
the first tone. MP4 acceptance checks its distinct AAC track, advancing H.264
pixels and paused rewind. Fixture encoding uses one filter/encoder thread where
needed to avoid unrelated video-generation timeouts under build load.

This acceptance exposed a decoder abort while preparing a tagged MP3. The Files
window opened successfully; GStreamer then asserted in
`gstdecodebin3.c:mq_slot_handle_stream_start` because a stream ID did not belong
to its collection. The initial diagnosis as a missed opening key was incorrect.
The failing application log is retained in `native-media-packaged-29`.

A disposable stock-GTK probe reproduced the same SIGABRT with a standard GIO
FileInputStream and with a filename, before either stream became prepared
(`native-media-stock-32`). This excludes Files' custom reader and browser state.
The matching GStreamer 1.28.7 source and trace show a fallback collection and a
later stream-start event carrying different stream IDs. Although GstPlay's
documentation still describes a legacy selector, the installed implementation
unconditionally creates playbin3. Testing that selector did not change the abort
(`native-media-stock-33-legacy`); Files adds no environment override. Keeping the
same encoded MP3 frames while omitting the leading ID3 tag passes 100 stock-GTK
open/prepare/clear cycles through each input route
(`native-media-stock-34-untagged`).

Prepared media therefore supplies a decoder byte view after validated leading
ID3 envelopes. This applies by content to audio and video inputs, including files
whose extension does not describe their actual encoded audio. The opened regular
file remains pinned as in ADR 0024. Logical seek positions start at the decoder
input, and its end remains bounded by the prepared file length. Encoded audio is
neither transcoded nor rewritten. The original file, file information size,
operation history and Open in default application retain the complete file.
This preview does not expose embedded tag fields or artwork; those can be read
separately if later product work adds them.

The worker recognizes ID3v2.2, v2.3 and v2.4 envelopes. It validates supported
version/flag fields, synchsafe size bytes, captured file bounds and any v2.4 footer.
Footer bytes must match the header's version, flags and size. It reads only the
small envelope headers/footers and accepts up to 32 leading tags, checking
cancellation between them. Invalid or truncated envelopes produce a preparation
error with normal application opening still available. Frame metadata is not
parsed. The ID3 structure follows the primary
[v2.4 specification](https://id3.org/id3v2.4.0-structure),
[v2.3 specification](https://id3.org/id3v2.3.0), and
[v2.2 specification](https://id3.org/id3v2-00).

Public preview regressions first reproduce the raw tag reaching decoder input,
then verify the bounded view, seek offsets and retained file after replacement.
Ten preview tests pass, covering a v2.4 footer, a sparse 128 MiB tag, multiple tag
versions, audio/video filename hints, invalid flags/sizes/footers and the tag-count
limit. Native format acceptance also repeatedly prepares the original tagged MP3;
the fixture's file is not stripped to bypass the failure. Media fixtures disable
system core collection so an assertion remains in the log without filling the
host disk or delaying cleanup. Runtime packaging and final native results are
recorded in the implementation ledger and root ADR 0017.

Final `native-media-id3-35` (dark) and `native-media-id3-36` (light, 1.25×) each
pass all 28 format/launcher checks against the freshly extracted package. Each
prepares the original tagged MP3 nine times before checking its decoded tones,
seek and rewind. `native-media-controls-37` passes the original seventeen WAV/
WebM playback, volume, focus, navigation and source/owner-lifetime checks against
the same package. There are no decoder aborts or GTK assertions in these final
logs. All 106 core tests pass, with eight child-process helpers ignored by the
parent runner. Native build, Files/CLI all-target Clippy and formatting pass.

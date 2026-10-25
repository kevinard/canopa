# Media previews retain the opened file

Status: accepted under delegated implementation authority, 2026-10-16.

Preview preparation opens a regular file once on its worker. Classification,
metadata and content preparation use that descriptor. Audio and video carry a
`MediaSource` to the desktop instead of asking GTK to reopen the original path.
A rename, deletion or replacement at that path cannot redirect the prepared
source to another file or turn it into a blocking FIFO read.

Each media reader retains the descriptor through shared ownership, has its own
byte position, and uses positional reads. Its end is the file length captured
during preparation, so subsequent appends do not create an unbounded preview.
The file is not copied into memory or temporary storage. This preserves streaming
for large local media. In-place writes and truncation can still affect its bytes;
the source pins a file instance, not an immutable content snapshot. Image and PDF
decoding continue to use bounded byte snapshots, now read from the same descriptor
used for classification. Poppler receives the private PDF snapshot.

[ADR 0025](0025-media-formats-and-id3.md) adds a bounded decoder view after leading
ID3 metadata to avoid a reproduced GTK/GStreamer discovery abort. File information
continues to describe the complete opened file.

GTK still owns playback, decoding and native controls. The desktop provides a
small read-only GInputStream/GSeekable adapter with accurate current positions,
seek handling, cancellation checks and serialized reader access. The installed
GIO Rust 0.22.10 adapter's `ReadInputStream::new_seekable` advertises seeking but its
`tell` implementation returns `-1`. Native playback reproduced paused video
rewind remaining at 0:07. A targeted GStreamer trace then showed its size probe
seeking to the end and failing to restore that reported position with Invalid
Argument. The custom adapter implements the complete position contract without
modifying a dependency fork or introducing another media framework.

The public preview tests prepare real temporary files and then replace their
names. The initial path-backed reader returned the replacement's bytes; the
retained source returns the original bytes. Other checks cover a replacement
FIFO, independent readers after the source handle is dropped, end-relative seeks,
invalid positions and growth beyond the prepared length. Six preview tests pass.
The existing native media workflow remains the regression seam for actual
decoding and seeking: it compares rendered video frames and recorded private PCM
after keyboard input, rather than relying on a seek slider moving.

The original failed input-stream runs and the diagnostic trace are retained in
`native-media-stream-21` and `native-media-stream-trace-23`. Run 22 timed out
generating its fixture video before application launch. Run 24 did not open its
initial preview, so it supplies no decoder result. With the corrected adapter,
`native-media-bound-25` (dark) and `native-media-bound-26` (light, 1.25×) each
pass all seventeen media checks, including paused rewind and owner-close audio
shutdown. `native-acceptance-35` passes the 27-check desktop regression with actual
image pixels and two PDF pages. The final core run passes 102 tests, with eight
explicit child-process helpers ignored by the parent runner. Build, all-target
Clippy, formatting, Python syntax and whitespace checks pass. The first final
core rerun received SIGTERM during compilation; its completed retry is retained
as `core-media-bound-retry.log`.

This decision binds one prepared regular file after it has been opened. Binding
the original browsing intent before open, unavailable-device handling and
directory alias races remain separate work. Later PDF page requests prepare
their source again. Opening a file in its normal application still uses the
selected path. Broader codec/runtime packaging and remaining preview metadata
and control cases also remain required by the product brief.

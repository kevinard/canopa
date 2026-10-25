# Iced media implementation — 2026-10-19

## Current verified media checkpoint

The playback backend and Iced frontend are implemented; the full Files goal
remains active. The candidate executable is03ed1b63 (125,267,624 bytes), built by
public-preview run31 from the unchanged Rust implementation reviewed at source23.
Source45 pins 623 inputs for the final native runs and scoped checks. The final
source/review manifest adds only the documentation of these results.

Green24 passes 64 library parents plus the actual isolated lazy-init child
(one ignored helper definition), in5.20s runtime. The current public-preview
run31 passes 10 tests. All-target Files Clippy30 with warnings denied passes;
final50 passes workspace formatting, the 585-package dependency-inventory check,
Python syntax and the local-runtime library check with default features disabled.
The headless normal dependency graph contains neither Iced nor GStreamer.

The real rapid-control regression fails at red22: two Mute activations without
MediaPoll leave mute enabled. Backend ToggleMuted/VolumeBy/SeekBy now serialize
intent. Relative seeks compose until actual decoder time confirms their accepted
target; an older AsyncDone cannot erase it. The controller regression also checks
volume0.1 and seek10s after Home/Right/Right without a poll. Actual video checks
+2/+2 reaches green4 and -2/-2 returns red0. Source23 review axes have zero
outstanding findings; no Rust changes follow that green checkpoint.

Four native Iced cases pass 34 grouped checks on the same candidate:

- Dark basic: completed per-case summary in `media-native-46`,9 checks.
- Light basic: `media-native-47`,9 checks,78.72s, terminal0.
- Dark formats: `media-native-formats-48`,8 checks,89.07s, terminal0.
- Light formats: `media-native-formats-49`,8 checks,93.45s, terminal0.

They use generated private media, isolated PipeWire/Pulse/null output, calibrated
recorded PCM and actual rendered pixels. Checks cover no autoplay, play/pause,
mute/volume and rapid keys, paused seeks/rewind, advancing VP8/H.264 pixels,
narrow controls and unfocused-hover source navigation. Another Browser opens an
independent text Quick Look while the original video and its audio continue.
Changing source and closing its owner stop the decoder; final Browser closure
exits successfully after cleanup. MP3 is prepared repeatedly before every close;
MP3/FLAC/Opus/AAC seeks change actual tones and rewind restores them. Both themes'
concise corrupt-media explanation and focused/unfocused Open were visually
inspected, separately from asserting the private Open helper's actual arguments.
The native matrix has 150 scenarios; these results count only the four media cases.

[The verification record](media-verification.json) links exact commands, receipts,
hashes, reviews and preservation. `media-native-basic-verification-47.json`
records the inspected basic captures. Source hashes match before/after every
completed run. Runtime/CI declare direct GStreamer/base development packages;
all 769 prior locked package versions/checksums remain, with14 new packages.

## Native diagnosis and acceptance limits

Native26 fails a fixture width observation, excluded as product red. Native28
runs the preserved fc23 fallback and genuinely fails to find enabled Play.
The initial new basic cases32/34 reach actual playback and paused video checks
but exceed the10s wtype deadline while typing a long path in another Browser.
The reduced paused probe36 completes7.07s; playing38 exceeds10s; probe40 with a30s
deadline completes19.45s and copies the exact entered bytes back for comparison.
The primary [wtype implementation](https://raw.githubusercontent.com/atx/wtype/master/main.c)
performs compositor roundtrips for each key press/release. The maintained media
fixture pastes long paths through the ordinary location input and keeps actual
keyboard control/navigation checks. This nested software observation is not a
production latency budget or a performance fix.

Attempt42 times out because forked wl-copy retains captured pipes; directing its
output to a file fixes the fixture's EOF wait. Attempt44 verifies the visible
corrupt-media explanation, but Open launches installed Firefox within the private
HOME/compositor because only text/plain was registered. The setup now explicitly
registers and verifies the private helper for WebM. Expected final Files exit is
observed before removing the terminal child from the harness's unexpected-exit
list; failure keeps cleanup ownership. Neither setup failure is counted as an
application defect or a successful acceptance case.

Runner46 receives SIGTERM143 during light acceptance, with no known cause. Only
its completed dark per-case result is counted. Cleanup verifies exact output
handles, retires12 private process groups without forced KILL and removes that
private runtime. Light47 is the complete retry. The suspected blank Open label
was not reproduced: independent image review and actual glyph pixels confirm it
in the retained captures. Source45 Standards and Spec follow-ups have zero
outstanding findings; earlier oracle gaps were closed by readiness, advancing
pixels and explicit rendered-error review.

Final source51 review identifies one P3 documentation overstatement, with no
application defect: the ADR's native coverage sentence could include pointer/touch
dragging and wheel suppression. Source53 narrows it to keyboard, hover navigation
and narrow layouts. Pointer/touch drag-release and wheel-suppression acceptance
remain pending; this correction changes no implementation or verified result.

Production startup/idle/input performance, accessibility, the full native matrix,
optimized/hosted release and package verification, remaining platform ownership,
source/restart authority and the rest of the PRD remain open. No product checklist
is closed by this scoped checkpoint.

## Earlier resource interruptions

Test16 has a missing handle, ENOSPC metadata log and zero-byte wrapper receipt;
no pass or exact exit is claimed. An independent process scan found no compiler.
Cache17 stopped on tmp quota after removing old Files metadata and relocating
one dependency; its incomplete ravif copy was removed with original retained.
A later external target cleanup (cause unknown) freed~10GiB and removed Files
artifacts/current executable. Successful test20 rebuilt candidate2f962d11 from
source19. Cache18 restored11 verified dependency archives and removed8 orphaned
reproducible Files test/rlib caches; prior fc23 gzip/source16 hashes match. Current
home~11GiB/tmp1.3GiB; no further cache recovery needed. Receipts18 andtest16
observation record this; the interrupted17 script wrote no final receipt.

## Earlier backend and frontend progress (superseded by the checkpoint above)

`playback` now owns lazy GStreamer initialization, bounded prepared-reader appsrc
input, worker-only transport/seek/volume/mute, a latest bounded RGBA frame and
coalesced snapshot notices. `Application` holds admission/cleanup through actual
pipeline teardown. One session owner cancels on Drop; observers do not own work.
The optional `media-playback` feature is now included by desktop. All769 original
locked package versions/checksums remain and14 packages were added.

Backend source06 has617 pinned inputs. Test05 passes4 parents and an isolated
lazy-init child. Test11 passes61 unfiltered library tests plus that actual child
(one ignored helper definition);5.69s runtime. Real decoder checks include paused
red/green frames, seek/rewind/burst/replay, original-path replacement with a FIFO,
ID3-adjusted WAV, independent sessions, failure and cancellation. All audio uses
a fake sink. Source06 scoped Standards/Spec reviews both report0 findings with
all11 hashes checked before/after. No native audio/rendering claim follows.

Test07 was deliberately interrupted before linking; test09 terminates101 when
the bin link fails with a linker bus error under disk pressure. Neither is a
test pass. Cache cleanups01/08 removed only classified reproducible Files caches;
relocations10/14 preserve byte-verified temporary copies and symlink paths. All
protected application/source snapshots remain. Test11 is the successful retry
with only the library target. Source06 hashes match before/after every run.

The Iced adapter now attaches the same application host and owns only rendering,
session handles, focus/drafts and source revision checks. It includes inline
Play/Pause, Seek, Mute, Volume and actual time; default Space/source navigation
stays consistent. Global notices also wake the final exit check after teardown.
Source12 all-target Clippy13 passes with warnings denied. Further fixes address
the already-prerolled first-frame attachment and Iced slider hover key capture;
their tests/Clippy/native checks remain pending. The current preserved executable
is still fc23, with the media fallback. Runtime/CI/attribution and native private
PCM/video/common-format verification remain open.

## Historical design checkpoint

The full Files goal remains active. The source-highlighting checkpoint is
verified/preserved in `syntax-verification.json` and `syntax-preservation-16.json`.
At the preparation checkpoint no Rust, Cargo, dependency or vendor change followed it. The then-current
fc23 executable still has the explicit audio/video fallback.

ADR0090 selects a worker-owned GStreamer playback session outside the renderer,
using the existing opened/bounded `MediaSource`. Iced consumes snapshots/frames
and sends commands; source/window/revision, focus and drafts stay in its adapter.
Initialization is lazy and session shutdown must retain process cleanup activity.
Typed bindings, activity/observer integration and production code are not added.

## Feasibility evidence

`media-pipeline-probe-01` and `media-pipeline-probe-02` contain exact scripts and
results. Their terminal handles are38578=0 and88679=0. Source files are generated
inside temporary directories and removed afterward. Audio always uses fakesink;
there is no actual audio-output or native UI claim. Decoder state reaches NULL.

Both variants use playbin3 with appsrc and a bounded RGBA appsink. A7548-byte VP8
source stays open while its pathname is replaced by a FIFO. Paused preroll is
red RGB(253,0,0) at0; accurate paused seek produces green RGB(3,122,5) at4 seconds;
rewind returns the actual first frame. Negotiated320x240 and6-second duration
are observed. The sink queue limit is1 and last-sample caching is disabled.

The random-access variant uses700 read/seek callbacks for this small fixture.
The seekable streaming variant uses5 reads and4 seeks, largest read4096, and
caps each request at64KiB. These are single-fixture observations, not benchmarks.
Random-access mode requires exact requested data and cannot be naively capped;
seekable streaming avoids that contract while supporting the observed seeks.
Large files, MP4 end metadata, short reads, failures and cancellation need tests.

Primary references:

- [Playbin appsrc/source setup](https://gstreamer.freedesktop.org/documentation/tutorials/playback/short-cutting-the-pipeline.html)
- [Appsrc/appsink scheduling, queue and clock contracts](https://gstreamer.freedesktop.org/documentation/application-development/advanced/pipeline-manipulation.html)
- [Maintained Rust AppSrc API](https://gstreamer.freedesktop.org/documentation/rust/stable/latest/docs/gstreamer_app/struct.AppSrc.html)

## Historical next implementation

Use official typed GStreamer/app/video bindings compatible with workspace glib0.22
and Rust1.95, under an optional playback feature. They are not cached locally;
updating the lockfile/fetch and attribution will be necessary. Keep existing
lockfile entries stable, and plan disk use before any compile/link (home last
about295MiB; tmp about1.3GiB). Do not delete protected binary aliases/snapshots.
No compiler/native/archive/cache handles remain live at this checkpoint.

Build a bounded decoder session with cancellable input, short-read/EOF/seek
handling, latest state/frame observation, worker-only commands and explicit drain
lifetime. Preserve MediaSource's ID3-adjusted length and file-instance authority.
Then connect Iced controls with existing focus/source guards. Port the maintained
private PCM/video fixture; verify no-autoplay, actual audio/video/seek/mute,
source replacement and owner closure, corrupt input and common formats in both
themes. Backend tests must not contact real audio devices. Existing GTK media
code and ADR0023-0025 are behavioral reference, not production architecture.

# 0090: Renderer-independent media playback

Status: scoped backend, Iced and native media verification, 2026-10-19.
Continues ADR0023/0024/0025/0059/0074. The new frontend passes four native media
cases in both appearances. Full product, accessibility, performance and release
acceptance remain open. The preserved fc23 executable is the earlier fallback
baseline; the verified media candidate is03ed1b63.

Keep the existing worker-opened `MediaSource` and its bounded decoder view.
Introduce a playback module outside `iced_desktop`. It owns decoder startup,
opened-file reads, transport state, seeking, volume, errors and shutdown. Iced
sends commands and renders immutable state and the latest frame. It owns widget
focus, slider drafts and native window/source revision guards. No decoder,
filesystem read or pipeline transition belongs in the view or update callback.

Use the distribution GStreamer runtime directly through its maintained Rust
bindings. GTK media widgets cease to own this feature. Do not add a production
Python worker, another player window or an external player's IPC protocol.
GStreamer initialization stays lazy: ordinary browsing and headless/resident
startup do not initialize media or scan decoder plugins. Keep a separate optional
playback feature so the portable core still builds without desktop libraries.
The optional `media-playback` feature uses the official GStreamer 0.25 bindings
and is included by `desktop`. Runtime declarations, hosted development packages
and the 585-package Linux dependency inventory are updated. Hosted and release
verification remain open.

A private decoder worker owns one playbin pipeline per active preview. Configure
its source using the fixed `appsrc://` route and the prepared reader, never the
selected pathname. Byte positions and size refer to the already bounded,
ID3-adjusted decoder view. Use seekable byte-stream mode, bounded input chunks
and a bounded source queue; satisfy cancellation, read errors and EOF explicitly.
Random-access pull mode requires the exact requested buffer length, so simply
capping those buffers would violate its contract. The worker serializes commands
and public state; source callbacks serialize reader positions.

Video passes through conversion/scaling to a bounded RGBA appsink, synchronized
to the pipeline clock. Keep at most one queued decoded sample and one latest
published frame; drop superseded frames. Validate negotiated dimensions, pixel
format and stride before copying. Iced fits that frame inside the preview and
never creates a decoder window. Audio uses the distribution's normal output
backend in production; automated checks retain the private audio server and sink.

Opening remains paused. Play/Pause, actual time seek, volume/mute, completion and
concise error feedback follow the shared preview focus/navigation contract.
Changing source or closing its owner cancels the old session, including a still
preparing decoder. The renderer must not block while a pipeline drains. The
process-owned runtime must retain cleanup activity until that session retires;
optional residency and Quit use the same lifetime rules. No old callback may
populate a newer selection. `Application` owns a lazy `Playback` host. Admission
takes an activity hold before spawning; it ends after worker/pipeline teardown.
Dropping a session cancels without joining on the UI thread. One coalesced notice
stream publishes current snapshots and wakes the host after cleanup. Losing
observation does not cancel an owned session; shutdown refuses while any worker
remains and closes further admission once idle.

The Iced adapter owns only the session handle, latest image handle, focus and
seek draft. Its controls remain visible below the preview and wrap in narrow
windows; volume/mute are inline rather than in a popover. Tab enters Open,
Play/Pause, Seek when supported, Mute and Volume. Focused seek/volume own arrows
and Home/End; otherwise the existing source-navigation keys retain their meaning.
Space activates a focused control or closes Quick Look with no focused control.
The slider widget receives pointer/touch input; its hover-based keyboard and
wheel handling is bypassed so it cannot steal navigation or leave an uncommitted
seek draft. Seek drags commit on release. Scoped native acceptance covers keyboard
controls, unfocused pointer-hover navigation and narrow floating layouts. Native
pointer/touch drag-release and wheel-suppression acceptance remain pending.

Relative seek/volume and mute toggling are backend commands. Iced never computes
their next value from an observed snapshot. The worker composes relative seeks
from the latest accepted target until actual decoder time confirms it; older
AsyncDone messages do not erase newer intent. While playing, confirmation also
bounds advancement by time since acceptance so a previous position after a
backward seek cannot be mistaken for the new timeline. Home/End and pointer
slider values remain absolute commands. A real-controller regression first
reproduced two Mute activations leaving mute enabled; the fix passes consecutive
mute, volume and seek commands without any intervening frontend poll.

Two disposable Python/GI probes use generated private VP8 files and a fake audio
sink, not the product. Both decode an actual paused red frame, seek to an actual
green frame and rewind while the original pathname is a FIFO. Both pipelines
reach NULL and release their reader. The seekable-mode variant limits input to
64 KiB chunks and the appsink to one frame. These checks establish feasibility
on installed GStreamer 1.28.7, not audible playback, production resource bounds,
Rust integration or native control/source-lifetime acceptance. See
[the preparation record](../../../.scratch/file-manager/ICED-MEDIA-WIP.md).

The first Rust backend run passes four real-decoder parents. The expanded run
passes all61 Iced/backend library tests plus its deliberately isolated lazy-init
child. They verify actual paused pixels, seek/rewind/burst seeks and replay,
retained input after pathname replacement by a FIFO, ID3-adjusted WAV input,
volume/mute command state, corrupt input, independent sessions and cancellation.
Every automated backend/controller session discards audio; this is not audible
or native rendering evidence. All769 previously locked package versions/checksums
remain, with14 new packages. Both scoped backend review axes report zero findings.

The expanded Iced/controller suite passes 64 parents and the isolated lazy child.
Actual fake-audio tests cover cached paused-frame attachment, media focus,
selection changes/closure and stale events, plus the consecutive-control
regression. The current public preview target passes 10 tests. Files all-target
Clippy with warnings denied, workspace formatting, Python syntax and the locked
dependency inventory check pass. The local-runtime library also checks without
default features; its normal dependency graph contains neither Iced nor GStreamer.

The four Iced media cases pass 34 grouped checks on candidate 03ed1b63: dark basic
in run 46, light basic in 47, and common formats in 48/49. They observe actual
isolated PCM, no autoplay, rapid volume input, mute, paused seek/rewind, advancing
VP8/H.264 pixels, independent source windows, source replacement and owner
closure. Every repeated tagged-MP3 cycle reaches enabled transport before close.
MP3, FLAC, Opus and AAC seeks must change the recorded tone and rewind restores
it. Final Browser closure retires the decoder before the owning process exits 0.
Both appearances' corrupt-media explanation and Open action were inspected in
the retained screenshots; the fixture separately verifies the actual Open argv.
The entire native matrix contains 150 scenarios; only these four are counted
here. The run 46 runner received SIGTERM during light acceptance, so its completed
dark per-case result is counted separately from the successful light retry.

The nested software desktop took 19.45 seconds to inject and verify a long path
through wtype during playback, versus 7.07 seconds with paused video. A longer
bounded diagnostic confirmed the entered bytes were intact. The maintained media
fixture pastes long paths through the normal location input and retains actual
keyboard control/navigation checks. This is no production latency result or
performance fix. Exact attempts, limits and evidence are recorded in
[the media checkpoint](../../../.scratch/file-manager/ICED-MEDIA-WIP.md).

Implementation must retain existing common-format and bounded-ID3 regressions,
then exercise actual private PCM, video pixels, paused seeking, volume/mute,
no-autoplay, focus, replacement/closure, corrupt input and both appearances in
the maintained native harness. Packaging and dependency attribution, decoder
shutdown under cancellation, accessibility and performance remain separate gates.

The design follows the primary GStreamer documentation for
[playbin source setup](https://gstreamer.freedesktop.org/documentation/tutorials/playback/short-cutting-the-pipeline.html)
and [appsrc/appsink scheduling and queues](https://gstreamer.freedesktop.org/documentation/application-development/advanced/pipeline-manipulation.html).

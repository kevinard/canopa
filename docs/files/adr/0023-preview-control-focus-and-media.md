# Quick Look preserves browsing and native media controls

Status: accepted under delegated implementation authority; implementation and
acceptance in progress, 2026-10-16.

Quick Look opens with focus on its preview area. Browsing keys continue to reach
the originating browser and Space dismisses the preview from that default focus.
Tab can enter native controls. Controls receive activation, seeking, scrolling
and text navigation before the preview handles unconsumed keys. This uses GTK's
normal bubbling phase rather than a list of widget-specific keyboard exceptions.
Space on a focused control activates it. Escape returns to the originating
browser; native popovers get their own opportunity to handle Escape first.
Before replacing content that owns the current focus, the preview focuses its
area again. Otherwise removing a media control lets GTK select the header's Open
button, and the next Space unexpectedly opens the file. Native acceptance
reproduced this exact transition from focused video controls to text.

This applies to the shared Quick Look used by Browser, Trash and Recovery, including
image/PDF controls. It preserves source ownership and the independent 760 × 600
floating window beside a 360-pixel browser. It does not introduce a second media
player or new application-owned decoding service.

Audio and video retain GTK's native MediaFile and MediaControls backend. Video
uses the stream's native paintable in a Picture with an always-visible transport
bar, matching audio. The installed GtkVideo exposed only a nonfocusable overlay
Play button while paused; native Tab acceptance reproduced that gap even after
checking every Play control. The paintable/control composition follows GTK's
documented interface and avoids depending on its private overlay widget tree.
Opening
a preview does not autoplay. Changing selection and closing Quick Look stop the
previous stream. Playback uses the installed GTK media backend and its codecs;
the runtime package must provide media support, not just GTK's widget library.
Unsupported or corrupt media must remain understandable and allow opening in
the normal application. A short playback explanation stays in the preview;
backend pipeline/debug details go to the diagnostic log. The current error is
checked when controls are created as well as on later notifications, so an
immediate backend failure is visible too. Error callbacks retain only a weak
label reference after the content is replaced.

Native acceptance uses generated WAV and VP8/Vorbis WebM files. It starts private
PipeWire/Pulse services, a null sink, and policy-only WirePlumber without hardware
monitors. Capture is calibrated with real PCM before testing the application.
Pulse and PipeWire addresses stay inside the private runtime; ALSA's default
fallback is null and SDL audio is restricted to Pulse. The fixture never targets
the user's speakers. A private accessibility bus observes user-visible controls;
actual Tab and Space perform their keyboard interactions. It does not use private
application state or expose test hooks.

Assertions use fresh recorded PCM for playback/stop, a distinct video tone and
actual video pixels for decoding/progression, and public slider values for seeking.
The monitor requests 20 ms latency so stopped audio can be distinguished from
old buffered samples. GTK's installed audio control uses the label Stop while
playing; tests verify its audible behavior rather than assuming the label Pause.
The native runner preflight includes the required media-fixture tools.

The first failing native case reproduced Space closing a focused Play control.
After the focus correction, native playback produced real PCM without autoplay.
A subsequent case reproduced Right being diverted from the seek slider into
browser navigation. Bubbling after native controls fixes that case; the five
audio/stop/seek checks pass in `native-media-07`. `native-media-video-10` passes
ten checks including actual VP8 frames, the distinct Vorbis tone, source-follow
shutdown and close behavior. `native-media-controls-14` passes fourteen checks
including volume, popover Escape, paused video seeking and corrupt media.
`native-media-focus-red-15` reproduces focus falling back to Open after replacing
controls. `native-media-16` (dark) and `native-media-17` (light, 1.25×) pass all
sixteen audio/video/control/focus/error checks after the fix. The existing
27-check desktop regression passes in `native-acceptance-33`.

Final `native-media-owner-18` (dark) and `native-media-20` (light, 1.25×) each pass
all seventeen checks, adding playback across another browser's focus and actual
audio shutdown when the source browser closes. `native-media-19` received SIGTERM
during its initial playback checks and was repeated; it is retained as interrupted
evidence. `native-acceptance-34` passes the final 27-check desktop regression at
light/1.25×. Native build, all-target Clippy, formatting, Python syntax, runner
prerequisites, maintained inventory and whitespace checks pass. No GTK warnings
or assertions appear in the final Files logs; the deliberately corrupt media's
decoder diagnostic is retained under the application-owned log prefix. Rendered
inspection confirms readable always-visible transport and concise error feedback
within the floating preview. The prior 99-test core checkpoint is unchanged;
this work changes desktop presentation/input and native acceptance.

Fixture diagnostics also distinguish actual behavior from backend assumptions.
GTK exposes the playing audio button as Stop; its operation is verified through
PCM rather than an assumed Pause label. At a 0.3 slider setting the recorded tone
had a 162 peak, so a full-volume threshold would falsely report silence. Volume
acceptance records the public slider values and tests mute/restoration through
fresh PCM; it restores full volume before checking the next stream. Video's
distinct 660 Hz track prevents the preceding 440 Hz audio from masquerading as
successful video audio.

[ADR 0024](0024-preview-opened-media.md) replaces pathname reopening with the
worker-opened regular file and records the seekable input-stream integration.
Binding browsing intent before open, format/backend packaging and remaining
preview control-focus acceptance remain part of the broader implementation.

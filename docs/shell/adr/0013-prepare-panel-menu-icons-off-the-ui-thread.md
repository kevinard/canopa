# Prepare Panel Menu Icons Off the UI Thread

Panel Menu rendering reads prepared RGBA handles from an application-owned cache.
It never resolves icon names, reads image files, decodes raster images or parses
SVGs. Previously each new row at the virtualized range boundary could do this work
synchronously, causing touchpad scrolling to pause despite continuous input.

One blocking worker prepares at most four distinct icons per request. It prioritizes
visible rows, then overscan, using the same row range as rendering. After completion,
the next request uses the current menu and scroll position; there is no backlog of
requests for earlier positions. Pending and missing icons reserve the same 18-pixel
slot. Completion updates only the name cache, so results arriving after search,
replacement or close cannot change selection, scroll position or foreground state.

The cache holds at most 1,024 entries, including misses, and evicts oldest entries.
Raster files use the shared bounded asset mechanism (8 MiB file, 2,048-pixel dimension,
4,194,304 source pixels, 32 MiB decoder allocation including output). SVGs use bounded
8 MiB reads and the same resvg version already used by Iced, on the worker. SVG
parser/filter allocations are not a hard memory bound. Both produce thumbnails at
most 72 pixels per side, preserving aspect ratio and straight RGBA alpha, for the
18-pixel slot through 4x scale. Higher output scales upscale these thumbnails.

This extends shared ADR 0005 for Panel Menu scheduling and SVG rasterization only.
The shared asset crate stays synchronous and GUI-independent; other shell icon
consumers retain their existing behavior. Theme lookup policy is unchanged. Icons
may appear shortly after their labels on a cold cache; scrolling never waits for
icon preparation.

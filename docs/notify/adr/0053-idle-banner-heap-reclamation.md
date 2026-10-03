# Reclaim free allocator pages after banner surfaces retire

Idle graphical Notify retained roughly 33 MiB of free heap pages after its startup
probe closed in a release build using Mesa software GL. Releasing those pages with
GNU libc's `malloc_trim(0)` reduced RSS and PSS without changing the GPU renderer,
surface transparency, or live notification state.

After the last banner surface closes, wait two seconds, then request best-effort
reclamation on the daemon's blocking executor. Opening a new stack cancels the
delay. A matching retired-window identity and the current idle surface state guard
against stale delayed messages. Duplicate close events do not schedule work.
Admission is bounded to one reclamation job per process, including queued jobs;
once admitted, it may finish while a new surface opens. It never blocks the UI
executor. Startup readiness and first-frame deadlines retain their existing meaning.

This path is limited to Linux with GNU libc. Other targets retain their allocator's
normal behavior. `malloc_trim` is MT-safe and only releases eligible unused pages;
it does not invalidate live allocations. See the
[Linux manual](https://www.man7.org/linux/man-pages/man3/malloc_trim.3.html).
There is no recurring idle timer or allocator tuning through environment variables.
The size of the saving depends on the allocator, driver, build, and workload.

The surface regression checks grace-period waiting, cancellation when content
returns, and duplicate/stale close handling. Release measurements and graphical
acceptance evidence are recorded in [the performance report](../../performance.md).

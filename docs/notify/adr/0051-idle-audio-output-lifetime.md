# Idle audio output lifetime

An empty Rodio mixer still drives its device callback with silence. Keeping that
stream open for the daemon lifetime consumed about 1.9% of one CPU core on the
reported PipeWire/ALSA session, including in a fresh daemon with no notifications.

Keep sound capability separate from an open output stream. Probe availability
when enabling sound or retrying unavailable output, then release the probe. Open
the shared stream on an admitted sound worker and retire it when no sound jobs
remain. A queued source's destruction wakes its worker; stream teardown happens
there, never on the audio callback that teardown must join. Device playback keeps
its reservation for a further 250 ms to allow buffered PCM to drain. This is a
best-effort allowance, not a device latency guarantee. There is no idle polling.

Admission and retirement share the presenter's state lock, so completion cannot
close the stream under a newly admitted sound. Preserve ADR 0034's four-job bound,
cancellation generations, and file/PCM limits. Idle retirement preserves sound
capability; output-open failure degrades it until settings are reapplied, as before.

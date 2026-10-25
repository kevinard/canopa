# Preview color fixture

The project-generated six-second VP8 WebM contains three seconds of red followed
by three seconds of green, 320 × 240 at 10 frames per second, without audio.
No external source media is used. It follows the project's source licenses.
Playback tests use a fake audio sink and actual decoded pixels.

Regenerate from the workspace root with FFmpeg and its libvpx encoder:

    ffmpeg -v error -filter_complex_threads 1 \
      -f lavfi -i color=c=red:s=320x240:r=10:d=3 \
      -f lavfi -i color=c=green:s=320x240:r=10:d=3 \
      -filter_complex '[0:v][1:v]concat=n=2:v=1:a=0[v]' -map '[v]' \
      -c:v libvpx -threads 1 -deadline realtime -cpu-used 8 \
      -fflags +bitexact -flags:v +bitexact \
      crates/canopa-files/tests/fixtures/preview-colors.webm

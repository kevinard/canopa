//! Pipeline transitions, transport and sample mapping stay on the media worker.
use super::*;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use gstreamer_video::{self as video, prelude::*};
use std::time::Instant;

struct Decoder {
    pipeline: gst::Element,
    sink: AppSink,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // May wait for streaming tasks. The caller is a worker, never the view.
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl Decoder {
    fn new(source: MediaSource, shared: &Shared, audio: AudioOutput) -> Result<Self, String> {
        gst::init().map_err(|e| e.to_string())?;
        if shared.cancellation.is_cancelled() {
            return Err(fl!("files-media-cancelled"));
        }
        let input =
            input::Input::new(source, shared.cancellation.clone()).map_err(|e| e.to_string())?;
        let pipeline = gst::ElementFactory::make("playbin3")
            .property("uri", "appsrc://")
            .build()
            .map_err(|e| e.to_string())?;
        let bin = gst::parse::bin_from_description(
            "videoconvert ! videoscale ! video/x-raw,format=RGBA,pixel-aspect-ratio=1/1,width=(int)[1,1280],height=(int)[1,720] ! appsink name=frames max-buffers=1 drop=true sync=true enable-last-sample=false",
            true,
        )
        .map_err(|e| e.to_string())?;
        let sink = bin
            .by_name("frames")
            .and_then(|element| element.downcast::<AppSink>().ok())
            .ok_or(fl!("files-media-no-video-output"))?;
        pipeline.set_property("video-sink", &bin);
        match audio {
            AudioOutput::System => {}
            #[cfg(test)]
            AudioOutput::Discard => {
                let sink = gst::ElementFactory::make("fakesink")
                    .property("sync", true)
                    .build()
                    .map_err(|e| e.to_string())?;
                pipeline.set_property("audio-sink", sink);
            }
        }
        pipeline.connect("source-setup", false, move |values| {
            if let Ok(element) = values[1].get::<gst::Element>()
                && let Ok(app) = element.downcast::<gstreamer_app::AppSrc>()
            {
                input.attach(&app);
            }
            None
        });
        let decoder = Self { pipeline, sink };
        decoder
            .pipeline
            .set_state(gst::State::Paused)
            .map_err(|e| e.to_string())?;
        Ok(decoder)
    }
}

pub(super) fn run(
    source: MediaSource,
    shared: Arc<Shared>,
    commands: mpsc::Receiver<Command>,
    audio: AudioOutput,
) {
    let mut state = Snapshot::default();
    let result = if shared.cancellation.is_cancelled() {
        Ok(())
    } else {
        play(source, &shared, commands, audio, &mut state)
    };
    state.seeking = false;
    if shared.cancellation.is_cancelled() {
        state.phase = Phase::Stopped;
        state.frame = None;
    } else if let Err(error) = result {
        eprintln!("[canopa-files media] {error}");
        state.phase = Phase::Failed(fl!("files-media-cannot-play"));
    }
    shared.publish(state);
}

fn play(
    source: MediaSource,
    shared: &Shared,
    commands: mpsc::Receiver<Command>,
    audio: AudioOutput,
    state: &mut Snapshot,
) -> Result<(), String> {
    let decoder = Decoder::new(source, shared, audio)?;
    let pipeline = &decoder.pipeline;
    let bus = pipeline
        .bus()
        .ok_or("The media decoder has no message bus.")?;
    let started = Instant::now();
    let mut prepared = false;
    let mut playing = false;
    let mut ended = false;
    let mut requested_position = None;
    let mut last_query = Instant::now() - Duration::from_secs(1);
    let mut frame_key = None;
    let mut serial = 0;
    loop {
        if shared.cancellation.is_cancelled() {
            return Ok(());
        }
        let mut dirty = false;
        for command in commands.try_iter().take(32) {
            state.notice = None;
            dirty = true;
            let seek = match command {
                Command::Playing(value) => {
                    playing = value;
                    None
                }
                Command::TogglePlayback => {
                    playing = !playing;
                    None
                }
                Command::Seek(position) => Some(position),
                Command::SeekBy(seconds) => {
                    let base = requested_position
                        .map(|(position, _)| position)
                        .unwrap_or_else(|| {
                            pipeline
                                .query_position::<gst::ClockTime>()
                                .map(|t| Duration::from_nanos(t.nseconds()))
                                .unwrap_or(state.position)
                        });
                    let distance = Duration::from_secs(seconds.unsigned_abs());
                    Some(if seconds < 0 {
                        base.saturating_sub(distance)
                    } else {
                        base.saturating_add(distance)
                    })
                }
                Command::Volume(volume) | Command::VolumeBy(volume) => {
                    let volume = if matches!(command, Command::VolumeBy(_)) {
                        (state.volume + volume).clamp(0.0, 1.0)
                    } else {
                        volume
                    };
                    pipeline.set_property("volume", volume);
                    state.volume = volume;
                    None
                }
                Command::Muted(_) | Command::ToggleMuted => {
                    let muted = if let Command::Muted(muted) = command {
                        muted
                    } else {
                        !state.muted
                    };
                    pipeline.set_property("mute", muted);
                    state.muted = muted;
                    None
                }
            };
            if let Some(position) = seek {
                let position = position
                    .min(state.duration.unwrap_or(Duration::MAX))
                    .min(Duration::from_nanos(u64::MAX - 1));
                if state.seekable
                    && pipeline
                        .seek_simple(
                            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                            gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                        )
                        .is_ok()
                {
                    state.seeking = true;
                    frame_key = None;
                    ended = false;
                    requested_position = Some((position, Instant::now()));
                } else {
                    state.notice = Some(fl!("files-media-no-seek"));
                }
            }
            if playing && ended {
                pipeline
                    .seek_simple(
                        gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                        gst::ClockTime::ZERO,
                    )
                    .map_err(|e| e.to_string())?;
                ended = false;
                state.seeking = true;
                requested_position = Some((Duration::ZERO, Instant::now()));
                frame_key = None;
            }
            pipeline
                .set_state(if playing {
                    gst::State::Playing
                } else {
                    gst::State::Paused
                })
                .map_err(|e| e.to_string())?;
        }
        // Keep both commands and cancellation responsive under a busy decoder.
        for message in bus
            .timed_pop(gst::ClockTime::from_mseconds(20))
            .into_iter()
            .chain(bus.iter())
            .take(64)
        {
            match message.view() {
                gst::MessageView::Error(error) => {
                    return Err(format!("{}; {:?}", error.error(), error.debug()));
                }
                gst::MessageView::Eos(_) => {
                    ended = true;
                    playing = false;
                    pipeline
                        .set_state(gst::State::Paused)
                        .map_err(|e| e.to_string())?;
                }
                gst::MessageView::AsyncDone(_) => {
                    prepared = true;
                    dirty |= state.seeking;
                    state.seeking = false;
                }
                _ => {}
            }
        }
        let phase = if ended {
            Phase::Ended
        } else {
            match pipeline.current_state() {
                gst::State::Playing => {
                    prepared = true;
                    Phase::Playing
                }
                gst::State::Paused => {
                    prepared = true;
                    Phase::Paused
                }
                _ => Phase::Preparing,
            }
        };
        if state.phase != phase {
            state.phase = phase;
            dirty = true;
        }
        if !prepared && started.elapsed() > Duration::from_secs(10) {
            return Err(fl!("files-media-timed-out"));
        }
        if prepared && last_query.elapsed() >= Duration::from_millis(100) {
            last_query = Instant::now();
            // Queries fail while a flushing seek is in flight. A failure is
            // no new answer: keep the last duration and seekability, so the
            // seek control (and its keyboard focus) survives a slow seek.
            let duration = answered(
                pipeline
                    .query_duration::<gst::ClockTime>()
                    .map(|t| Some(Duration::from_nanos(t.nseconds()))),
                state.duration,
            );
            let position = pipeline
                .query_position::<gst::ClockTime>()
                .map(|t| Duration::from_nanos(t.nseconds()))
                .unwrap_or(state.position);
            let mut query = gst::query::Seeking::new(gst::Format::Time);
            let seekable = answered(
                pipeline.query(&mut query).then(|| query.result().0),
                state.seekable,
            );
            // An AsyncDone from an earlier transition must not discard a newer
            // seek intent. Compose relative commands until actual decoder time
            // reaches the accepted target; playing time can already be beyond it.
            if requested_position.is_some_and(|(target, accepted)| {
                position.abs_diff(target) <= Duration::from_millis(50)
                    || (playing
                        && position >= target
                        && position - target <= accepted.elapsed() + Duration::from_millis(50))
            }) {
                requested_position = None;
            }
            if state.duration != duration
                || state.position != position
                || state.seekable != seekable
            {
                state.duration = duration;
                state.position = position;
                state.seekable = seekable;
                dirty = true;
            }
        }
        let sample = decoder
            .sink
            .try_pull_preroll(gst::ClockTime::ZERO)
            .or_else(|| decoder.sink.try_pull_sample(gst::ClockTime::ZERO));
        if let Some(sample) = sample {
            let buffer = sample.buffer().ok_or("The video frame has no pixels.")?;
            let key = (buffer.as_ptr() as usize, buffer.pts());
            if frame_key != Some(key) {
                serial += 1;
                state.frame = Some(frame(&sample, serial)?);
                frame_key = Some(key);
                dirty = true;
            }
        }
        if dirty {
            shared.publish(state.clone());
        }
    }
}

/// A pipeline query's answer, or the previous one when the query failed.
fn answered<T>(answer: Option<T>, previous: T) -> T {
    answer.unwrap_or(previous)
}

fn frame(sample: &gst::Sample, serial: u64) -> Result<Frame, String> {
    let info = video::VideoInfo::from_caps(sample.caps().ok_or("Missing video format.")?)
        .map_err(|e| e.to_string())?;
    let (width, height) = (info.width(), info.height());
    if info.format() != video::VideoFormat::Rgba
        || width == 0
        || height == 0
        || width > 1280
        || height > 720
    {
        return Err("Unsupported video frame dimensions or format.".into());
    }
    let buffer = sample.buffer().ok_or("The video frame has no pixels.")?;
    let mapped =
        video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).map_err(|e| e.to_string())?;
    let row = width as usize * 4;
    let stride =
        usize::try_from(mapped.plane_stride()[0]).map_err(|_| "Invalid video row stride.")?;
    let bytes = mapped.plane_data(0).map_err(|e| e.to_string())?;
    let needed = stride
        .checked_mul(height as usize - 1)
        .and_then(|size| size.checked_add(row))
        .ok_or("Invalid video frame size.")?;
    if stride < row || bytes.len() < needed {
        return Err("Incomplete video frame pixels.".into());
    }
    let mut rgba = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        rgba.extend_from_slice(&bytes[y * stride..y * stride + row]);
    }
    Ok(Frame {
        serial,
        bitmap: crate::preview::Bitmap {
            width,
            height,
            rgba: rgba.into(),
        },
        timestamp: buffer.pts().map(|t| Duration::from_nanos(t.nseconds())),
    })
}

#[cfg(test)]
mod answer_tests {
    use super::*;

    #[test]
    fn failed_queries_during_a_seek_keep_the_previous_answers() {
        let known = Some(Duration::from_secs(30));
        assert_eq!(answered(None, known), known);
        let seeked = Some(Duration::from_secs(12));
        assert_eq!(answered(Some(seeked), known), seeked);
        assert_eq!(answered(None, None::<Duration>), None);
        assert!(answered(None, true));
        assert!(!answered(Some(false), true));
    }
}

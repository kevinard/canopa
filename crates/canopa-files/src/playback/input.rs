//! Bounded seekable input; decoder callbacks never reopen a pathname.
use super::*;
use gstreamer as gst;
use gstreamer_app::{AppSrc, AppSrcCallbacks, AppStreamType};
use std::io::{Read, Seek, SeekFrom};

const CHUNK_BYTES: usize = 64 * 1024;

trait Reader: Read + Seek + Send {}
impl<T: Read + Seek + Send> Reader for T {}

pub(super) struct Input {
    reader: Mutex<Box<dyn Reader>>,
    length: u64,
    cancellation: Cancellation,
}

impl Input {
    pub fn new(source: MediaSource, cancellation: Cancellation) -> io::Result<Arc<Self>> {
        let mut reader = source.reader();
        let length = reader.seek(SeekFrom::End(0))?;
        i64::try_from(length)
            .map_err(|_| io::Error::other("The media decoder input is too large."))?;
        reader.seek(SeekFrom::Start(0))?;
        Ok(Arc::new(Self {
            reader: Mutex::new(Box::new(reader)),
            length,
            cancellation,
        }))
    }

    pub fn attach(self: &Arc<Self>, app: &AppSrc) {
        app.set_format(gst::Format::Bytes);
        app.set_stream_type(AppStreamType::Seekable);
        app.set_size(self.length as i64);
        app.set_max_bytes((2 * CHUNK_BYTES) as u64);
        let feed = self.clone();
        let seek = self.clone();
        app.set_callbacks(
            AppSrcCallbacks::builder()
                .need_data(move |app, requested| {
                    if feed.cancellation.is_cancelled() {
                        return;
                    }
                    let amount = (requested as usize).clamp(1, CHUNK_BYTES);
                    let mut data = vec![0; amount];
                    let result = loop {
                        if feed.cancellation.is_cancelled() {
                            return;
                        }
                        let result = feed
                            .reader
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .read(&mut data);
                        if !matches!(&result, Err(e) if e.kind() == io::ErrorKind::Interrupted) {
                            break result;
                        }
                    };
                    match result {
                        Ok(0) => {
                            let _ = app.end_of_stream();
                        }
                        Ok(count) => {
                            data.truncate(count);
                            if !feed.cancellation.is_cancelled() {
                                let _ = app.push_buffer(gst::Buffer::from_mut_slice(data));
                            }
                        }
                        Err(error) => {
                            gst::element_error!(app, gst::ResourceError::Read, ["{error}"]);
                        }
                    }
                })
                .seek_data(move |_, offset| {
                    !seek.cancellation.is_cancelled()
                        && offset <= seek.length
                        && seek
                            .reader
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .seek(SeekFrom::Start(offset))
                            .is_ok()
                })
                .build(),
        );
    }
}

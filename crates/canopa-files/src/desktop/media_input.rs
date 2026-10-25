//! A media decoder needs both seeking and an accurate current byte position.
use gtk::{gio, glib, subclass::prelude::*};
use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::Mutex,
};

trait Reader: Read + Seek + Send {}
impl<T: Read + Seek + Send> Reader for T {}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MediaInput {
        pub(super) reader: Mutex<Option<Box<dyn Reader>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaInput {
        const NAME: &'static str = "CanopaFilesMediaInput";
        type Type = super::MediaInput;
        type ParentType = gio::InputStream;
        type Interfaces = (gio::Seekable,);
    }
    impl ObjectImpl for MediaInput {}

    impl MediaInput {
        fn with_reader<T>(
            &self,
            cancellable: Option<&gio::Cancellable>,
            mut operation: impl FnMut(&mut dyn Reader) -> io::Result<T>,
        ) -> Result<T, glib::Error> {
            use gio::prelude::*;
            let mut reader = self.reader.lock().map_err(|_| {
                glib::Error::new(gio::IOErrorEnum::Failed, "Media reader unavailable")
            })?;
            let reader = reader
                .as_mut()
                .ok_or_else(|| glib::Error::new(gio::IOErrorEnum::Closed, "Media reader closed"))?;
            loop {
                if let Some(cancellable) = cancellable {
                    cancellable.set_error_if_cancelled()?;
                }
                match operation(reader.as_mut()) {
                    Ok(value) => return Ok(value),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        let kind = match error.kind() {
                            io::ErrorKind::InvalidInput => gio::IOErrorEnum::InvalidArgument,
                            io::ErrorKind::PermissionDenied => gio::IOErrorEnum::PermissionDenied,
                            _ => gio::IOErrorEnum::Failed,
                        };
                        return Err(glib::Error::new(kind, &error.to_string()));
                    }
                }
            }
        }
    }
    impl gio::subclass::prelude::InputStreamImpl for MediaInput {
        fn read(
            &self,
            buffer: &mut [u8],
            cancellable: Option<&gio::Cancellable>,
        ) -> Result<usize, glib::Error> {
            self.with_reader(cancellable, |reader| reader.read(buffer))
        }
        fn close(&self, _: Option<&gio::Cancellable>) -> Result<(), glib::Error> {
            self.reader
                .lock()
                .map_err(|_| {
                    glib::Error::new(gio::IOErrorEnum::Failed, "Media reader unavailable")
                })?
                .take();
            Ok(())
        }
    }
    impl gio::subclass::prelude::SeekableImpl for MediaInput {
        fn tell(&self) -> i64 {
            self.with_reader(None, |reader| reader.stream_position())
                .ok()
                .and_then(|position| i64::try_from(position).ok())
                .unwrap_or(-1)
        }
        fn can_seek(&self) -> bool {
            self.reader.lock().is_ok_and(|reader| reader.is_some())
        }
        fn seek(
            &self,
            offset: i64,
            kind: glib::SeekType,
            cancellable: Option<&gio::Cancellable>,
        ) -> Result<(), glib::Error> {
            let from = match kind {
                glib::SeekType::Set if offset >= 0 => SeekFrom::Start(offset as u64),
                glib::SeekType::Cur => SeekFrom::Current(offset),
                glib::SeekType::End => SeekFrom::End(offset),
                _ => {
                    return Err(glib::Error::new(
                        gio::IOErrorEnum::InvalidArgument,
                        "Invalid media position",
                    ));
                }
            };
            self.with_reader(cancellable, |reader| reader.seek(from))
                .map(|_| ())
        }
        fn can_truncate(&self) -> bool {
            false
        }
        fn truncate(&self, _: i64, _: Option<&gio::Cancellable>) -> Result<(), glib::Error> {
            Err(glib::Error::new(
                gio::IOErrorEnum::NotSupported,
                "Media input is read-only",
            ))
        }
    }
}

glib::wrapper! {
    pub struct MediaInput(ObjectSubclass<imp::MediaInput>)
        @extends gio::InputStream, @implements gio::Seekable;
}
impl MediaInput {
    pub(super) fn new(source: crate::preview::MediaSource) -> Self {
        let stream: Self = glib::Object::new();
        *stream.imp().reader.lock().expect("new media reader") = Some(Box::new(source.reader()));
        stream
    }
}

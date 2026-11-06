//! One immutable frame from the exact producer serial, without mapping its memory.
use super::super::{MAX_BYTES, MAX_PIXELS, MAX_SIDE};
use pipewire::{self as pw, spa};
use std::{
    cell::RefCell,
    fs::File,
    io::Cursor,
    os::{fd::BorrowedFd, unix::fs::FileExt},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::watch;

pub(super) async fn capture(
    serial: u64,
    compositor_pid: u32,
    mut cancelled: watch::Receiver<bool>,
) -> anyhow::Result<image::RgbaImage> {
    let stop = Arc::new(AtomicBool::new(false));
    struct Stop(Arc<AtomicBool>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let guard = Stop(stop.clone());
    let mut task = tokio::task::spawn_blocking(move || read(serial, compositor_pid, stop));
    tokio::select! {
        biased;
        _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => {
            guard.0.store(true, Ordering::Relaxed);
            // The caller is a retained capture task: admission stays occupied
            // until the actual PipeWire reader and producer have retired.
            let _ = task.await;
            anyhow::bail!("Window screenshot cancelled")
        }
        result = &mut task => result?,
    }
}

#[derive(Clone, Copy)]
struct Layout {
    width: u32,
    height: u32,
    stride: u32,
    offset: u32,
    bgra: bool,
}
impl Layout {
    fn checked(
        width: u32,
        height: u32,
        stride: i32,
        offset: u32,
        size: u32,
        max: u32,
        bgra: bool,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=MAX_SIDE).contains(&width)
                && (1..=MAX_SIDE).contains(&height)
                && u64::from(width) * u64::from(height) <= MAX_PIXELS as u64,
            "Window screenshot exceeds pixel limits"
        );
        let stride = u32::try_from(stride)?;
        let needed = u64::from(height - 1) * u64::from(stride) + u64::from(width) * 4;
        anyhow::ensure!(
            stride >= width * 4
                && max as usize <= MAX_BYTES
                && offset <= max
                && size <= max - offset
                && needed <= u64::from(size),
            "Invalid window screenshot buffer layout"
        );
        Ok(Self {
            width,
            height,
            stride,
            offset,
            bgra,
        })
    }
    fn copy(
        self,
        file: &File,
        map_offset: u32,
        stop: &AtomicBool,
    ) -> anyhow::Result<image::RgbaImage> {
        let mut image = image::RgbaImage::new(self.width, self.height);
        let row_bytes = self.width as usize * 4;
        for (y, row) in image.as_mut().chunks_mut(row_bytes).enumerate() {
            anyhow::ensure!(!stop.load(Ordering::Relaxed), "Window screenshot cancelled");
            let offset =
                u64::from(map_offset) + u64::from(self.offset) + y as u64 * u64::from(self.stride);
            // pread returns an error on truncation; a producer-owned mapping
            // could otherwise SIGBUS the portal if its backing file shrinks.
            file.read_exact_at(row, offset)?;
            for pixel in row.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                if !self.bgra {
                    pixel[3] = 255;
                }
            }
        }
        Ok(image)
    }
}

fn pod(object: spa::pod::Object) -> anyhow::Result<Vec<u8>> {
    Ok(spa::pod::serialize::PodSerializer::serialize(
        Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .map_err(|_| anyhow::anyhow!("Invalid window capture format"))?
    .0
    .into_inner())
}
fn pixels(
    data: &spa::buffer::Data,
    format: spa::param::video::VideoInfoRaw,
    stop: &AtomicBool,
) -> anyhow::Result<image::RgbaImage> {
    let raw = data.as_raw();
    anyhow::ensure!(
        data.type_() == spa::buffer::DataType::MemFd && !raw.chunk.is_null(),
        "Window capture needs shared-memory pixels"
    );
    let chunk = data.chunk();
    anyhow::ensure!(
        !chunk.flags().contains(spa::buffer::ChunkFlags::CORRUPTED),
        "Corrupt window capture"
    );
    let size = format.size();
    let layout = Layout::checked(
        size.width,
        size.height,
        chunk.stride(),
        chunk.offset(),
        chunk.size(),
        raw.maxsize,
        format.format() == spa::param::video::VideoFormat::BGRA,
    )?;
    let descriptor = i32::try_from(raw.fd)?;
    anyhow::ensure!(descriptor >= 0, "Missing window screenshot descriptor");
    // SAFETY: the PipeWire buffer owns this descriptor throughout the callback;
    // duplicate it before the buffer can be queued back to the producer.
    let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
    let file = File::from(rustix::io::fcntl_dupfd_cloexec(borrowed, 3)?);
    let filesystem = rustix::fs::fstatfs(&file)?;
    anyhow::ensure!(
        filesystem.f_type as u64 == libc::TMPFS_MAGIC as u64 && file.metadata()?.is_file(),
        "Window screenshot descriptor is not shared memory"
    );
    layout.copy(&file, raw.mapoffset, stop)
}
fn require_producer(
    core: &pw::core::CoreRc,
    mainloop: &pw::main_loop::MainLoopRc,
    serial: u64,
    pid: u32,
    stop: &AtomicBool,
) -> anyhow::Result<()> {
    use std::{cell::Cell, collections::HashMap};
    let registry = core.get_registry()?;
    let nodes = Rc::new(RefCell::new(Vec::new()));
    let clients = Rc::new(RefCell::new(HashMap::new()));
    let count = Rc::new(Cell::new(0usize));
    let failed = Rc::new(Cell::new(false));
    let seen_nodes = nodes.clone();
    let seen_clients = clients.clone();
    let globals = count.clone();
    let invalid = failed.clone();
    let _registry = registry
        .add_listener_local()
        .global(move |global| {
            globals.set(globals.get() + 1);
            if globals.get() > 4096 {
                invalid.set(true);
                return;
            }
            let Some(props) = global.props.as_ref() else {
                return;
            };
            if global.type_ == pw::types::ObjectType::Client {
                if let Some(pid) = props
                    .get("pipewire.sec.pid")
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    seen_clients.borrow_mut().insert(global.id, pid);
                }
            } else if global.type_ == pw::types::ObjectType::Node
                && props
                    .get("object.serial")
                    .and_then(|v| v.parse::<u64>().ok())
                    == Some(serial)
            {
                if let Some(client) = props.get("client.id").and_then(|v| v.parse::<u32>().ok()) {
                    seen_nodes.borrow_mut().push(client);
                } else {
                    invalid.set(true);
                }
            }
        })
        .register();
    let done = Rc::new(Cell::new(false));
    let finished = done.clone();
    let errors = failed.clone();
    let sequence = core.sync(0)?;
    let _core = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == pw::core::PW_ID_CORE && seq == sequence {
                finished.set(true);
            }
        })
        .error(move |_, _, _, _| errors.set(true))
        .register();
    let end = Instant::now() + Duration::from_secs(5);
    while !done.get() {
        anyhow::ensure!(
            !failed.get() && !stop.load(Ordering::Relaxed) && Instant::now() < end,
            "Window producer identity unavailable"
        );
        mainloop.loop_().iterate(Duration::from_millis(20));
    }
    let nodes = nodes.borrow();
    let clients = clients.borrow();
    anyhow::ensure!(
        !failed.get() && nodes.len() == 1 && clients.get(&nodes[0]) == Some(&pid),
        "Window producer belongs to another process"
    );
    Ok(())
}
fn read(
    serial: u64,
    compositor_pid: u32,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<image::RgbaImage> {
    anyhow::ensure!(
        serial != 0 && !stop.load(Ordering::Relaxed),
        "Window source unavailable"
    );
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(pw::init);
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    require_producer(&core, &mainloop, serial, compositor_pid, &stop)?;
    let stream = pw::stream::StreamBox::new(
        &core,
        "Canopa window screenshot",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video", *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen", *pw::keys::TARGET_OBJECT => serial.to_string(),
        },
    )?;
    struct State {
        format: Option<spa::param::video::VideoInfoRaw>,
        result: Rc<RefCell<Option<anyhow::Result<image::RgbaImage>>>>,
        stop: Arc<AtomicBool>,
    }
    let result = Rc::new(RefCell::new(None));
    let failures = result.clone();
    let _core = core
        .add_listener_local()
        .error(move |_, _, _, _| {
            if failures.borrow().is_none() {
                *failures.borrow_mut() =
                    Some(Err(anyhow::anyhow!("Window capture server disconnected")));
            }
        })
        .register();
    let _listener = stream
        .add_local_listener_with_user_data(State {
            format: None,
            result: result.clone(),
            stop: stop.clone(),
        })
        .state_changed(|_, data, _, state| {
            if matches!(state, pw::stream::StreamState::Error(_)) && data.result.borrow().is_none()
            {
                *data.result.borrow_mut() =
                    Some(Err(anyhow::anyhow!("Window capture stream failed")));
            }
        })
        .param_changed(|stream, data, id, param| {
            if data.result.borrow().is_some() || id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Some(param) = param else {
                data.format = None;
                return;
            };
            let update = || -> anyhow::Result<_> {
                anyhow::ensure!(
                    spa::param::format_utils::parse_format(param)?
                        == (
                            spa::param::format::MediaType::Video,
                            spa::param::format::MediaSubtype::Raw
                        ),
                    "Invalid window media type"
                );
                let mut format = spa::param::video::VideoInfoRaw::new();
                format.parse(param)?;
                anyhow::ensure!(
                    [
                        spa::param::video::VideoFormat::BGRx,
                        spa::param::video::VideoFormat::BGRA
                    ]
                    .contains(&format.format()),
                    "Unsupported window pixels"
                );
                let size = format.size();
                Layout::checked(
                    size.width,
                    size.height,
                    i32::try_from(
                        size.width
                            .checked_mul(4)
                            .ok_or_else(|| anyhow::anyhow!("Window size overflow"))?,
                    )?,
                    0,
                    u32::try_from(MAX_BYTES)?,
                    u32::try_from(MAX_BYTES)?,
                    false,
                )?;
                let memfd = 1 << spa::buffer::DataType::MemFd.as_raw();
                let flags =
                    spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
                        spa::utils::ChoiceFlags::empty(),
                        spa::utils::ChoiceEnum::Flags {
                            default: memfd,
                            flags: vec![memfd],
                        },
                    )));
                let bytes = pod(spa::pod::Object {
                    type_: spa::utils::SpaTypes::ObjectParamBuffers.as_raw(),
                    id: spa::param::ParamType::Buffers.as_raw(),
                    properties: vec![spa::pod::Property::new(
                        spa::sys::SPA_PARAM_BUFFERS_dataType,
                        flags,
                    )],
                })?;
                stream.update_params(&mut [spa::pod::Pod::from_bytes(&bytes)
                    .ok_or_else(|| anyhow::anyhow!("Invalid buffer parameters"))?])?;
                Ok(format)
            };
            match update() {
                Ok(format) => data.format = Some(format),
                Err(error) => *data.result.borrow_mut() = Some(Err(error)),
            }
        })
        .process(|stream, data| {
            if data.result.borrow().is_some() {
                return;
            }
            let Some(format) = data.format else {
                return;
            };
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let planes = buffer.datas_mut();
            if planes.len() == 1
                && !planes[0].as_raw().chunk.is_null()
                && planes[0].chunk().size() == 0
            {
                return;
            }
            let frame = if planes.len() == 1 {
                pixels(&planes[0], format, &data.stop)
            } else {
                Err(anyhow::anyhow!("Invalid window capture planes"))
            };
            *data.result.borrow_mut() = Some(frame);
        })
        .register()?;
    let bytes = pod(spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            spa::param::video::VideoFormat::BGRA,
            spa::param::video::VideoFormat::BGRA,
            spa::param::video::VideoFormat::BGRx
        ),
    ))?;
    stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT,
        &mut [spa::pod::Pod::from_bytes(&bytes)
            .ok_or_else(|| anyhow::anyhow!("Invalid window format"))?],
    )?;
    let end = Instant::now() + Duration::from_secs(10);
    while result.borrow().is_none() {
        anyhow::ensure!(
            !stop.load(Ordering::Relaxed) && Instant::now() < end,
            "Window capture cancelled or timed out"
        );
        mainloop.loop_().iterate(Duration::from_millis(20));
    }
    let frame = result
        .borrow_mut()
        .take()
        .expect("completed window frame")?;
    anyhow::ensure!(!stop.load(Ordering::Relaxed), "Window screenshot cancelled");
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    #[ignore = "requires the private window screenshot/PipeWire fixture"]
    fn native_window_frame_reader() {
        use std::io::BufRead;
        assert_eq!(
            std::env::var("CANOPA_WINDOW_FRAME_FIXTURE").as_deref(),
            Ok("1")
        );
        for line in std::io::stdin().lock().lines() {
            let command: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
            let serial = command["serial"].as_u64().unwrap();
            let pid = u32::try_from(command["pid"].as_u64().unwrap()).unwrap();
            let result = match read(serial, pid, Arc::new(AtomicBool::new(false))) {
                Ok(frame) => {
                    frame.save(command["output"].as_str().unwrap()).unwrap();
                    serde_json::json!({"width":frame.width(),"height":frame.height()})
                }
                Err(error) => serde_json::json!({"error":error.to_string()}),
            };
            println!("FRAME {result}");
            std::io::stdout().flush().unwrap();
        }
    }
    #[test]
    fn rows_respect_offsets_stride_alpha_and_truncation() {
        let descriptor =
            rustix::fs::memfd_create(c"screenshot-test", rustix::fs::MemfdFlags::CLOEXEC).unwrap();
        let mut file = File::from(descriptor);
        file.write_all(&[99, 99, 3, 2, 1, 4, 0, 0, 6, 5, 4, 7])
            .unwrap();
        let layout = Layout::checked(1, 2, 6, 2, 10, 12, true).unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(
            layout.copy(&file, 0, &stop).unwrap().into_raw(),
            [1, 2, 3, 4, 4, 5, 6, 7]
        );
        file.set_len(10).unwrap();
        assert!(layout.copy(&file, 0, &stop).is_err());
        stop.store(true, Ordering::Relaxed);
        assert!(layout.copy(&file, 0, &stop).is_err());
    }
    #[test]
    fn malformed_geometry_cannot_authorize_a_read_or_allocation() {
        for (w, h, stride, offset, size, max) in [
            (0, 1, 4, 0, 4, 4),
            (1, 0, 4, 0, 4, 4),
            (2, 1, 4, 0, 8, 8),
            (1, 1, -4, 0, 4, 4),
            (1, 2, 4, 0, 4, 4),
            (1, 1, 4, 4, 4, 4),
            (MAX_SIDE, MAX_SIDE, 65536, 0, 256, 256),
        ] {
            assert!(Layout::checked(w, h, stride, offset, size, max, false).is_err());
        }
    }
}

use std::{
    collections::BTreeMap,
    fs::File,
    io,
    os::{
        fd::AsFd,
        unix::{fs::FileExt, net::UnixStream},
    },
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop,
    protocol::{wl_buffer, wl_callback, wl_output, wl_registry, wl_shm, wl_shm_pool},
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1 as frame, zwlr_screencopy_manager_v1 as manager,
};

use super::{LogicalRect, MAX_BYTES, MAX_OUTPUTS, MAX_PIXELS, MAX_SIDE, OutputFrame};

struct Output {
    object: wl_output::WlOutput,
    name: String,
    position: Option<(i32, i32)>,
    logical_size: Option<(i32, i32)>,
    mode: Option<(i32, i32)>,
    transform: Option<wl_output::Transform>,
}

#[derive(Default)]
struct State {
    outputs: BTreeMap<u32, Output>,
    shm: Option<wl_shm::WlShm>,
    xdg: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    manager: Option<manager::ZwlrScreencopyManagerV1>,
    sync: bool,
    capturing: bool,
    error: Option<&'static str>,
    frame: Pending,
    globals: usize,
    used_bytes: usize,
    used_pixels: usize,
}

#[derive(Default)]
struct Pending {
    layout: Option<Layout>,
    buffer: Option<(File, wl_buffer::WlBuffer)>,
    inverted: bool,
    ready: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Layout {
    format: wl_shm::Format,
    width: u32,
    height: u32,
    stride: u32,
}

impl Layout {
    pub(super) fn checked(
        format: WEnum<wl_shm::Format>,
        width: u32,
        height: u32,
        stride: u32,
    ) -> anyhow::Result<Self> {
        let WEnum::Value(
            format @ (wl_shm::Format::Xrgb8888
            | wl_shm::Format::Argb8888
            | wl_shm::Format::Xbgr8888
            | wl_shm::Format::Abgr8888),
        ) = format
        else {
            anyhow::bail!("Screenshot buffer format unavailable");
        };
        anyhow::ensure!(
            width > 0
                && height > 0
                && width <= MAX_SIDE
                && height <= MAX_SIDE
                && stride >= width * 4
                && stride <= width * 4 + 65_536
                && stride as u64 * height as u64 <= MAX_BYTES as u64,
            "Screenshot buffer exceeds limits"
        );
        Ok(Self {
            format,
            width,
            height,
            stride,
        })
    }

    fn bytes(self) -> usize {
        self.stride as usize * self.height as usize
    }

    pub(super) fn pixels(
        self,
        file: &File,
        inverted: bool,
        transform: wl_output::Transform,
        stop: &AtomicBool,
        end: Instant,
    ) -> anyhow::Result<image::RgbaImage> {
        let rotated = matches!(
            transform,
            wl_output::Transform::_90
                | wl_output::Transform::_270
                | wl_output::Transform::Flipped90
                | wl_output::Transform::Flipped270
        );
        let (width, height) = if rotated {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(width as usize * height as usize * 4)?;
        pixels.resize(width as usize * height as usize * 4, 0);
        let mut row = vec![0; self.stride as usize];
        for y in 0..self.height {
            check(stop, end)?;
            file.read_exact_at(&mut row, y as u64 * self.stride as u64)?;
            let upright_y = if inverted { self.height - 1 - y } else { y };
            for x in 0..self.width {
                let word = u32::from_ne_bytes(
                    row[x as usize * 4..x as usize * 4 + 4]
                        .try_into()
                        .expect("four-byte pixel"),
                );
                let (red, blue) = match self.format {
                    wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888 => {
                        ((word >> 16) as u8, word as u8)
                    }
                    _ => (word as u8, (word >> 16) as u8),
                };
                let alpha = match self.format {
                    wl_shm::Format::Argb8888 | wl_shm::Format::Abgr8888 => (word >> 24) as u8,
                    _ => 255,
                };
                let unpremultiply = |channel: u8| {
                    if alpha == 0 {
                        0
                    } else {
                        ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha))
                            .min(255) as u8
                    }
                };
                let (dx, dy) = point(x, upright_y, self.width, self.height, transform);
                let offset = (dy as usize * width as usize + dx as usize) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[
                    unpremultiply(red),
                    unpremultiply((word >> 8) as u8),
                    unpremultiply(blue),
                    alpha,
                ]);
            }
        }
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("Screenshot pixels unavailable"))
    }
}

fn point(x: u32, y: u32, w: u32, h: u32, transform: wl_output::Transform) -> (u32, u32) {
    use wl_output::Transform;
    match transform {
        Transform::Normal => (x, y),
        // wl_output describes the buffer transform; decode with its inverse.
        Transform::_90 => (h - 1 - y, x),
        Transform::_180 => (w - 1 - x, h - 1 - y),
        Transform::_270 => (y, w - 1 - x),
        Transform::Flipped => (w - 1 - x, y),
        Transform::Flipped90 => (y, x),
        Transform::Flipped180 => (x, h - 1 - y),
        Transform::Flipped270 => (h - 1 - y, w - 1 - x),
        _ => unreachable!("known output transform"),
    }
}

fn check(stop: &AtomicBool, end: Instant) -> anyhow::Result<()> {
    anyhow::ensure!(!stop.load(Ordering::Relaxed), "Screenshot cancelled");
    anyhow::ensure!(Instant::now() < end, "Screenshot timed out");
    Ok(())
}

fn pump(
    queue: &mut EventQueue<State>,
    state: &mut State,
    stop: &AtomicBool,
    end: Instant,
) -> anyhow::Result<()> {
    check(stop, end)?;
    queue.dispatch_pending(state)?;
    if let Some(error) = state.error {
        anyhow::bail!(error);
    }
    if state.sync || state.frame.ready {
        return Ok(());
    }
    let Some(guard) = queue.prepare_read() else {
        return Ok(());
    };
    let mut flags = PollFlags::IN;
    if let Err(error) = queue.flush() {
        match error {
            wayland_client::backend::WaylandError::Io(error)
                if error.kind() == io::ErrorKind::WouldBlock =>
            {
                flags |= PollFlags::OUT
            }
            error => return Err(error.into()),
        }
    }
    let descriptor = guard.connection_fd();
    let mut descriptors = [PollFd::new(&descriptor, flags)];
    let remaining = end
        .saturating_duration_since(Instant::now())
        .min(std::time::Duration::from_millis(50));
    let timeout = Timespec {
        tv_sec: remaining.as_secs() as _,
        tv_nsec: remaining.subsec_nanos() as _,
    };
    match poll(&mut descriptors, Some(&timeout)) {
        Ok(_)
            if descriptors[0]
                .revents()
                .intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) =>
        {
            if let Err(error) = guard.read() {
                match error {
                    wayland_client::backend::WaylandError::Io(error)
                        if error.kind() == io::ErrorKind::WouldBlock => {}
                    error => return Err(error.into()),
                }
            }
        }
        Ok(_) => {}
        Err(rustix::io::Errno::INTR) => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn sync(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    stop: &AtomicBool,
    end: Instant,
) -> anyhow::Result<()> {
    state.sync = false;
    connection.display().sync(&queue.handle(), ());
    while !state.sync {
        pump(queue, state, stop, end)?;
    }
    state.sync = false;
    Ok(())
}

pub(super) fn capture(
    stream: UnixStream,
    stop: &AtomicBool,
    end: Instant,
) -> anyhow::Result<Vec<OutputFrame>> {
    let connection = Connection::from_socket(stream)?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut state = State::default();
    connection.display().get_registry(&qh, ());
    sync(&connection, &mut queue, &mut state, stop, end)?;
    anyhow::ensure!(
        state.manager.is_some()
            && state.shm.is_some()
            && state.xdg.is_some()
            && !state.outputs.is_empty(),
        "Screenshot capture protocols unavailable"
    );
    let xdg = state.xdg.as_ref().expect("checked xdg output");
    for (&id, output) in &state.outputs {
        xdg.get_xdg_output(&output.object, &qh, id);
    }
    sync(&connection, &mut queue, &mut state, stop, end)?;
    state.capturing = true;
    let mut frames = Vec::new();
    let ids: Vec<_> = state.outputs.keys().copied().collect();
    for id in ids {
        check(stop, end)?;
        let output = &state.outputs[&id];
        let (x, y) = output
            .position
            .ok_or_else(|| anyhow::anyhow!("Screenshot output geometry unavailable"))?;
        let (w, h) = output
            .logical_size
            .ok_or_else(|| anyhow::anyhow!("Screenshot output geometry unavailable"))?;
        anyhow::ensure!(
            x.unsigned_abs() <= 131_072
                && y.unsigned_abs() <= 131_072
                && w > 0
                && h > 0
                && w <= MAX_SIDE as i32
                && h <= MAX_SIDE as i32,
            "Screenshot output geometry exceeds limits"
        );
        let mode = output
            .mode
            .ok_or_else(|| anyhow::anyhow!("Screenshot output mode unavailable"))?;
        let transform = output
            .transform
            .ok_or_else(|| anyhow::anyhow!("Screenshot output transform unavailable"))?;
        let name = if output.name.is_empty() {
            format!("Display {}", frames.len() + 1)
        } else {
            output.name.clone()
        };
        let frame = state
            .manager
            .as_ref()
            .expect("checked screencopy")
            .capture_output(0, &output.object, &qh, ());
        state.frame = Pending::default();
        while state.frame.layout.is_none() || state.frame.buffer.is_none() {
            pump(&mut queue, &mut state, stop, end)?;
        }
        let layout = state.frame.layout.expect("buffer has layout");
        anyhow::ensure!(
            (layout.width as i32, layout.height as i32) == mode,
            "Screenshot output changed"
        );
        while !state.frame.ready {
            pump(&mut queue, &mut state, stop, end)?;
        }
        let (file, buffer) = state.frame.buffer.take().expect("ready buffer");
        frame.destroy();
        buffer.destroy();
        let image = layout.pixels(&file, state.frame.inverted, transform, stop, end)?;
        frames.push(OutputFrame {
            name,
            logical: LogicalRect {
                x,
                y,
                width: w as u32,
                height: h as u32,
            },
            pixels: image,
        });
        state.frame = Pending::default();
    }
    Ok(frames)
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if state.error.is_some() {
            return;
        }
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                state.globals += 1;
                if state.globals > 512 {
                    state.error = Some("Screenshot registry exceeds limits");
                    return;
                }
                match interface.as_str() {
                    "wl_output" if version >= 2 => {
                        if state.capturing || state.outputs.len() >= MAX_OUTPUTS {
                            state.error =
                                Some("Screenshot output layout changed or exceeds limits");
                            return;
                        }
                        let object = registry.bind(name, version.min(4), qh, name);
                        state.outputs.insert(
                            name,
                            Output {
                                object,
                                name: String::new(),
                                position: None,
                                logical_size: None,
                                mode: None,
                                transform: None,
                            },
                        );
                    }
                    "wl_shm" if state.shm.is_none() => {
                        state.shm = Some(registry.bind(name, 1, qh, ()))
                    }
                    "zxdg_output_manager_v1" if version >= 2 && state.xdg.is_none() => {
                        state.xdg = Some(registry.bind(name, version.min(3), qh, ()))
                    }
                    "zwlr_screencopy_manager_v1" if version >= 3 && state.manager.is_none() => {
                        state.manager = Some(registry.bind(name, 3, qh, ()))
                    }
                    _ => {}
                }
            }
            wl_registry::Event::GlobalRemove { name } if state.outputs.contains_key(&name) => {
                state.error = Some("Screenshot output removed")
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.sync = true;
    }
}

impl Dispatch<wl_output::WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.get_mut(id) else {
            return;
        };
        match event {
            wl_output::Event::Geometry { transform, .. } => {
                if state.capturing {
                    state.error = Some("Screenshot output changed");
                }
                output.transform = match transform {
                    WEnum::Value(value) if u32::from(value) <= 7 => Some(value),
                    _ => None,
                };
            }
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) => {
                if state.capturing {
                    state.error = Some("Screenshot output changed");
                }
                output.mode = Some((width, height));
            }
            _ => {}
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, u32> for State {
    fn event(
        state: &mut Self,
        _: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.get_mut(id) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                if state.capturing {
                    state.error = Some("Screenshot output changed");
                }
                output.position = Some((x, y));
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                if state.capturing {
                    state.error = Some("Screenshot output changed");
                }
                output.logical_size = Some((width, height));
            }
            zxdg_output_v1::Event::Name { name }
                if name.len() <= 256 && !name.chars().any(char::is_control) =>
            {
                output.name = name
            }
            zxdg_output_v1::Event::Name { .. } => {
                state.error = Some("Screenshot output name exceeds limits")
            }
            _ => {}
        }
    }
}

impl Dispatch<frame::ZwlrScreencopyFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        object: &frame::ZwlrScreencopyFrameV1,
        event: frame::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if state.error.is_some() {
            return;
        }
        match event {
            frame::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if state.frame.layout.is_some() {
                    state.error = Some("Screenshot repeated buffer");
                    return;
                }
                match Layout::checked(format, width, height, stride) {
                    Ok(layout)
                        if layout.bytes() <= MAX_BYTES - state.used_bytes
                            && width as usize * height as usize
                                <= MAX_PIXELS - state.used_pixels =>
                    {
                        state.used_bytes += layout.bytes();
                        state.used_pixels += width as usize * height as usize;
                        state.frame.layout = Some(layout);
                    }
                    _ => state.error = Some("Screenshot buffer unsupported or exceeds limits"),
                }
            }
            frame::Event::BufferDone => {
                let Some(layout) = state.frame.layout else {
                    state.error = Some("Screenshot shared memory unavailable");
                    return;
                };
                if state.frame.buffer.is_some() {
                    state.error = Some("Screenshot repeated buffer");
                    return;
                }
                let allocate = || -> anyhow::Result<_> {
                    let file = File::from(rustix::fs::memfd_create(
                        c"canopa-capture",
                        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
                    )?);
                    file.set_len(layout.bytes() as u64)?;
                    rustix::fs::fcntl_add_seals(
                        &file,
                        rustix::fs::SealFlags::GROW
                            | rustix::fs::SealFlags::SHRINK
                            | rustix::fs::SealFlags::SEAL,
                    )?;
                    let pool = state
                        .shm
                        .as_ref()
                        .expect("capture requires shm")
                        .create_pool(file.as_fd(), layout.bytes() as i32, qh, ());
                    let buffer = pool.create_buffer(
                        0,
                        layout.width as i32,
                        layout.height as i32,
                        layout.stride as i32,
                        layout.format,
                        qh,
                        (),
                    );
                    pool.destroy();
                    object.copy(&buffer);
                    Ok((file, buffer))
                };
                match allocate() {
                    Ok(buffer) => state.frame.buffer = Some(buffer),
                    Err(_) => state.error = Some("Screenshot buffer unavailable"),
                }
            }
            frame::Event::Flags {
                flags: WEnum::Value(flags),
            } if flags.bits() & !frame::Flags::YInvert.bits() == 0 => {
                state.frame.inverted = flags.contains(frame::Flags::YInvert)
            }
            frame::Event::Flags { .. } => state.error = Some("Screenshot buffer flags unsupported"),
            frame::Event::Ready { .. } if state.frame.buffer.is_some() => state.frame.ready = true,
            frame::Event::Ready { .. } | frame::Event::Failed => {
                state.error = Some("Screenshot compositor capture failed")
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore manager::ZwlrScreencopyManagerV1);
delegate_noop!(State: ignore zxdg_output_manager_v1::ZxdgOutputManagerV1);

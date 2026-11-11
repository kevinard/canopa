//! Wallpaper selection and preview model. The daemon owns committed assignments.
use crate::{
    background::{self, Fit},
    ipc,
};
use anyhow::{Result, ensure};
use canopa_session_ui::{
    layout::{Anchor, Layout, Rect, Region},
    settings::Settings,
    view::{Control, SessionView},
};
use image::RgbImage;
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub enum Action {
    None,
    Preview,
    Apply(ipc::Request),
    Close,
}
pub struct Controller {
    pub ui: SessionView,
    pub settings: Settings,
    pub images: Vec<PathBuf>,
    pub outputs: Vec<ipc::Output>,
    pub image: usize,
    pub output: usize,
    pub fit: Fit,
    pub preview_color: canopa_theme::Rgb,
    pub focused: usize,
    pub generation: u64,
    pub applying: bool,
    pub ready: bool,
    inputs: Vec<PathBuf>,
    preview: Option<Preview>,
}
impl Controller {
    pub fn new(
        mut settings: Settings,
        mut inputs: Vec<PathBuf>,
        mut outputs: Vec<ipc::Output>,
    ) -> Result<Self> {
        outputs.retain(|o| o.width > 0 && o.height > 0);
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        outputs.truncate(64);
        ensure!(
            !outputs.is_empty(),
            "wallpaper daemon has no configured displays"
        );
        if inputs.is_empty() {
            inputs.extend(outputs.iter().filter_map(|o| o.background.image.clone()));
            if let Some(home) = std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                let pictures = home.join("Pictures");
                let wallpapers = pictures.join("Wallpapers");
                inputs.push(if wallpapers.is_dir() {
                    wallpapers
                } else {
                    pictures
                });
            }
        }
        let images = scan(&inputs)?;
        let preview_color = settings.color;
        settings.color = settings.appearance.palette().background;
        settings.background.image = None;
        settings.outputs.clear();
        settings.widgets = vec!["selection".into()];
        settings.custom.clear();
        settings.layout = Layout {
            margin: 28.,
            gap: 10.,
            regions: vec![
                Region {
                    anchor: Anchor::TopCenter,
                    widgets: vec!["selection".into()],
                    row: false,
                    x: 0.,
                    y: 0.,
                },
                Region {
                    anchor: Anchor::BottomCenter,
                    widgets: vec!["authentication".into()],
                    row: false,
                    x: 0.,
                    y: 0.,
                },
            ],
        };
        let mut ui = SessionView::new(&settings, String::new())?;
        ui.hide_input = true;
        ui.pair_controls = true;
        ui.action_labels = Some(("Apply wallpaper".into(), "Close".into()));
        let fit = outputs[0].background.fit;
        let mut value = Self {
            ui,
            settings,
            images,
            outputs,
            image: 0,
            output: 0,
            fit,
            preview_color,
            focused: 0,
            generation: 0,
            applying: false,
            ready: false,
            inputs,
            preview: None,
        };
        value.refresh();
        Ok(value)
    }
    pub fn refresh(&mut self) {
        let filename = self.images[self.image]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .filter(|c| !c.is_control())
            .take(52)
            .collect::<String>();
        self.ui.custom.insert(
            "selection".into(),
            format!(
                "Wallpaper  ·  {}  ·  {} of {}",
                filename,
                self.image + 1,
                self.images.len()
            ),
        );
        let output = self.output_name().unwrap_or("All monitors").to_owned();
        self.ui.primary_control = (self.ready && !self.applying).then_some(0);
        self.ui.controls = vec![
            Control::new(1, "Previous image   · ←"),
            Control::new(2, "Next image   · →"),
            Control::new(3, format!("Monitor: {output}   · ↑/↓")),
            Control::new(4, format!("Fit: {:?}   · F", self.fit)),
            Control::new(5, "Close   · Escape"),
            Control::new(
                0,
                if self.applying {
                    "Applying…"
                } else {
                    "Apply wallpaper"
                },
            ),
        ];
        for control in &mut self.ui.controls {
            if control.id == self.focused {
                control.selected = true;
            }
        }
        self.ui.interaction_epoch = (self.generation, 0);
    }
    pub fn output_name(&self) -> Option<&str> {
        self.output
            .checked_sub(1)
            .and_then(|i| self.outputs.get(i))
            .map(|o| o.name.as_str())
    }
    pub fn watch_targets(&self) -> Vec<canopa_file::watch::Target> {
        use canopa_file::watch::Target;
        let mut targets = vec![Target::File(self.images[self.image].clone())];
        targets.extend(
            self.inputs
                .iter()
                .filter(|p| p.is_dir())
                .take(32)
                .cloned()
                .map(Target::Directory),
        );
        targets
    }
    pub fn action(&mut self, id: usize) -> Action {
        if id == 5 {
            return Action::Close;
        }
        if self.applying {
            return Action::None;
        }
        match id {
            0 if self.ready => {
                self.applying = true;
                self.ui.message = "Applying the wallpaper…".into();
                self.refresh();
                return Action::Apply(ipc::Request::Set {
                    image: self.images[self.image].clone(),
                    output: self.output_name().map(str::to_owned),
                    fit: self.fit,
                    persist: true,
                });
            }
            1 => self.image = (self.image + self.images.len() - 1) % self.images.len(),
            2 => self.image = (self.image + 1) % self.images.len(),
            3 => self.output = (self.output + 1) % (self.outputs.len() + 1),
            4 => {
                self.fit = match self.fit {
                    Fit::Fill => Fit::Fit,
                    Fit::Fit => Fit::Stretch,
                    Fit::Stretch => Fit::Center,
                    Fit::Center => Fit::Fill,
                }
            }
            _ => return Action::None,
        }
        Action::Preview
    }
    pub fn cycle(&mut self) {
        self.focused = (self.focused + 1) % 6;
        self.refresh();
    }
    pub fn rescan(&mut self) -> Result<()> {
        let current = self.images[self.image].clone();
        self.images = scan(&self.inputs)?;
        self.image = self.images.iter().position(|p| p == &current).unwrap_or(0);
        Ok(())
    }
    pub fn set_outputs(&mut self, mut outputs: Vec<ipc::Output>) {
        let selected = self.output_name().map(str::to_owned);
        outputs.retain(|o| o.width > 0 && o.height > 0);
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        outputs.truncate(64);
        self.outputs = outputs;
        self.output = selected
            .and_then(|name| self.outputs.iter().position(|o| o.name == name))
            .map(|i| i + 1)
            .unwrap_or(0);
    }
    pub fn request_preview(&mut self) -> Spec {
        self.preview = None;
        self.generation = self.generation.wrapping_add(1);
        self.ready = false;
        self.ui.failed = false;
        self.ui.message = "Preparing monitor preview…".into();
        self.refresh();
        let outputs = self
            .output
            .checked_sub(1)
            .and_then(|i| self.outputs.get(i))
            .cloned()
            .map(|o| vec![o])
            .unwrap_or_else(|| self.outputs.iter().take(8).cloned().collect());
        Spec {
            generation: self.generation,
            image: self.images[self.image].clone(),
            outputs,
            fit: self.fit,
            color: self.preview_color,
        }
    }
    pub fn previewed(&mut self, generation: u64, preview: Result<Preview>) {
        if generation != self.generation {
            return;
        }
        self.ready = preview.is_ok();
        self.ui.failed = !self.ready;
        match preview {
            Ok(preview) => {
                self.preview = Some(preview);
                self.ui.message = "Enter applies and saves · Tab selects · F5 refreshes".into();
            }
            Err(_) => {
                self.preview = None;
                self.ui.message = "This image could not be previewed. Choose another image.".into();
            }
        }
        self.refresh();
    }
    pub fn applied(&mut self, result: Result<()>) {
        self.applying = false;
        self.ui.failed = result.is_err();
        self.ui.message = if result.is_ok() {
            "Wallpaper applied and saved"
        } else {
            "Wallpaper change failed; the previous assignment was retained"
        }
        .into();
        self.refresh();
    }
    pub fn render(&self, width: u32, height: u32) -> Result<(RgbImage, Vec<(usize, Rect)>)> {
        let scale = (width as f32 / 800.)
            .min(height as f32 / 800.)
            .clamp(0.2, 4.);
        let color = self.settings.color;
        let (mut image, hits) = self.ui.render_at_scale(
            &self.settings,
            RgbImage::from_pixel(width, height, image::Rgb([color.0, color.1, color.2])),
            scale,
        )?;
        if let Some(preview) = &self.preview {
            let rendered = image::imageops::resize(
                &preview.image,
                (720. * scale) as u32,
                (f32::from(PREVIEW_HEIGHT) * scale) as u32,
                image::imageops::FilterType::Triangle,
            );
            let left = ((width as f32 - 720. * scale) / 2.).max(0.);
            let top = 72. * scale;
            image::imageops::overlay(&mut image, &rendered, left as i64, top as i64);
            for (name, x, y, max) in &preview.labels {
                self.ui.draw_label(
                    &mut image,
                    name,
                    (left + x * scale, top + y * scale),
                    13. * scale,
                    max * scale,
                    self.settings.appearance.palette().text,
                );
            }
        }
        Ok((image, hits))
    }
}
const PREVIEW_HEIGHT: u16 = 500;

pub fn scan(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut budget = 2048usize;
    for path in inputs.iter().take(256) {
        if path.is_file() {
            paths.push(std::path::absolute(path)?);
        } else if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.take(budget) {
                budget = budget.saturating_sub(1);
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                if path.is_file()
                    && path.extension().and_then(|e| e.to_str()).is_some_and(|s| {
                        matches!(s.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png")
                    })
                {
                    paths.push(std::path::absolute(path)?);
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths.truncate(256);
    ensure!(
        !paths.is_empty(),
        "no local wallpaper images found; pass an image file or folder"
    );
    Ok(paths)
}
#[derive(Clone)]
pub struct Spec {
    pub generation: u64,
    pub image: PathBuf,
    pub outputs: Vec<ipc::Output>,
    pub fit: Fit,
    pub color: canopa_theme::Rgb,
}
pub struct Preview {
    image: RgbImage,
    labels: Vec<(String, f32, f32, f32)>,
}
fn prepare(spec: &Spec, cancelled: &impl Fn() -> bool) -> Result<Preview> {
    ensure!(!spec.outputs.is_empty(), "no displays available");
    let source = background::load(&spec.image)?;
    let columns = if spec.outputs.len() == 1 {
        1
    } else if spec.outputs.len() <= 4 {
        2
    } else {
        4
    };
    let rows = spec.outputs.len().div_ceil(columns);
    let cellw = 720 / columns as u32;
    let cellh = u32::from(PREVIEW_HEIGHT) / rows as u32;
    let mut image = RgbImage::from_pixel(
        720,
        u32::from(PREVIEW_HEIGHT),
        image::Rgb([spec.color.0, spec.color.1, spec.color.2]),
    );
    let mut labels = Vec::new();
    for (index, output) in spec.outputs.iter().enumerate() {
        ensure!(!cancelled(), "preview cancelled");
        let width = output.width.saturating_mul(output.scale);
        let height = output.height.saturating_mul(output.scale);
        let rendered =
            background::render_source(Some(&source), spec.fit, spec.color, width, height)?;
        let ratio = (cellw.saturating_sub(12).max(1) as f64 / rendered.width() as f64)
            .min(cellh.saturating_sub(28).max(1) as f64 / rendered.height() as f64);
        let thumb = image::imageops::resize(
            &rendered,
            (rendered.width() as f64 * ratio).round().max(1.) as u32,
            (rendered.height() as f64 * ratio).round().max(1.) as u32,
            image::imageops::FilterType::Triangle,
        );
        let x = index as u32 % columns as u32 * cellw;
        let y = index as u32 / columns as u32 * cellh;
        image::imageops::overlay(
            &mut image,
            &thumb,
            (x + (cellw - thumb.width()) / 2) as i64,
            y as i64,
        );
        labels.push((
            format!("{} · {}×{}", output.name, width, height),
            x as f32 + cellw as f32 / 2.,
            (y + cellh - 8) as f32,
            cellw as f32 - 8.,
        ));
    }
    Ok(Preview { image, labels })
}
struct Shared {
    pending: Mutex<Option<Spec>>,
    wake: Condvar,
    generation: AtomicU64,
    stopped: AtomicBool,
}
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(notify: impl Fn(u64, Result<Preview>) + Send + 'static) -> Self {
        let shared = Arc::new(Shared {
            pending: Mutex::new(None),
            wake: Condvar::new(),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
        let state = shared.clone();
        let thread = std::thread::spawn(move || {
            loop {
                let spec = {
                    let mut pending = state.pending.lock().unwrap();
                    while pending.is_none() && !state.stopped.load(Ordering::Acquire) {
                        pending = state.wake.wait(pending).unwrap();
                    }
                    if state.stopped.load(Ordering::Acquire) {
                        return;
                    }
                    pending.take().unwrap()
                };
                let cancelled = || {
                    state.stopped.load(Ordering::Acquire)
                        || state.generation.load(Ordering::Acquire) != spec.generation
                };
                let result = prepare(&spec, &cancelled);
                if !cancelled() {
                    notify(spec.generation, result);
                }
            }
        });
        Self {
            shared,
            thread: Some(thread),
        }
    }
    pub fn request(&self, spec: Spec) {
        self.shared
            .generation
            .store(spec.generation, Ordering::Release);
        *self.shared.pending.lock().unwrap() = Some(spec);
        self.shared.wake.notify_one();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let pending = self.shared.pending.lock().unwrap();
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.wake.notify_one();
        drop(pending);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn output(name: &str, width: u32, height: u32) -> ipc::Output {
        ipc::Output {
            name: name.into(),
            width,
            height,
            scale: 1,
            background: Default::default(),
        }
    }
    #[test]
    fn monitor_previews_keep_landscape_and_portrait_proportions_and_stale_results_cannot_enable_apply()
     {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.png");
        let second = root.path().join("b.png");
        image::RgbImage::from_pixel(16, 8, image::Rgb([220, 35, 45]))
            .save(&path)
            .unwrap();
        std::fs::copy(&path, &second).unwrap();
        let spec = Spec {
            generation: 1,
            image: path.clone(),
            outputs: vec![output("wide", 128, 80), output("tall", 80, 120)],
            fit: Fit::Fill,
            color: canopa_theme::Rgb(0, 0, 0),
        };
        let preview = prepare(&spec, &|| false).unwrap();
        let bounds = |left: u32, right: u32| {
            let points: Vec<_> = preview
                .image
                .enumerate_pixels()
                .filter(|(x, _, p)| *x >= left && *x < right && p.0 == [220, 35, 45])
                .map(|(x, y, _)| (x, y))
                .collect();
            (
                points.iter().map(|p| p.0).max().unwrap()
                    - points.iter().map(|p| p.0).min().unwrap(),
                points.iter().map(|p| p.1).max().unwrap()
                    - points.iter().map(|p| p.1).min().unwrap(),
            )
        };
        let wide = bounds(0, 360);
        let tall = bounds(360, 720);
        assert!(wide.0 > wide.1 && tall.1 > tall.0);
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Wallpaper,
        )
        .unwrap()
        .0;
        let mut c =
            Controller::new(settings, vec![path, second.clone()], spec.outputs.clone()).unwrap();
        let old = c.request_preview();
        assert!(matches!(c.action(2), Action::Preview));
        let current = c.request_preview();
        c.previewed(old.generation, Ok(preview));
        assert!(!c.ready && matches!(c.action(0), Action::None));
        c.previewed(current.generation, prepare(&current, &|| false));
        assert!(
            matches!(c.action(0),Action::Apply(ipc::Request::Set{image,persist:true,..}) if image==second)
        );
        let worker = Worker::start(|_, _| {});
        let before = std::time::Instant::now();
        drop(worker);
        assert!(before.elapsed() < std::time::Duration::from_millis(500));
    }
}

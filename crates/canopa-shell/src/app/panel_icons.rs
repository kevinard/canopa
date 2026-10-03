//! Prepared menu icons. Views only read pixels; one worker fills the visible
//! range and overscan in small batches, with no queued work for old scroll positions.
use std::collections::{HashMap, VecDeque};

use iced::Task;
use iced::widget::image::Handle;

use super::foreground::PanelMenuRenderState;
use super::messages::PanelEvent;
use super::{App, Message};

mod prepare;

const BATCH_SIZE: usize = 4;
const CACHE_SIZE: usize = 1024;

pub(crate) type Loaded = Vec<(String, Option<Handle>)>;

#[derive(Default)]
pub(super) struct PanelIcons {
    ready: HashMap<String, Option<Handle>>,
    order: VecDeque<String>,
    loading: bool,
}

impl PanelIcons {
    pub(super) fn get(&self, name: &str) -> Option<&Handle> {
        self.ready.get(name).and_then(Option::as_ref)
    }

    fn request(&mut self, menu: &PanelMenuRenderState, row_extent: f32) -> Vec<String> {
        if self.loading || !menu.show_icons {
            return Vec::new();
        }
        let range = super::panel_sizing::menu_render_range(
            menu.entries.len(),
            menu.scroll_y,
            menu.viewport_height,
            row_extent,
        );
        let first_visible = ((menu.scroll_y / row_extent) as usize).clamp(range.start, range.end);
        let mut names = Vec::new();
        // Visible rows first, then upcoming rows, then the buffer behind us.
        for index in (first_visible..range.end).chain(range.start..first_visible) {
            if let Some(name) = menu.entries[index].icon.as_ref()
                && !self.ready.contains_key(name)
                && !names.contains(name)
            {
                names.push(name.clone());
                if names.len() == BATCH_SIZE {
                    break;
                }
            }
        }
        self.loading = !names.is_empty();
        names
    }

    pub(super) fn complete(&mut self, loaded: Loaded) {
        self.loading = false;
        for (name, handle) in loaded {
            if !self.ready.contains_key(&name) {
                if self.ready.len() == CACHE_SIZE
                    && let Some(oldest) = self.order.pop_front()
                {
                    self.ready.remove(&oldest);
                }
                self.order.push_back(name.clone());
            }
            // Remember missing/invalid icons too: scrolling must not retry them.
            self.ready.insert(name, handle);
        }
    }
}

impl App {
    pub(super) fn prepare_panel_icons(&mut self) -> Task<Message> {
        let menu = self.foreground.active_panel_menu_render_state();
        let extent = self.active_menu_metrics().extent;
        let names = self.panel_icons.request(&menu, extent);
        if names.is_empty() {
            return Task::none();
        }
        Task::perform(load(names), |loaded| {
            Message::Panel(PanelEvent::IconsLoaded(loaded))
        })
    }
}

async fn load(names: Vec<String>) -> Loaded {
    let requested = names.clone();
    tokio::task::spawn_blocking(move || {
        names
            .into_iter()
            .map(|name| {
                let handle = crate::icon_theme::resolve_icon_path(&name)
                    .and_then(|path| prepare::from_path(&path));
                (name, handle)
            })
            .collect()
    })
    .await
    .unwrap_or_else(|_| requested.into_iter().map(|name| (name, None)).collect())
}

#[cfg(test)]
mod tests;

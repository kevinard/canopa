use std::collections::{BTreeMap, BTreeSet};

use iced::{Element, Length, Task, widget::image, window::Id};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use super::{App, Message, eligible_outputs::EligibleOutputState};

#[derive(Default)]
pub(super) struct Wallpaper {
    snapshot: crate::wallpaper::Snapshot,
    windows: BTreeMap<String, Id>,
    awaiting_configuration: BTreeSet<Id>,
}

impl Wallpaper {
    pub(super) fn view(&self, window: Id) -> Option<Element<'_, Message>> {
        if !self.windows.values().any(|id| *id == window) {
            return None;
        }
        self.snapshot.background.as_ref().map(|handle| {
            image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover)
                .into()
        })
    }

    pub(super) fn closed(&mut self, window: Id) -> bool {
        self.awaiting_configuration.remove(&window);
        let previous = self.windows.len();
        self.windows.retain(|_, id| *id != window);
        previous != self.windows.len()
    }

    pub(super) fn contains(&self, window: Id) -> bool {
        self.windows.values().any(|id| *id == window)
    }

    fn configured(&mut self, window: Id) -> Option<String> {
        if !self.awaiting_configuration.remove(&window) {
            return None;
        }
        self.windows
            .iter()
            .find_map(|(output, id)| (*id == window).then(|| output.clone()))
    }

    fn reconcile(&mut self, outputs: &EligibleOutputState) -> Vec<Task<Message>> {
        let EligibleOutputState::Known(outputs) = outputs else {
            return Vec::new();
        };
        let wanted = if self.snapshot.background.is_some() {
            outputs.as_slice()
        } else {
            &[]
        };
        let mut tasks = Vec::new();
        self.windows.retain(|output, id| {
            if wanted.contains(output) {
                true
            } else {
                tasks.push(Task::done(Message::RemoveWindow(*id)));
                self.awaiting_configuration.remove(id);
                false
            }
        });
        for output in wanted {
            if self.windows.contains_key(output) {
                continue;
            }
            let (id, task) = Message::layershell_open(settings(output));
            self.windows.insert(output.clone(), id);
            self.awaiting_configuration.insert(id);
            tasks.push(task);
        }
        tasks
    }
}

fn settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        size: Some((0, 0)),
        layer: Layer::Background,
        exclusive_zone: Some(-1),
        keyboard_interactivity: KeyboardInteractivity::None,
        events_transparent: true,
        output_option: OutputOption::OutputName(output.into()),
        namespace: Some("canopa-shell-wallpaper".into()),
        ..Default::default()
    }
}

impl App {
    pub(super) fn handle_wallpaper(&mut self, event: crate::wallpaper::Event) -> Task<Message> {
        self.wallpaper.snapshot = event.snapshot.clone();
        let tasks = self.reconcile_wallpaper();
        event.installed();
        Task::batch(tasks)
    }

    pub(super) fn reconcile_wallpaper(&mut self) -> Vec<Task<Message>> {
        self.wallpaper.reconcile(&self.eligible_outputs)
    }

    pub(super) fn wallpaper_configured(&mut self, window: Id) -> Task<Message> {
        let Some(output) = self.wallpaper.configured(window) else {
            return Task::none();
        };
        // Niri stacks later layer surfaces above earlier ones on the same layer.
        // Wait for the new wallpaper's configure before recreating affected bars.
        // Ordinary image changes and subsequent resizes retain all bar windows.
        let retired: Vec<_> = self
            .surfaces
            .surfaces
            .iter()
            .filter_map(|(id, surface)| {
                (surface.output_name == output
                    && Self::bar_for_surface_name(&self.config.bars, &surface.bar_name).layer
                        == crate::config::BarLayer::Background)
                    .then_some(*id)
            })
            .collect();
        let mut effects = Vec::new();
        for id in retired {
            effects.extend(self.surfaces.retire_bar_surface(id));
        }
        let desired =
            super::bar_surface_plan::plan_bar_surfaces(&self.config.bars, &self.eligible_outputs);
        effects.extend(self.surfaces.reconcile_bar_surfaces(desired));
        Task::batch(self.apply_bar_surface_lifecycle_effects(effects))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected() -> Wallpaper {
        Wallpaper {
            snapshot: crate::wallpaper::Snapshot {
                background: Some(image::Handle::from_rgba(1, 1, vec![1, 2, 3, 255])),
            },
            ..Wallpaper::default()
        }
    }

    #[test]
    fn output_registry_is_independent_and_retains_windows_across_image_changes() {
        let mut wallpaper = selected();
        assert!(
            wallpaper
                .reconcile(&EligibleOutputState::Unknown)
                .is_empty()
        );
        let outputs = EligibleOutputState::Known(vec!["DP-1".into(), "DP-2".into()]);
        assert_eq!(wallpaper.reconcile(&outputs).len(), 2);
        let initial = wallpaper.windows.clone();
        wallpaper.snapshot = selected().snapshot;
        assert!(wallpaper.reconcile(&outputs).is_empty());
        assert_eq!(wallpaper.windows, initial);
        assert!(wallpaper.view(initial["DP-1"]).is_some());
        assert!(wallpaper.view(Id::unique()).is_none());
        assert_eq!(
            wallpaper
                .reconcile(&EligibleOutputState::Known(vec!["DP-2".into()]))
                .len(),
            1
        );
        assert_eq!(wallpaper.windows["DP-2"], initial["DP-2"]);
        wallpaper.snapshot.background = None;
        assert_eq!(wallpaper.reconcile(&outputs).len(), 1);
        assert!(wallpaper.windows.is_empty());
    }

    #[test]
    fn surfaces_cover_the_output_and_cannot_take_input_or_reserve_workarea() {
        let settings = settings("DP-1");
        assert_eq!(settings.layer, Layer::Background);
        assert_eq!(settings.exclusive_zone, Some(-1));
        assert_eq!(settings.keyboard_interactivity, KeyboardInteractivity::None);
        assert!(settings.events_transparent);
        assert_eq!(
            settings.output_option,
            OutputOption::OutputName("DP-1".into())
        );
        assert_eq!(
            settings.anchor,
            Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
        );
    }

    #[test]
    fn first_wallpaper_configuration_reopens_only_background_bars_on_that_output() {
        let mut config = crate::config::Config::default();
        config.bars[0].layer = crate::config::BarLayer::Background;
        let (mut app, _) = App::boot(config, vec!["DP-1".into(), "DP-2".into()]);
        let before: BTreeSet<_> = app.surfaces.surfaces.keys().copied().collect();
        app.wallpaper = selected();
        let _ = app.reconcile_wallpaper();
        let window = app.wallpaper.windows["DP-1"];
        let _ = app.wallpaper_configured(window);
        let after: BTreeSet<_> = app.surfaces.surfaces.keys().copied().collect();
        assert_eq!(before.intersection(&after).count(), 1);
        assert_eq!(after.len(), 2);
        let _ = app.wallpaper_configured(window);
        assert_eq!(
            app.surfaces
                .surfaces
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            after
        );
    }
}

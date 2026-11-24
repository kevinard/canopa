//! Request-owned picker decoration. Window previews never flow into a portal reply.
use super::Message;
use crate::{icons, worker};
use canopa_services::window_preview::Client;
use iced::{Task, widget::image::Handle};
use std::{collections::HashMap, time::Duration};

pub(super) const THUMBNAIL_WIDTH: u32 = 112;
pub(super) const THUMBNAIL_HEIGHT: u32 = 62;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Target {
    pub id: u64,
    pub app_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct Visual {
    pub thumbnail: Option<Handle>,
    pub icon: Option<Handle>,
}

#[derive(Default)]
pub(super) struct Previews {
    wanted: Vec<Target>,
    running: bool,
    visuals: HashMap<Target, Visual>,
}

impl Previews {
    pub fn get(&self, target: &Target) -> Option<&Visual> {
        self.visuals.get(target)
    }

    pub fn set_targets(
        &mut self,
        wanted: Vec<Target>,
        pid: u32,
        foreground: canopa_theme::Rgb,
    ) -> Task<Message> {
        if self.wanted != wanted {
            self.visuals.retain(|target, _| wanted.contains(target));
            self.wanted = wanted;
        }
        if self.running {
            return Task::none();
        }
        let missing: Vec<_> = self
            .wanted
            .iter()
            .filter(|target| !self.visuals.contains_key(*target))
            .cloned()
            .collect();
        if missing.is_empty() {
            return Task::none();
        }
        self.running = true;
        let fallback = missing.clone();
        Task::perform(
            async move {
                worker::run(move || {
                    let mut visuals = prepare_icons(&missing, foreground);
                    if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        runtime.block_on(async {
                            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                                let Ok(connection) = zbus::Connection::session().await else {
                                    return;
                                };
                                let Some(client) = Client::connect(connection, Some(pid)).await
                                else {
                                    return;
                                };
                                for (target, visual) in &mut visuals {
                                    if let Some(preview) = client.capture(target.id, 224).await {
                                        visual.thumbnail = Some(Handle::from_bytes(preview.png));
                                    }
                                }
                            })
                            .await;
                        });
                    }
                    visuals
                })
                .await
                .unwrap_or_else(|_| {
                    fallback
                        .into_iter()
                        .map(|target| {
                            (
                                target,
                                Visual {
                                    icon: None,
                                    thumbnail: None,
                                },
                            )
                        })
                        .collect()
                })
            },
            Message::PreviewsReady,
        )
    }

    pub fn complete(&mut self, visuals: Vec<(Target, Visual)>) {
        self.running = false;
        self.visuals.extend(
            visuals
                .into_iter()
                .filter(|(target, _)| self.wanted.contains(target)),
        );
    }
}

fn prepare_icons(targets: &[Target], foreground: canopa_theme::Rgb) -> Vec<(Target, Visual)> {
    let catalog = canopa_apps::discover_choice_metadata().unwrap_or_default();
    let icons_for_targets: Vec<_> = targets
        .iter()
        .map(|target| {
            let id = target.app_id.as_deref()?;
            // Only trusted desktop entries may supply an absolute icon path.
            canopa_apps::match_desktop_application_for_app_id(id, &catalog)
                .and_then(|app| app.icon.clone())
                .or_else(|| {
                    (!id.starts_with('.') && !id.contains(['/', '\\', '\0'])).then(|| id.to_owned())
                })
        })
        .collect();
    let names: Vec<_> = icons_for_targets.iter().flatten().cloned().collect();
    let icons = icons::prepare(&names, foreground);
    targets
        .iter()
        .cloned()
        .zip(icons_for_targets)
        .map(|(target, name)| {
            let icon = name.and_then(|name| icons.get(&name).cloned());
            (
                target,
                Visual {
                    icon,
                    thumbnail: None,
                },
            )
        })
        .collect()
}

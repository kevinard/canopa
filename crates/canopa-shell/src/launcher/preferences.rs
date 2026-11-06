//! Poll shared preferences and publish discovery off the UI thread, without queries.
use super::Config;
#[cfg(not(test))]
use canopa_launcher::Catalog;
use canopa_launcher::Preferences;
use iced::Subscription;
#[cfg(not(test))]
use iced::futures::SinkExt;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct Event(pub Result<Preferences, String>);
pub struct Runtime {
    pub value: Arc<Preferences>,
    pub error: Option<String>,
    pub diagnostics: BTreeMap<String, (String, Vec<String>)>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            value: Arc::new(Preferences::default()),
            error: None,
            diagnostics: BTreeMap::new(),
        }
    }
}
impl Runtime {
    pub fn apply(&mut self, event: Event) {
        match event.0 {
            Ok(value) => {
                if *self.value != value {
                    self.value = Arc::new(value);
                }
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
    #[cfg(test)]
    pub fn subscription(&self, _configs: Vec<Config>) -> Subscription<Event> {
        Subscription::none()
    }
    #[cfg(not(test))]
    pub fn subscription(&self, configs: Vec<Config>) -> Subscription<Event> {
        if configs.is_empty() {
            return Subscription::none();
        }
        Subscription::run_with(
            Key {
                configs,
                diagnostics: self.diagnostics.clone(),
            },
            stream,
        )
    }
}
#[cfg(not(test))]
#[derive(Clone, Hash)]
struct Key {
    configs: Vec<Config>,
    diagnostics: BTreeMap<String, (String, Vec<String>)>,
}
#[cfg(not(test))]
fn stream(key: &Key) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Event> + Send>> {
    let key = key.clone();
    Box::pin(iced::stream::channel(
        1,
        move |mut sender: iced::futures::channel::mpsc::Sender<Event>| async move {
            let mut previous = None;
            let mut previous_catalog: Option<Catalog> = None;
            loop {
                let configs = key.configs.clone();
                let diagnostics = key.diagnostics.clone();
                let old_catalog = previous_catalog.clone();
                let (result, catalog) = tokio::task::spawn_blocking(move || {
                    let result = canopa_launcher::preferences_path()
                        .and_then(|path| Preferences::read(&path))
                        .map_err(|e| e.to_string());
                    let catalog = result
                        .as_ref()
                        .ok()
                        .map(|prefs| super::bundled::catalog(&configs, prefs, &diagnostics));
                    if let Some(catalog) = &catalog
                        && old_catalog.as_ref() != Some(catalog)
                        && let Ok(path) = canopa_launcher::catalog_path()
                    {
                        let _ = catalog.write(&path);
                    }
                    (result, catalog)
                })
                .await
                .unwrap_or_else(|_| (Err("Launcher preferences worker stopped".into()), None));
                previous_catalog = catalog;
                if previous.as_ref() != Some(&result) {
                    previous = Some(result.clone());
                    if sender.send(Event(result)).await.is_err() {
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
    ))
}

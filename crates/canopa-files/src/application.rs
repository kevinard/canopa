//! Process-owned workflow composition. No renderer or browser is initialized here.
use crate::{
    configuration::{ConfigurationJobs, ConfigurationSnapshot},
    desktop_notifications::Worker,
    local_actions::LocalActions,
    local_volumes::LocalOperations,
};
use std::{
    io,
    path::{Path, PathBuf},
};

pub struct Application {
    activity: crate::local_activity::LocalActivity,
    initial: ConfigurationSnapshot,
    #[cfg(feature = "media-playback")]
    pub playback: crate::playback::Playback,
    pub configuration: Result<ConfigurationJobs, String>,
    pub operations: Result<LocalOperations, String>,
    pub actions: Result<LocalActions, String>,
    _notifications: Result<Worker, String>,
    file_manager: Option<crate::file_manager::Host>,
}

impl Application {
    /// The caller must hold exclusive runtime ownership before starting this host.
    pub fn start(directory: &Path, request: &canopa_config::Request) -> io::Result<Self> {
        let initial = ConfigurationSnapshot::load(request).map_err(io::Error::other)?;
        let configuration = ConfigurationJobs::start(initial.clone()).map_err(|e| e.to_string());
        let notifications = Worker::new().map_err(|e| e.to_string());
        let activity = crate::local_activity::LocalActivity::default();
        let operations = LocalOperations::start_with_activity(
            directory.to_owned(),
            notifications.clone(),
            activity.clone(),
        )
        .map_err(|e| e.to_string());
        let actions = match &operations {
            Ok(operations) => LocalActions::start_with_storage(
                notifications.clone(),
                activity.clone(),
                operations.jobs().storage_source(),
            )
            .map_err(|e| e.to_string()),
            Err(error) => Err(format!(
                "Custom-action storage protection is unavailable: {error}"
            )),
        };
        Ok(Self {
            activity,
            initial,
            #[cfg(feature = "media-playback")]
            playback: crate::playback::Playback::default(),
            configuration,
            operations,
            actions,
            _notifications: notifications,
            file_manager: None,
        })
    }

    pub fn configuration_snapshot(&self) -> ConfigurationSnapshot {
        self.configuration.as_ref().map_or_else(
            |_| self.initial.clone(),
            |jobs| (*jobs.snapshot().current).clone(),
        )
    }

    /// Attach standard desktop presentation to this already owned runtime.
    /// Another file manager's existing bus name is left untouched.
    pub fn serve_file_manager(
        &mut self,
        instance: crate::activation::Instance,
    ) -> io::Result<bool> {
        if self.file_manager.is_some() {
            return Ok(true);
        }
        self.file_manager = crate::file_manager::Host::start(instance)?;
        Ok(self.file_manager.is_some())
    }

    pub fn activity(&self) -> &crate::local_activity::LocalActivity {
        &self.activity
    }

    pub fn holds_application(&self) -> bool {
        if self.activity.holds_application() {
            return true;
        }
        #[cfg(feature = "media-playback")]
        if self.playback.holds_application() {
            return true;
        }
        self.configuration
            .as_ref()
            .is_ok_and(|jobs| jobs.snapshot().pending > 0)
            || self
                .operations
                .as_ref()
                .is_ok_and(|operations| operations.jobs().snapshot().holds_application())
            || self
                .actions
                .as_ref()
                .is_ok_and(|actions| actions.snapshot().holds_application())
    }

    /// Wait for actual queue teardown before releasing the process's instance lock.
    /// A refused shutdown leaves ownership with the caller.
    pub async fn shutdown(&self) -> io::Result<()> {
        if self.holds_application() {
            return Err(io::Error::other(
                "Files still has accepted work or unsaved results.",
            ));
        }
        if let Some(host) = &self.file_manager {
            host.shutdown()?;
        }
        #[cfg(feature = "media-playback")]
        self.playback.shutdown()?;
        if let Ok(operations) = &self.operations {
            operations.shutdown().await?;
        }
        if let Ok(actions) = &self.actions {
            actions
                .shutdown()?
                .await
                .map_err(io::Error::other)?
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
}

pub fn state_directory(home: &Path) -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/state"))
        .join("canopa-files")
}

pub fn data_home(home: &Path) -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))
}

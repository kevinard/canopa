use canopa_files::{
    actions::CustomAction,
    configuration::{ConfigurationJobs, ConfigurationSnapshot},
};
use std::{
    fs,
    time::{Duration, Instant},
};

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            Instant::now() < until,
            "configuration runtime did not settle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_owner_keeps_configuration_work_alive_after_frontend_and_reply_detach() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(&path, "schema-version 1\nnotifications { }\n").unwrap();
    let base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let owner = ConfigurationJobs::start(base.clone()).unwrap();
    let frontend = owner.clone();
    let notices = frontend.take_notices().unwrap();
    let action = CustomAction::new("inspect", "Inspect", "/bin/true %F");
    let reply = frontend.save(base.clone(), vec![action.clone()]).unwrap();
    drop(reply);
    drop(notices);
    drop(frontend);
    let saved = wait_for(|| {
        let snapshot = owner.snapshot();
        (snapshot.pending == 0
            && snapshot
                .latest
                .as_ref()
                .is_some_and(|publication| publication.error.is_none()))
        .then_some(snapshot.current)
    });
    assert_eq!(saved.actions, [action]);
    assert!(fs::read_to_string(&path).unwrap().contains("notifications"));
    assert_eq!(
        ConfigurationSnapshot::load(&canopa_config::Request::common(&path))
            .unwrap()
            .actions,
        saved.actions
    );

    // A detached caller's rejected write is still observable without stealing
    // a one-shot reply or consuming another frontend's latest publication.
    drop(owner.save(base, vec![]).unwrap());
    let rejected = wait_for(|| {
        let snapshot = owner.snapshot();
        if snapshot.pending == 0
            && let Some(error) = snapshot
                .latest
                .as_ref()
                .and_then(|publication| publication.error.clone())
        {
            Some((error, snapshot.current))
        } else {
            None
        }
    });
    assert!(
        rejected.0.contains("configuration changed"),
        "{}",
        rejected.0
    );
    assert_eq!(rejected.1.actions, saved.actions);
    assert_eq!(owner.snapshot().current.actions, saved.actions);

    fs::write(
        &path,
        "schema-version 1\nfiles { view column-width=390; }\nnotifications { }\n",
    )
    .unwrap();
    let changed = wait_for(|| {
        let snapshot = owner.snapshot();
        (snapshot.current.settings.view.column_width == 390).then_some(snapshot)
    });
    assert!(changed.latest.unwrap().error.is_none());
    assert!(changed.current.actions.is_empty());
    assert_eq!(changed.pending, 0);
}

fn await_reply<T>(
    mut receiver: futures_channel::oneshot::Receiver<T>,
) -> Result<T, futures_channel::oneshot::Canceled> {
    wait_for(|| receiver.try_recv().transpose())
}

#[test]
fn a_coalesced_rejection_carries_the_latest_valid_configuration_and_its_version() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    fs::write(&path, "schema-version 1\n").unwrap();
    let base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let owner = ConfigurationJobs::start(base.clone()).unwrap();
    let mut settings = base.settings.clone();
    settings.view.column_width = 390;
    let accepted = await_reply(owner.save_settings(base.clone(), settings).unwrap())
        .unwrap()
        .unwrap();
    assert!(
        await_reply(owner.save(base, vec![]).unwrap())
            .unwrap()
            .is_err()
    );
    // The frontend receives one coalesced publication, after both the successful
    // update and rejection. It must be able to apply the valid candidate and
    // explain the rejection from that same publication.
    let latest = owner.snapshot().latest.unwrap();
    assert_eq!(latest.current.settings.view.column_width, 390);
    assert!(
        latest
            .error
            .as_deref()
            .unwrap()
            .contains("configuration changed")
    );
    assert_eq!(latest.current_revision, accepted.revision());
    assert!(latest.revision() > latest.current_revision);
    let loaded = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    assert_eq!(loaded.settings.view.column_width, 390);
}

#[test]
fn settings_and_actions_share_one_guarded_source_and_preserve_each_other() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    std::fs::write(&path, "schema-version 1\nnotifications { }\n").unwrap();
    let base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let worker = ConfigurationJobs::start(base.clone()).unwrap();
    let action = CustomAction::new("inspect", "Inspect", "/bin/true %F");
    let reply = worker.save(base, vec![action.clone()]).unwrap();
    let base = await_reply(reply).unwrap().unwrap().current;
    let mut settings = base.settings.clone();
    settings.toolbar.push("action:inspect".into());
    settings.view.column_width = 420;
    let reply = worker.save_settings((*base).clone(), settings).unwrap();
    let saved = await_reply(reply).unwrap().unwrap().current;
    assert_eq!(saved.actions[0].command, action.command);
    assert!(saved.actions[0].toolbar);
    assert_eq!(saved.settings.view.column_width, 420);
    let stale = worker.save((*base).clone(), vec![]).unwrap();
    assert!(
        await_reply(stale)
            .unwrap()
            .unwrap_err()
            .contains("configuration changed")
    );
    let mut action = saved.actions[0].clone();
    action.name = "Updated".into();
    let reply = worker.save((*saved).clone(), vec![action]).unwrap();
    let saved = await_reply(reply).unwrap().unwrap().current;
    assert_eq!(saved.settings.view.column_width, 420);
    assert_eq!(saved.settings.toolbar.last().unwrap(), "action:inspect");
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("notifications")
    );
    let defaults = ConfigurationSnapshot::load(&canopa_config::Request::Defaults).unwrap();
    assert!(!defaults.is_editable());
    let defaults_owner = ConfigurationJobs::start(defaults.clone()).unwrap();
    let reply = defaults_owner
        .save_settings(defaults.clone(), defaults.settings.clone())
        .unwrap();
    assert!(
        await_reply(reply)
            .unwrap()
            .unwrap_err()
            .contains("cannot be edited")
    );
}

#[test]
fn worker_serializes_saves_and_rejects_stale_drafts_without_replacing_newer_configuration() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    std::fs::write(&path, "schema-version 1\nnotifications { }\n").unwrap();
    let base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let worker = ConfigurationJobs::start(base.clone()).unwrap();
    let action = CustomAction::new("inspect", "Inspect", "/bin/true %F");
    let reply = worker.save(base.clone(), vec![action.clone()]).unwrap();
    let publication = await_reply(reply).unwrap().unwrap();
    let first = publication.revision();
    let saved = publication.current;
    assert_eq!(saved.actions, [action]);
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("notifications")
    );
    let stale = worker.save(base, vec![]).unwrap();
    assert!(
        await_reply(stale)
            .unwrap()
            .unwrap_err()
            .contains("configuration changed")
    );
    let snapshot = worker.snapshot();
    let publication = snapshot.latest.unwrap();
    let version = publication.revision();
    let error = publication.error.expect("missing stale-write rejection");
    assert!(version > first);
    assert!(error.contains("configuration changed"));
    assert_eq!(snapshot.current.actions, saved.actions);
    assert_eq!(worker.snapshot().latest.unwrap().revision(), version);
    let reload = worker.reload((*saved).clone()).unwrap();
    let publication = await_reply(reload).unwrap().unwrap();
    let next = publication.revision();
    let reloaded = publication.current;
    assert!(next > version);
    assert_eq!(reloaded.actions, saved.actions);
}

#[test]
fn undrained_configuration_notices_do_not_block_save_acknowledgements() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    std::fs::write(&path, "schema-version 1\n").unwrap();
    let mut base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let worker = ConfigurationJobs::start(base.clone()).unwrap();
    let mut last_revision = 0;
    for index in 0..10 {
        let action = CustomAction::new("inspect", format!("Inspect {index}"), "/bin/true %F");
        let reply = worker.save(base, vec![action.clone()]).unwrap();
        let publication = await_reply(reply).unwrap().unwrap();
        let revision = publication.revision();
        let saved = publication.current;
        assert!(revision > last_revision);
        assert_eq!(saved.actions, [action]);
        last_revision = revision;
        base = (*saved).clone();
    }
    // Never subscribe or read the notification channel: it stays full,
    // while each independent command still acknowledges its exact save.
    let publication = worker
        .snapshot()
        .latest
        .expect("missing latest configuration");
    let revision = publication.revision();
    let latest = publication.current;
    assert_eq!(revision, last_revision);
    assert_eq!(latest.actions[0].name, "Inspect 9");
    assert_eq!(worker.snapshot().latest.unwrap().revision(), last_revision);
    let mut notices = worker.take_notices().unwrap();
    assert!(worker.take_notices().is_none());
    let mut count = 0;
    while notices.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 1, "undrained publications coalesce into one wakeup");
}

#[test]
fn requests_cannot_redirect_the_owning_configuration_selection() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned.kdl");
    let other_path = root.path().join("other.kdl");
    let original = "schema-version 1\nfiles { view column-width=260; }\nnotifications { }\n";
    let foreign_bytes = "schema-version 1\nfiles { view column-width=490; }\nnotifications { }\n";
    fs::write(&path, original).unwrap();
    fs::write(&other_path, foreign_bytes).unwrap();
    let base = ConfigurationSnapshot::load(&canopa_config::Request::common(&path)).unwrap();
    let other = ConfigurationSnapshot::load(&canopa_config::Request::common(&other_path)).unwrap();
    let owner = ConfigurationJobs::start(base.clone()).unwrap();
    let result = await_reply(owner.reload(other.clone()).unwrap()).unwrap();
    assert!(
        result.is_err(),
        "Reload redirected the owner's original source selection"
    );
    assert!(result.unwrap_err().contains("another source selection"));
    let mut settings = other.settings.clone();
    settings.view.column_width = 510;
    let result = await_reply(owner.save_settings(other.clone(), settings).unwrap()).unwrap();
    assert!(result.unwrap_err().contains("another source selection"));
    let result = await_reply(
        owner
            .save(
                other,
                vec![CustomAction::new("foreign", "Foreign", "/bin/true %F")],
            )
            .unwrap(),
    )
    .unwrap();
    assert!(result.unwrap_err().contains("another source selection"));
    let defaults = ConfigurationSnapshot::load(&canopa_config::Request::Defaults).unwrap();
    let result = await_reply(owner.reload(defaults).unwrap()).unwrap();
    assert!(result.unwrap_err().contains("another source selection"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert_eq!(fs::read_to_string(&other_path).unwrap(), foreign_bytes);
    assert_eq!(owner.snapshot().current.settings.view.column_width, 260);
    // Rejections do not switch polling to another source or defaults. A real
    // edit to the original path still advances the same owner without a UI.
    fs::write(
        &path,
        "schema-version 1\nfiles { view column-width=410; }\nnotifications { }\n",
    )
    .unwrap();
    let current = wait_for(|| {
        let snapshot = owner.snapshot();
        (snapshot.current.settings.view.column_width == 410).then_some(snapshot.current)
    });
    let mut settings = current.settings.clone();
    settings.view.column_width = 450;
    let saved = await_reply(owner.save_settings((*current).clone(), settings).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(saved.current.settings.view.column_width, 450);
    assert_eq!(
        ConfigurationSnapshot::load(&canopa_config::Request::common(&path))
            .unwrap()
            .settings
            .view
            .column_width,
        450
    );
    assert_eq!(fs::read_to_string(&other_path).unwrap(), foreign_bytes);
}

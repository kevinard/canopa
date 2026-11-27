use canopa_session_ui::view::{AuthenticationPanel, Control, Emblem};
pub use canopa_session_ui::view::{Prompt, SessionView as LockUi};

pub const UNLOCK: usize = usize::MAX - 4;
pub(crate) const CHANGE_PASSWORD: usize = usize::MAX - 2;

pub fn configure(ui: &mut LockUi) {
    ui.authentication_panel = Some(AuthenticationPanel {
        // The login screen's, so the clock and identity sit at the same place.
        height_reserve: 420.,
        heading: fl!("lock-heading"),
        subtitle: fl!("lock-subtitle"),
        emblem: Emblem::Lock,
        dialog: false,
        choices: &[],
        utilities: &[],
        disclosure: None,
        list_actions: false,
        checked_control: None,
        choice_details: Default::default(),
        account: None,
        confirm_identity: false,
        facts: Vec::new(),
    });
}

pub fn update_actions(ui: &mut LockUi, can_retry: bool) {
    ui.controls.retain(|control| control.id != UNLOCK);
    ui.primary_control = if ui.hide_input {
        Some(CHANGE_PASSWORD)
    } else if ui.prompt.is_some() || can_retry {
        ui.controls.push(Control::new(
            UNLOCK,
            if ui.prompt.is_some() {
                fl!("lock-unlock")
            } else {
                fl!("lock-try-again")
            },
        ));
        Some(UNLOCK)
    } else {
        None
    };
}

/// Show who is locked: their picture when they have one, else their initials.
/// The picture is the configured `avatar`, the account's AccountsService icon or
/// `~/.face`; each is read through the bounded image loader.
pub fn identify(ui: &mut LockUi, login: &str) {
    ui.set_avatar_initials(Some(canopa_session_ui::view::initials(&ui.username)).filter(|s| !s.is_empty()));
    if ui.has_configured_avatar() {
        return;
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let candidates = [
        (!login.is_empty() && !login.contains('/'))
            .then(|| std::path::Path::new("/var/lib/AccountsService/icons").join(login)),
        home.map(|home| home.join(".face")),
    ];
    if let Some(image) = candidates
        .into_iter()
        .flatten()
        .filter(|path| path.is_file())
        .find_map(|path| canopa_session_ui::background::load(&path).ok())
    {
        ui.set_user_avatar(Some(std::sync::Arc::new(image::imageops::thumbnail(
            &image, 256, 256,
        ))));
    }
}

use canopa_session_ui::view::{AuthenticationPanel, Control, Emblem};
pub use canopa_session_ui::view::{Prompt, SessionView as LockUi};

pub const UNLOCK: usize = usize::MAX - 4;
pub(crate) const CHANGE_PASSWORD: usize = usize::MAX - 2;

pub fn configure(ui: &mut LockUi) {
    ui.authentication_panel = Some(AuthenticationPanel {
        height_reserve: 408.,
        heading: fl!("lock-heading"),
        eyebrow: fl!("lock-eyebrow"),
        emblem: Emblem::Lock,
        dialog: false,
        choices: &[],
        utilities: &[],
        disclosure: None,
        list_actions: false,
        checked_control: None,
        choice_details: Default::default(),
        account: None,
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

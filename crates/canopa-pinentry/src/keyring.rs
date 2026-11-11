//! One dialog for the GCR bridge. The response travels over its anonymous pipe.
use anyhow::{Result, ensure};
use canopa_pinentry::{
    protocol::{Kind, Request, Response},
    wayland,
};
use canopa_session_ui::settings::Settings;
use std::{
    fs::File,
    io::Write,
    os::{fd::AsFd, unix::fs::FileTypeExt},
};

#[derive(clap::Args)]
pub struct Dialog {
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: String,
    #[arg(long)]
    warning: String,
    #[arg(long)]
    accept: String,
    #[arg(long)]
    cancel: String,
    #[arg(long, conflicts_with = "password_new")]
    confirm: bool,
    #[arg(long)]
    password_new: bool,
}

pub fn run(settings: &Settings, dialog: Dialog) -> Result<i32> {
    let input = File::from(std::io::stdin().as_fd().try_clone_to_owned()?);
    let mut output = File::from(std::io::stdout().as_fd().try_clone_to_owned()?);
    ensure!(
        input.metadata()?.file_type().is_fifo() && output.metadata()?.file_type().is_fifo(),
        "keyring dialog requires private pipes"
    );
    let defaults = Request::default();
    let request = Request {
        kind: if dialog.confirm {
            Kind::Confirm
        } else {
            Kind::Pin
        },
        title: dialog.title,
        description: dialog.description,
        error: dialog.warning,
        ok: if dialog.accept.is_empty() {
            defaults.ok
        } else {
            dialog.accept
        },
        cancel: if dialog.cancel.is_empty() {
            defaults.cancel
        } else {
            dialog.cancel
        },
        repeat: dialog.password_new.then(|| fl!("pinentry-repeat-password")),
        timeout: settings.auth_timeout,
        ..defaults
    };
    match wayland::run_keyring(settings, &request)? {
        Response::Pin { secret, .. } => {
            // Write directly from bounded secret storage without a stdio buffer.
            output.write_all(secret.as_bytes())?;
            Ok(0)
        }
        Response::Accepted => Ok(0),
        Response::Canceled | Response::NotConfirmed | Response::Timeout => Ok(2),
        Response::Unavailable => Ok(1),
    }
}

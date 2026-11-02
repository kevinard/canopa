use anyhow::{Result, ensure};
use canopa_config::{Component, Document, Request};
use canopa_session_ui::{
    layout::{Anchor, Layout, Region},
    settings::Settings,
};

pub fn load(request: &Request) -> Result<(Settings, Document)> {
    let document = Document::load(request, Component::Polkit, "")?;
    ensure!(
        document
            .selected()
            .nodes()
            .iter()
            .all(|n| ["font", "auth-timeout"].contains(&n.name().value())),
        "unknown polkit setting"
    );
    let mut settings = Settings::parse(
        document.selected(),
        document.appearance(),
        document.info().path.as_deref(),
        Component::Polkit,
    )?;
    settings.use_wallpaper = false;
    settings.widgets = vec![
        "avatar".into(),
        "message-0".into(),
        "message-1".into(),
        "action".into(),
    ];
    settings.layout = Layout {
        regions: vec![Region {
            anchor: Anchor::Center,
            widgets: vec![
                "avatar".into(),
                "message-0".into(),
                "message-1".into(),
                "action".into(),
                "authentication".into(),
            ],
            row: false,
            x: 0.,
            y: 0.,
        }],
        margin: 24.,
        gap: 12.,
    };
    Ok((settings, document))
}

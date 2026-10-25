use canopa_theme::{Appearance, ColorRole, Rgb};

pub(super) fn install(appearance: &Appearance) -> gtk::CssProvider {
    let provider = gtk::CssProvider::new();
    update(&provider, appearance);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    provider
}

pub(super) fn update(provider: &gtk::CssProvider, appearance: &Appearance) {
    // The native chooser is outside our scoped widget CSS. This process-local
    // GTK 4.12 preference gives its fallback dialog the same light/dark mode.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(
            appearance.color_scheme == canopa_theme::ColorScheme::Dark,
        );
    }
    let p = appearance.palette();
    let rgb = |Rgb(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}");
    let family = appearance
        .typography()
        .family
        .iter()
        .map(|name| {
            format!(
                "\"{}\"",
                name.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace(['\n', '\r'], " ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let css = format!(
        r#"
        window.canopa-files {{ background: {background}; color: {text}; font-family: {family}; font-size: {body}px; }}
        .canopa-files headerbar {{ background: {raised}; color: {text}; box-shadow: none; border-bottom: 1px solid {border}; min-height: 36px; }}
        .canopa-files .toolbar {{ padding: 8px; background: {surface}; border-bottom: 1px solid {border}; }}
        .canopa-files button {{ background: transparent; background-image: none; box-shadow: none; border: 1px solid transparent; border-radius: 8px; min-height: 24px; min-width: 24px; color: {text}; }}
        .canopa-files dropdown button {{ background: {raised}; border-color: {border}; }}
        .canopa-files popover contents {{ background: {raised}; color: {text}; border-color: {border}; }}
        .canopa-files button:disabled {{ color: {muted}; }}
        .canopa-files button:hover {{ background: {raised}; }}
        .canopa-files .operation-actions > flowboxchild {{ padding: 0; }}
        .canopa-files entry {{ background: {surface}; color: {text}; border-color: {border}; min-width: 0; }}
        .canopa-files .sidebar {{ background: {spine}; padding: 12px 6px; border-right: 1px solid {border}; }}
        .canopa-files .sidebar button {{ padding: 8px; }}
        .canopa-files .file-column {{ background: {surface}; border-right: 1px solid {border}; }}
        .canopa-files .column-heading, .canopa-files .status {{ color: {muted}; font-size: {label}px; padding: 8px 12px; }}
        .canopa-files listview, .canopa-files gridview {{ background: {surface}; color: {text}; }}
        .canopa-files listview row {{ padding: 4px 8px; border-radius: 8px; margin: 1px 4px; }}
        .canopa-files gridview child {{ padding: 10px; border-radius: 10px; }}
        .canopa-files listview row:selected, .canopa-files gridview child:selected {{ background: {selection}; color: {text}; }}
        .canopa-files .file-details {{ color: {muted}; font-size: {label}px; }}
        .canopa-files .status {{ border-top: 1px solid {border}; background: {surface}; }}
        .canopa-files .error {{ padding: 10px; color: {danger}; background: {surface}; }}
        .canopa-files notebook > header {{ background: {raised}; }}
        .canopa-files textview, .canopa-files textview text {{ background: {surface}; color: {text}; }}
        .canopa-files selection {{ background-color: {selection}; color: {text}; }}
        .canopa-files .preview-body {{ background: {background}; padding: 12px; }}
        .canopa-files :focus-visible {{ outline-color: {focus}; }}
        .canopa-files .drop-target {{ outline: 2px solid {focus}; outline-offset: -2px; }}
    "#,
        background = rgb(p.background),
        surface = rgb(p.surface),
        raised = rgb(p.raised),
        text = rgb(p.text),
        muted = rgb(p.muted),
        border = rgb(p.border),
        selection = rgb(appearance.role(ColorRole::AccentSurface)),
        spine = rgb(appearance.role(ColorRole::Spine)),
        danger = rgb(p.danger),
        focus = rgb(appearance.role(ColorRole::Focus)),
        body = appearance.typography().body_size.px(),
        label = appearance.typography().label_size.px()
    );
    provider.load_from_string(&css);
}

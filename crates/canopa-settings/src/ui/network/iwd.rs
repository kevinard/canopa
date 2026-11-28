//! The Network page on an iwd and systemd-networkd system (layout of the
//! NetworkManager page): Wi-Fi from iwd, saved networks, wired links read from
//! networkd, and the sections that need NetworkManager, labelled as such.
use super::*;
use crate::network::{IwdAction, IwdState};
use canopa_services::network::{iwd, networkd, wifi::WifiSecurity};
use iced::widget::column;

fn iwd_apply(action: IwdAction) -> Message {
    apply(NetworkAction::Iwd(action))
}

fn security_name(security: WifiSecurity) -> String {
    security_label(match security {
        WifiSecurity::Open => Security::Open,
        WifiSecurity::Personal => Security::Personal,
        WifiSecurity::Enterprise => Security::Enterprise,
    })
}

/// A label and its value, such as "Gateway  192.168.1.1".
fn detail<'a>(label: String, values: &[String]) -> Element<'a, Message> {
    super::super::kit::row(label, None, text(values.join("\n")).size(13))
}

impl State {
    pub(super) fn iwd_view<'a>(&'a self, state: &'a IwdState) -> Element<'a, Message> {
        let needs =
            |title: String| settings_group(title, muted(fl!("settings-network-needs-manager")));
        column![
            self.iwd_wifi_section(state.wifi.as_ref()),
            self.iwd_saved_section(state.wifi.as_ref()),
            networkd_wired_section(state.links.as_deref()),
            needs("VPN".into()),
            needs(fl!("settings-network-hotspot")),
            needs(fl!("settings-network-connections")),
        ]
        .spacing(28)
        .into()
    }

    fn iwd_join_form(&self) -> Option<Element<'_, Message>> {
        let join = self.network_forms.iwd_join.as_ref()?;
        let forms = &self.network_forms;
        let (header, control): (Element<'_, Message>, String) = match join {
            Some(network) => (
                text(fl!(
                    "settings-network-join-named",
                    network = network.name.as_str()
                ))
                .into(),
                format!("wifi-{}", network.name),
            ),
            None => (
                text_field(forms, Field::Ssid, fl!("settings-network-name")),
                "wifi-join".into(),
            ),
        };
        let body = column![
            header,
            secret(
                "password",
                fl!("settings-network-wifi-password"),
                forms.get(Field::Password),
                |value| { edit(Edit::Text(Field::Password, value)) }
            ),
            direction::row([
                row_button(
                    "network-join".into(),
                    fl!("settings-network-join"),
                    Some(edit(Edit::SubmitIwdJoin))
                ),
                row_button(
                    "network-join-cancel".into(),
                    fl!("settings-cancel"),
                    Some(edit(Edit::CloseJoin))
                ),
            ])
            .spacing(10),
            self.network.control_status(&control, message),
        ]
        .spacing(10);
        Some(
            container(body)
                .padding(12)
                .width(Length::Fill)
                .style(super::super::kit::note)
                .into(),
        )
    }

    fn iwd_wifi_section<'a>(&'a self, wifi: Option<&'a iwd::Overview>) -> Element<'a, Message> {
        let page = &self.network;
        let Some(wifi) = wifi else {
            return settings_group("Wi-Fi", muted(fl!("settings-network-iwd-not-running")));
        };
        if wifi.devices.is_empty() {
            return settings_group("Wi-Fi", muted(fl!("settings-network-no-wifi")));
        }
        let mut body: Vec<Element<'_, Message>> = Vec::new();
        let several = wifi.devices.len() > 1;
        for device in &wifi.devices {
            let control = format!("radio-{}", device.name);
            let powered = match page.live.pending(&control) {
                Some(NetworkAction::Iwd(IwdAction::Powered(_, value))) => *value,
                _ => device.powered,
            };
            body.push(
                column![
                    toggle(
                        format!("network-{control}"),
                        if several {
                            format!("Wi-Fi ({})", device.name)
                        } else {
                            "Wi-Fi".into()
                        },
                        powered,
                        iwd_apply(IwdAction::Powered(device.clone(), !powered)),
                    ),
                    page.control_status(&control, message),
                ]
                .spacing(4)
                .into(),
            );
        }
        if let Some(form) = self.iwd_join_form() {
            body.push(form);
        }
        for network in wifi.networks.iter().take(16) {
            let description = fl!(
                "settings-network-ap-description",
                state = if network.connected {
                    "connected"
                } else if network.known.is_some() {
                    "saved"
                } else {
                    "other"
                },
                security = security_name(network.security()),
                strength = canopa_i18n::format::percent(f64::from(network.strength))
            );
            let control = format!("wifi-{}", network.name);
            let action: Element<'_, Message> = if network.connected {
                row_button(
                    format!("network-{control}"),
                    fl!("settings-network-disconnect"),
                    Some(iwd_apply(IwdAction::Disconnect(network.clone()))),
                )
            } else if network.known.is_some() || network.kind == "open" {
                row_button(
                    format!("network-{control}"),
                    fl!("settings-network-connect"),
                    Some(iwd_apply(IwdAction::Connect(network.clone(), None))),
                )
            } else if network.kind == "psk" {
                row_button(
                    format!("network-{control}"),
                    fl!("settings-network-connect-ask"),
                    Some(edit(Edit::JoinIwd(Some(network.clone())))),
                )
            } else {
                // iwd joins an enterprise network only from its provisioning file.
                muted(fl!("settings-network-needs-provisioning"))
            };
            body.push(
                column![
                    direction::row([
                        column![text(&network.name), muted(description)]
                            .spacing(2)
                            .align_x(direction::start())
                            .width(Length::Fill)
                            .into(),
                        action,
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                    page.control_status(&control, message),
                ]
                .spacing(4)
                .into(),
            );
        }
        if wifi.devices.iter().any(|device| device.state.is_some()) {
            if wifi.networks.is_empty() {
                body.push(super::super::kit::empty(fl!("settings-network-none-found")));
            }
            body.push(row_button(
                "network-hidden".into(),
                fl!("settings-network-join-hidden"),
                Some(edit(Edit::JoinIwd(None))),
            ));
        }
        settings_group("Wi-Fi", super::super::kit::rows(body))
    }

    fn iwd_saved_section<'a>(&'a self, wifi: Option<&'a iwd::Overview>) -> Element<'a, Message> {
        let page = &self.network;
        let Some(wifi) = wifi else {
            return settings_group(
                fl!("settings-network-saved"),
                muted(fl!("settings-network-iwd-not-running")),
            );
        };
        let mut body: Vec<Element<'_, Message>> = Vec::new();
        for known in &wifi.known {
            let mut description = security_name(known.security());
            if wifi.in_range(known).is_none() {
                description = fl!("settings-network-not-in-range", security = description);
            }
            let forget = format!("known-{}", known.name);
            let autoconnect = format!("{forget}-autoconnect");
            let enabled = match page.live.pending(&autoconnect) {
                Some(NetworkAction::Iwd(IwdAction::Autoconnect(_, value))) => *value,
                _ => known.autoconnect,
            };
            body.push(
                column![
                    direction::row([
                        column![text(&known.name), muted(description)]
                            .spacing(2)
                            .align_x(direction::start())
                            .width(Length::Fill)
                            .into(),
                        row_button(
                            format!("network-{forget}"),
                            fl!("settings-network-forget-network"),
                            Some(iwd_apply(IwdAction::Forget(known.clone()))),
                        ),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                    toggle(
                        format!("network-{autoconnect}"),
                        fl!("settings-network-autoconnect"),
                        enabled,
                        iwd_apply(IwdAction::Autoconnect(known.clone(), !enabled)),
                    ),
                    page.control_status(&forget, message),
                    page.control_status(&autoconnect, message),
                ]
                .spacing(4)
                .into(),
            );
        }
        if body.is_empty() {
            body.push(super::super::kit::empty(fl!("settings-network-no-saved")));
        }
        settings_group(fl!("settings-network-saved"), super::super::kit::rows(body))
    }
}

/// Wired links from networkd, read-only: networkd's `.network` files set them.
fn networkd_wired_section(links: Option<&[networkd::Link]>) -> Element<'static, Message> {
    let Some(links) = links else {
        return settings_group(
            fl!("settings-network-wired"),
            muted(fl!("settings-network-networkd-not-running")),
        );
    };
    let mut body: Vec<Element<'static, Message>> = Vec::new();
    for link in links.iter().filter(|link| link.kind == "ether") {
        let status = match link.operational.as_str() {
            "routable" | "degraded" | "enslaved" => fl!("settings-network-connected"),
            _ if !link.has_carrier() => fl!("settings-network-wired-unplugged"),
            _ => fl!("settings-network-disconnected"),
        };
        body.push(
            column![text(link.name.clone()), muted(status)]
                .spacing(2)
                .into(),
        );
        for (label, values) in [
            (fl!("settings-network-address"), &link.addresses),
            (fl!("settings-network-ip-gateway"), &link.gateways),
            ("DNS".to_owned(), &link.dns),
        ] {
            if !values.is_empty() {
                body.push(detail(label, values));
            }
        }
    }
    if body.is_empty() {
        body.push(super::super::kit::empty(fl!("settings-network-no-wired")));
    }
    body.push(muted(fl!("settings-network-wired-networkd-hint")));
    settings_group(fl!("settings-network-wired"), super::super::kit::rows(body))
}

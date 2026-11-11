use super::*;
use crate::{
    ServiceState,
    network::{profiles, secret_agent},
    testing::{TestBus, network::*},
};
use futures_util::StreamExt;

fn str_of<'a>(settings: &'a Settings, setting: &str, key: &str) -> &'a str {
    <&str>::try_from(&settings[setting][key]).unwrap()
}

#[tokio::test]
async fn the_overview_lists_devices_networks_profiles_and_plugins() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    assert_eq!(read(&client).await, Err(Error::Unavailable));
    let _nm = FakeNetworkManager::serve(&bus).await;
    let plugins = tempfile::tempdir().unwrap();
    std::fs::write(
        plugins.path().join("nm-openvpn-service.name"),
        "[VPN Connection]\nname=openvpn\nservice=org.freedesktop.NetworkManager.openvpn\n",
    )
    .unwrap();
    let overview = read_with_plugins(&client, plugins.path()).await.unwrap();
    assert!(overview.wireless_enabled);
    assert_eq!(overview.vpn_plugins[0].name, "openvpn");
    let ethernet = overview
        .devices
        .iter()
        .find(|device| device.kind == DeviceKind::Ethernet)
        .unwrap();
    assert!(ethernet.connected());
    assert_eq!(ethernet.connection.as_deref(), Some(WIRED));
    assert_eq!(ethernet.addresses, ["192.168.1.20/24"]);
    let names: Vec<_> = overview
        .access_points
        .iter()
        .map(AccessPoint::name)
        .collect();
    assert_eq!(names, ["Home", "eduroam", "Cafe"]);
    let securities: Vec<_> = overview
        .access_points
        .iter()
        .map(|ap| ap.security)
        .collect();
    assert_eq!(
        securities,
        [Security::Personal, Security::Enterprise, Security::Open]
    );
    assert_eq!(
        overview
            .saved_for(&overview.access_points[0])
            .map(|c| c.id.as_str()),
        Some("Home")
    );
    let wired = overview.connection(WIRED).unwrap();
    assert_eq!(wired.kind, ConnectionKind::Ethernet);
    assert!(wired.active.is_some());
    assert_eq!(
        overview.connection(VPN).unwrap().kind,
        ConnectionKind::Vpn("org.freedesktop.NetworkManager.openvpn".into())
    );
}

#[tokio::test]
async fn wifi_vpn_and_profile_commands_reach_network_manager() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    let nm = FakeNetworkManager::serve(&bus).await;
    let mut changes = watch(client.clone());
    assert!(matches!(
        changes.next().await,
        Some(ServiceState::Available(_))
    ));

    set_wireless_enabled(&client, false).await.unwrap();
    assert!(nm.calls().contains(&"WirelessEnabled false".to_string()));
    assert!(read(&client).await.unwrap().access_points.is_empty());
    set_wireless_enabled(&client, true).await.unwrap();

    // VPN connect and disconnect.
    let overview = read(&client).await.unwrap();
    let vpn = Target::from(overview.connection(VPN).unwrap());
    let active = activate(&client, &vpn, "/", "/").await.unwrap();
    assert!(
        read(&client)
            .await
            .unwrap()
            .connection(VPN)
            .unwrap()
            .active
            .is_some()
    );
    deactivate(&client, &active).await.unwrap();
    assert!(
        read(&client)
            .await
            .unwrap()
            .connection(VPN)
            .unwrap()
            .active
            .is_none()
    );

    // A stale target is refused rather than changing another profile.
    let stale = Target {
        path: VPN.into(),
        uuid: "00000000-0000-4000-8000-000000000000".into(),
    };
    assert_eq!(
        activate(&client, &stale, "/", "/").await,
        Err(Error::Changed)
    );

    // Settings changes keep stored secrets.
    let home = Target::from(overview.connection(HOME_PROFILE).unwrap());
    update(&client, &home, &Change::Autoconnect(false))
        .await
        .unwrap();
    update(&client, &home, &Change::Metered(Metered::Yes))
        .await
        .unwrap();
    update(
        &client,
        &home,
        &Change::Proxy(Proxy::Manual {
            server: "proxy.example.com:3128".into(),
        }),
    )
    .await
    .unwrap();
    let stored = nm.profile("Home").unwrap();
    assert_eq!(
        str_of(&stored, "802-11-wireless-security", "psk"),
        "stored-secret"
    );
    let home_now = read(&client).await.unwrap();
    let home_now = home_now.connection(HOME_PROFILE).unwrap();
    assert!(!home_now.autoconnect);
    assert_eq!(home_now.metered, Metered::Yes);
    assert_eq!(
        home_now.proxy,
        Proxy::Manual {
            server: "proxy.example.com:3128".into()
        }
    );
    assert!(matches!(
        update(
            &client,
            &home,
            &Change::Proxy(Proxy::Manual {
                server: "nope".into()
            })
        )
        .await,
        Err(Error::Invalid(_))
    ));

    // Wired IPv4 by hand, with DNS, applied again because it is connected.
    let wired = Target::from(read(&client).await.unwrap().connection(WIRED).unwrap());
    let manual = IpConfig {
        method: "manual".into(),
        addresses: vec!["192.168.1.50/24".into()],
        gateway: Some("192.168.1.1".into()),
        dns: vec!["9.9.9.9".into()],
    };
    update(&client, &wired, &Change::Ip("ipv4", manual.clone()))
        .await
        .unwrap();
    let read_back = read(&client).await.unwrap();
    assert_eq!(read_back.connection(WIRED).unwrap().ipv4, manual);
    assert!(
        nm.calls()
            .iter()
            .any(|call| call == &format!("Activate {WIRED}"))
    );
    assert!(matches!(
        update(
            &client,
            &wired,
            &Change::Ip(
                "ipv4",
                IpConfig {
                    method: "manual".into(),
                    addresses: vec!["300.1.1.1/24".into()],
                    ..Default::default()
                }
            )
        )
        .await,
        Err(Error::Invalid(_))
    ));

    // Forget a network.
    delete(&client, &home).await.unwrap();
    assert!(
        read(&client)
            .await
            .unwrap()
            .connection(HOME_PROFILE)
            .is_none()
    );
    assert_eq!(delete(&client, &home).await, Err(Error::Changed));

    // New profiles: WireGuard, a hidden network and a hotspot.
    let tunnel = profiles::parse_wireguard(
        "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.0.0.2/24\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nAllowedIPs = 0.0.0.0/0\nEndpoint = vpn.example.com:51820\n",
        "Tunnel",
    )
    .unwrap();
    add(&client, profiles::wireguard(&tunnel).unwrap())
        .await
        .unwrap();
    let hidden = profiles::wifi(
        b"Secret",
        &profiles::WifiSecurity::Personal {
            password: "hidden network".into(),
            sae: false,
            storage: profiles::SecretStorage::System,
        },
        true,
    )
    .unwrap();
    add_and_activate(&client, hidden, WIFI, "/").await.unwrap();
    let (_, hotspot) = add_and_activate(
        &client,
        profiles::hotspot("Share", "share this").unwrap(),
        WIFI,
        "/",
    )
    .await
    .unwrap();
    let overview = read(&client).await.unwrap();
    let kinds: Vec<_> = overview
        .connections
        .iter()
        .map(|c| (c.id.as_str(), c.kind.clone()))
        .collect();
    assert!(kinds.contains(&("Tunnel", ConnectionKind::WireGuard)));
    assert!(
        kinds.iter().any(|(id, kind)| *id == "Hotspot"
            && matches!(kind, ConnectionKind::Wifi { hotspot: true, .. }))
    );
    // The hotspot replaced the hidden network on the Wi-Fi device.
    assert!(
        overview
            .connections
            .iter()
            .find(|c| c.id == "Secret")
            .unwrap()
            .active
            .is_none()
    );
    deactivate(&client, &hotspot).await.unwrap();
    disconnect_device(&client, ETHERNET).await.unwrap();
    assert!(!read(&client).await.unwrap().devices[0].connected());

    // Each change was followed.
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), changes.next())
            .await
            .unwrap()
            .is_some_and(|state| state.is_available())
    );
}

#[tokio::test]
async fn the_agent_asks_only_when_network_manager_allows_it_and_keeps_nothing() {
    let bus = TestBus::new();
    let nm = FakeNetworkManager::serve(&bus).await;
    let client = bus.connect().await;
    let (registration, mut requests) = secret_agent::register(&client, "org.canopa.Test")
        .await
        .unwrap();
    // Without interaction, an agent without a store has nothing to say.
    let error = nm
        .ask_agent("Home", "802-11-wireless-security", &[], 0)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("NoSecrets"), "{error}");
    let ask = nm.ask_agent("Home", "802-11-wireless-security", &[], 0x3);
    let answer = async {
        let request = requests.recv().await.unwrap();
        assert_eq!(request.connection, "Home");
        assert!(request.retry);
        assert_eq!(request.fields[0].key, "psk");
        request.answer(HashMap::from([("psk".into(), "typed now".into())]));
    };
    let (secrets, ()) = tokio::join!(ask, answer);
    let secrets = secrets.unwrap();
    assert_eq!(
        str_of(&secrets, "802-11-wireless-security", "psk"),
        "typed now"
    );
    // A cancelled question is UserCanceled.
    let ask = nm.ask_agent("Office VPN", "vpn", &["password"], 0x1);
    let cancel = async { requests.recv().await.unwrap().cancel() };
    let (result, ()) = tokio::join!(ask, cancel);
    assert!(result.unwrap_err().to_string().contains("UserCanceled"));
    // Only NetworkManager may ask.
    let stranger = bus.connect().await;
    let proxy = zbus::Proxy::new(
        &stranger,
        client.unique_name().unwrap().to_string(),
        secret_agent::AGENT_PATH,
        "org.freedesktop.NetworkManager.SecretAgent",
    )
    .await
    .unwrap();
    let refused: zbus::Result<Settings> = proxy
        .call(
            "GetSecrets",
            &(
                Settings::new(),
                zbus::zvariant::ObjectPath::try_from("/x").unwrap(),
                "vpn",
                Vec::<String>::new(),
                1u32,
            ),
        )
        .await;
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("PermissionDenied")
    );
    drop(registration);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while nm.state.lock().unwrap().agent.is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

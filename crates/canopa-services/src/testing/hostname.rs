//! A fake systemd-hostnamed for a laptop named `canopa-test`.
use super::TestBus;
use crate::hostname::{PATH, SERVICE};
use std::sync::{Arc, Mutex};
use zbus::{fdo, object_server::SignalEmitter};

struct Hostnamed(Arc<Mutex<(String, String)>>);

#[zbus::interface(name = "org.freedesktop.hostname1")]
impl Hostnamed {
    async fn set_pretty_hostname(
        &self,
        name: &str,
        _interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.0.lock().unwrap().0 = name.into();
        self.pretty_hostname_changed(&emitter).await?;
        Ok(())
    }
    async fn set_static_hostname(
        &self,
        name: &str,
        _interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.0.lock().unwrap().1 = name.into();
        self.static_hostname_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(property)]
    fn hostname(&self) -> String {
        self.0.lock().unwrap().1.clone()
    }
    #[zbus(property)]
    fn static_hostname(&self) -> String {
        self.0.lock().unwrap().1.clone()
    }
    #[zbus(property)]
    fn pretty_hostname(&self) -> String {
        self.0.lock().unwrap().0.clone()
    }
    #[zbus(property)]
    fn chassis(&self) -> &str {
        "laptop"
    }
    #[zbus(property)]
    fn hardware_vendor(&self) -> &str {
        "Test Computers"
    }
    #[zbus(property)]
    fn hardware_model(&self) -> &str {
        "Test Book 14"
    }
    #[zbus(property)]
    fn firmware_version(&self) -> &str {
        "1.2.3"
    }
    #[zbus(property)]
    fn kernel_name(&self) -> &str {
        "Linux"
    }
    #[zbus(property)]
    fn kernel_release(&self) -> &str {
        "6.12.0"
    }
    #[zbus(property)]
    fn operating_system_pretty_name(&self) -> &str {
        "Arch Linux"
    }
}

/// hostnamed on a private bus.
pub struct FakeHostnamed {
    pub connection: zbus::Connection,
    /// The pretty and static names.
    pub state: Arc<Mutex<(String, String)>>,
}

impl FakeHostnamed {
    pub async fn serve(bus: &TestBus) -> Self {
        let state = Arc::new(Mutex::new((String::new(), "canopa-test".to_owned())));
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(PATH, Hostnamed(state.clone()))
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, state }
    }
}

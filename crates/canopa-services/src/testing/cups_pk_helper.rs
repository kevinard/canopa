//! A fake cups-pk-helper that records calls and knows one queue at a time.
use super::TestBus;
use crate::cups_pk_helper::{PATH, SERVICE};
use std::sync::{Arc, Mutex};

struct Mechanism {
    calls: Arc<Mutex<Vec<String>>>,
    queues: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.opensuse.CupsPkHelper.Mechanism")]
impl Mechanism {
    fn printer_add(
        &self,
        name: &str,
        uri: &str,
        ppd: &str,
        _info: &str,
        _location: &str,
    ) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("PrinterAdd {name} {uri} {ppd}"));
        self.queues.lock().unwrap().push(name.into());
        String::new()
    }
    fn printer_delete(&self, name: &str) -> String {
        let mut queues = self.queues.lock().unwrap();
        if !queues.iter().any(|queue| queue == name) {
            return "The printer or class does not exist.".into();
        }
        queues.retain(|queue| queue != name);
        self.calls
            .lock()
            .unwrap()
            .push(format!("PrinterDelete {name}"));
        String::new()
    }
    fn printer_set_enabled(&self, name: &str, _enabled: bool) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("PrinterSetEnabled {name}"));
        String::new()
    }
    fn printer_set_accept_jobs(&self, name: &str, _accept: bool, _reason: &str) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("PrinterSetAcceptJobs {name}"));
        String::new()
    }
    fn printer_set_default(&self, name: &str) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("PrinterSetDefault {name}"));
        String::new()
    }
    fn job_cancel_purge(&self, job: i32, _purge: bool) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("JobCancelPurge {job}"));
        String::new()
    }
}

/// cups-pk-helper on a private bus.
pub struct FakeCupsPkHelper {
    pub connection: zbus::Connection,
    /// Each call, as `Method arguments`.
    pub calls: Arc<Mutex<Vec<String>>>,
}

impl FakeCupsPkHelper {
    pub async fn serve(bus: &TestBus) -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(
                PATH,
                Mechanism {
                    calls: calls.clone(),
                    queues: Arc::default(),
                },
            )
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self { connection, calls }
    }
}

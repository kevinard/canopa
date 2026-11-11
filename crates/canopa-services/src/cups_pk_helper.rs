//! cups-pk-helper: printer administration for people allowed by polkit,
//! without membership in CUPS's system group.
//!
//! The mechanism asks polkit for each call, which may show a password
//! prompt; it then asks CUPS. Each call returns CUPS's error text, empty on
//! success, which becomes [`Error::Refused`].
//!
//! ```no_run
//! use canopa_services::cups_pk_helper as helper;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! if helper::available(&system).await {
//!     helper::printer_add(&system, "Office", "ipp://office.local/ipp/print", "everywhere", "Office printer", "").await?;
//!     helper::printer_set_default(&system, "Office").await?;
//! }
//! # Ok(()) }
//! ```
use crate::{Error, bus::call_interactive};

pub const SERVICE: &str = "org.opensuse.CupsPkHelper.Mechanism";
pub const PATH: &str = "/";
pub const INTERFACE: &str = "org.opensuse.CupsPkHelper.Mechanism";

/// Whether cups-pk-helper is running or can be started.
pub async fn available(connection: &zbus::Connection) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(connection).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(SERVICE) else {
        return false;
    };
    if let Ok(Ok(true)) =
        tokio::time::timeout(crate::bus::DEADLINE, dbus.name_has_owner(name)).await
    {
        return true;
    }
    matches!(
        tokio::time::timeout(crate::bus::DEADLINE, dbus.list_activatable_names()).await,
        Ok(Ok(names)) if names.iter().any(|known| known.as_str() == SERVICE)
    )
}

/// A CUPS queue name: printable, no spaces, `/` or `#`, at most 127 bytes.
pub fn valid_queue(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 127
        && name
            .chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '/' | '#' | '\\' | '\'' | '"'))
}

async fn call<B>(connection: &zbus::Connection, method: &str, body: &B) -> Result<(), Error>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let proxy = zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?;
    let error: String = call_interactive(&proxy, method, body).await?;
    if error.trim().is_empty() {
        Ok(())
    } else {
        Err(Error::Refused(crate::bus::bounded_text(error.trim(), 200)))
    }
}

/// Add (or change) a queue. `ppd` is a PPD name; `everywhere` asks CUPS to
/// set up a driverless IPP Everywhere queue from the printer itself.
pub async fn printer_add(
    connection: &zbus::Connection,
    name: &str,
    uri: &str,
    ppd: &str,
    info: &str,
    location: &str,
) -> Result<(), Error> {
    if !valid_queue(name) {
        return Err(Error::Invalid("printer name"));
    }
    if uri.len() > 1024 || uri.chars().any(char::is_control) || !uri.contains(':') {
        return Err(Error::Invalid("printer address"));
    }
    call(connection, "PrinterAdd", &(name, uri, ppd, info, location)).await
}

/// Enable a queue and let it accept jobs, as a newly added queue needs.
pub async fn printer_enable(connection: &zbus::Connection, name: &str) -> Result<(), Error> {
    if !valid_queue(name) {
        return Err(Error::Invalid("printer name"));
    }
    call(connection, "PrinterSetEnabled", &(name, true)).await?;
    call(connection, "PrinterSetAcceptJobs", &(name, true, "")).await
}

pub async fn printer_delete(connection: &zbus::Connection, name: &str) -> Result<(), Error> {
    if !valid_queue(name) {
        return Err(Error::Invalid("printer name"));
    }
    call(connection, "PrinterDelete", &(name,)).await
}

/// Make a queue the default for everyone.
pub async fn printer_set_default(connection: &zbus::Connection, name: &str) -> Result<(), Error> {
    if !valid_queue(name) {
        return Err(Error::Invalid("printer name"));
    }
    call(connection, "PrinterSetDefault", &(name,)).await
}

/// Cancel anyone's job.
pub async fn job_cancel(connection: &zbus::Connection, job: i32) -> Result<(), Error> {
    call(connection, "JobCancelPurge", &(job, false)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, cups_pk_helper::FakeCupsPkHelper};

    #[tokio::test]
    async fn adds_removes_and_reports_cups_errors() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert!(!available(&client).await);
        let fake = FakeCupsPkHelper::serve(&bus).await;
        assert!(available(&client).await);
        printer_add(
            &client,
            "Office",
            "ipp://office.local/ipp/print",
            "everywhere",
            "Office",
            "",
        )
        .await
        .unwrap();
        printer_enable(&client, "Office").await.unwrap();
        printer_set_default(&client, "Office").await.unwrap();
        job_cancel(&client, 12).await.unwrap();
        printer_delete(&client, "Office").await.unwrap();
        assert_eq!(
            fake.calls.lock().unwrap().as_slice(),
            [
                "PrinterAdd Office ipp://office.local/ipp/print everywhere",
                "PrinterSetEnabled Office",
                "PrinterSetAcceptJobs Office",
                "PrinterSetDefault Office",
                "JobCancelPurge 12",
                "PrinterDelete Office"
            ]
        );
        assert_eq!(
            printer_delete(&client, "Missing").await,
            Err(Error::Refused(
                "The printer or class does not exist.".into()
            ))
        );
        assert_eq!(
            printer_add(&client, "Bad Name", "ipp://x", "", "", "").await,
            Err(Error::Invalid("printer name"))
        );
    }
}

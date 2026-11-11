//! Service errors in the person's language.
//!
//! `canopa_services::Error::Invalid` carries the shared services crate's
//! English text: a sentence ("A Wi-Fi password has 8 to 63 characters.") or
//! the name of the value that was refused ("user name"). The live pages'
//! `failure()` mappings show it through [`invalid`]; text this table does not
//! know is shown as the service wrote it.
use canopa_services::Error;

/// An `Error::Invalid` text as a sentence people read.
pub fn invalid(what: &str) -> String {
    match what {
        // Values that were refused.
        "device name" => fl!("settings-invalid-device-name"),
        "host name" => fl!("settings-invalid-host-name"),
        "full name" => fl!("settings-invalid-full-name"),
        "user name" => fl!("settings-invalid-user-name"),
        "picture" => fl!("settings-invalid-picture"),
        "language" | "locale" => fl!("settings-invalid-language"),
        "formats" => fl!("settings-invalid-formats"),
        "keyboard layout" => fl!("settings-invalid-keyboard-layout"),
        "printer name" => fl!("settings-invalid-printer-name"),
        "printer address" => fl!("settings-invalid-printer-address"),
        "time zone name" => fl!("settings-invalid-time-zone"),
        "time before 1970" => fl!("settings-invalid-time-before-1970"),
        "account path" | "bus name" | "device path" | "object path" | "interface name" => {
            fl!("settings-invalid-request")
        }
        // Sentences.
        "A DNS server is not a valid address." => fl!("settings-invalid-dns"),
        "A Wi-Fi password has 8 to 63 characters." => fl!("settings-invalid-wifi-password"),
        "A hotspot password has 8 to 63 characters." => fl!("settings-invalid-hotspot-password"),
        "A network name has 1 to 32 bytes." => fl!("settings-invalid-network-name"),
        "A preshared key is not a WireGuard key." => fl!("settings-invalid-preshared-key"),
        "A tunnel needs a peer." => fl!("settings-invalid-tunnel-peer"),
        "An address is not valid." => fl!("settings-invalid-address"),
        "Brightness must be between 1% and 100%." => fl!("settings-invalid-brightness"),
        "Certificate paths must be UTF-8." => fl!("settings-invalid-certificate-path"),
        "Choose an authentication method." => fl!("settings-invalid-eap-method"),
        "Enter the password." => fl!("settings-invalid-password-missing"),
        "Enter your identity (user name)." => fl!("settings-invalid-identity-missing"),
        "ListenPort is not a port." => fl!("settings-invalid-listen-port"),
        "MTU is not a number." => fl!("settings-invalid-mtu"),
        "Manual addressing needs an address." => fl!("settings-invalid-manual-address"),
        "Name the connection." => fl!("settings-invalid-connection-name"),
        "PersistentKeepalive is not a number." => fl!("settings-invalid-keepalive"),
        "The file has an unknown peer setting." => fl!("settings-invalid-peer-setting"),
        "The file has an unknown section." => fl!("settings-invalid-section"),
        "The file is not a WireGuard configuration." => fl!("settings-invalid-wireguard-file"),
        "The gateway is not a valid address." => fl!("settings-invalid-gateway"),
        "The prefix must be a number." => fl!("settings-invalid-prefix"),
        "The private key is not a WireGuard key." => fl!("settings-invalid-private-key"),
        "Unknown address method." => fl!("settings-invalid-address-method"),
        "Write the proxy as host:port." => fl!("settings-invalid-proxy"),
        "A Wayland login is required." => fl!("settings-invalid-wayland-login"),
        "An active local login is required." => fl!("settings-invalid-local-login"),
        other if other.ends_with('.') => other.to_owned(),
        other => fl!("settings-error-invalid-value", what = other),
    }
}

/// The usual words for a service error; `unavailable` names the service.
pub fn failed(error: Error, unavailable: impl FnOnce() -> String) -> String {
    match error {
        Error::Unavailable => unavailable(),
        Error::NotAuthorized => fl!("settings-error-not-authorized"),
        Error::TimedOut => fl!("settings-error-timed-out"),
        Error::Changed => fl!("settings-error-setting-changed"),
        Error::Invalid(what) => invalid(what),
        Error::Refused(message) => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_service_text_is_translated_and_the_rest_kept() {
        assert_eq!(invalid("user name"), "That is not a valid user name.");
        assert_eq!(
            invalid("A Wi-Fi password has 8 to 63 characters."),
            "A Wi-Fi password has 8 to 63 characters."
        );
        assert_eq!(invalid("Something new."), "Something new.");
        assert_eq!(invalid("widget"), "That is not a valid widget.");
    }
}

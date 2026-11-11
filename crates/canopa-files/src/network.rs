//! Network locations, independent of GIO and the renderer: server addresses the
//! user may connect to, the GVFS backend package each one needs, readable
//! connection failures, and where GVFS exposes mounted locations as folders.
use std::path::{Path, PathBuf};

/// Address input longer than this is refused before parsing.
pub const MAX_ADDRESS: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scheme {
    Smb,
    Sftp,
    Ftp,
    Ftps,
    Dav,
    Davs,
    Nfs,
}

impl Scheme {
    pub const ALL: [Self; 7] = [
        Self::Smb,
        Self::Sftp,
        Self::Ftp,
        Self::Ftps,
        Self::Dav,
        Self::Davs,
        Self::Nfs,
    ];

    /// The scheme GIO receives. `ssh://` is accepted as an alias of `sftp://`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Smb => "smb",
            Self::Sftp => "sftp",
            Self::Ftp => "ftp",
            Self::Ftps => "ftps",
            Self::Dav => "dav",
            Self::Davs => "davs",
            Self::Nfs => "nfs",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_ascii_lowercase().as_str() {
            "ssh" => Some(Self::Sftp),
            id => Self::ALL.into_iter().find(|scheme| scheme.id() == id),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Smb => "Windows share (SMB)",
            Self::Sftp => "SSH (SFTP)",
            Self::Ftp => "FTP",
            Self::Ftps => "FTP with TLS",
            Self::Dav => "WebDAV",
            Self::Davs => "WebDAV with TLS",
            Self::Nfs => "NFS",
        }
    }

    /// The Arch package that installs this GVFS backend.
    pub fn package(self) -> &'static str {
        backend_package(self.id()).unwrap_or("gvfs")
    }
}

/// The Arch package providing the GVFS backend for a URI scheme, for the
/// locations Files can connect to, browse or mount.
pub fn backend_package(scheme: &str) -> Option<&'static str> {
    Some(match scheme {
        "smb" => "gvfs-smb",
        "nfs" => "gvfs-nfs",
        "mtp" => "gvfs-mtp",
        "gphoto2" => "gvfs-gphoto2",
        "afc" => "gvfs-afc",
        "dns-sd" => "gvfs-dnssd",
        "wsdd" => "gvfs-wsdd",
        "sftp" | "ssh" | "ftp" | "ftps" | "ftpis" | "dav" | "davs" | "network" => "gvfs",
        _ => return None,
    })
}

/// Why a location scheme is unusable: GIO does not list it as supported.
pub fn missing_backend(scheme: &str) -> String {
    let what = match scheme {
        "smb" => "Windows shares (SMB)",
        "sftp" | "ssh" => "SSH servers (SFTP)",
        "ftp" | "ftps" | "ftpis" => "FTP servers",
        "dav" | "davs" => "WebDAV servers",
        "nfs" => "NFS servers",
        "mtp" => "phones and media players (MTP)",
        "gphoto2" => "cameras (PTP)",
        "afc" => "iPhones and iPads",
        "network" => "browsing the local network",
        _ => "this kind of location",
    };
    match backend_package(scheme) {
        Some("gvfs") => format!(
            "Files cannot open {what}: GVFS is not installed or its {scheme} backend is missing. Install the gvfs package."
        ),
        Some(package) => {
            format!("Files cannot open {what}: install the {package} package, then try again.")
        }
        None => format!("Files cannot open {what}: no GVFS backend handles it."),
    }
}

/// A validated server address. `uri` is what GIO mounts; it never holds a
/// password, so it is safe to show, log and bookmark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerAddress {
    pub scheme: Scheme,
    pub uri: String,
    pub host: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// Decoded path, `/` for the server's top level.
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("Enter a server address, for example smb://server/share.")]
    Empty,
    #[error("The address is too long.")]
    TooLong,
    #[error(
        "Start the address with a protocol: smb://, sftp://, ftp://, ftps://, dav://, davs:// or nfs://."
    )]
    MissingScheme,
    #[error(
        "Files cannot connect to {0}:// addresses. Use smb, sftp, ftp, ftps, dav, davs or nfs."
    )]
    UnsupportedScheme(String),
    #[error("For WebDAV, use dav:// or davs:// instead of {0}://.")]
    WebDavHint(String),
    #[error("Enter the server's name or address after {0}://.")]
    MissingHost(String),
    #[error("Don't include a password in the address. Files asks for it when it connects.")]
    Password,
    #[error("The address cannot contain a query (?) or fragment (#).")]
    QueryOrFragment,
    #[error("The address is not valid: {0}.")]
    Invalid(String),
}

impl ServerAddress {
    /// Parse what a user typed. Accepts `ssh://` for SFTP and Windows UNC
    /// paths (`\\server\share`) for SMB; refuses embedded passwords.
    pub fn parse(input: &str) -> Result<Self, AddressError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(AddressError::Empty);
        }
        if input.len() > MAX_ADDRESS {
            return Err(AddressError::TooLong);
        }
        if input.chars().any(char::is_control) {
            return Err(AddressError::Invalid(
                "it contains control characters".into(),
            ));
        }
        let unc;
        let input = if let Some(rest) = input.strip_prefix(r"\\") {
            unc = format!("smb://{}", rest.replace('\\', "/"));
            unc.as_str()
        } else {
            input
        };
        let Some((scheme, rest)) = input.split_once("://") else {
            return Err(AddressError::MissingScheme);
        };
        if scheme.is_empty()
            || !scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
        {
            return Err(AddressError::MissingScheme);
        }
        let lowered = scheme.to_ascii_lowercase();
        let Some(kind) = Scheme::from_id(&lowered) else {
            return Err(match lowered.as_str() {
                "http" | "https" | "webdav" | "webdavs" => AddressError::WebDavHint(lowered),
                _ => AddressError::UnsupportedScheme(lowered),
            });
        };
        if rest.is_empty() || rest.starts_with('/') {
            return Err(AddressError::MissingHost(lowered));
        }
        let url = url::Url::parse(&format!("{}://{rest}", kind.id()))
            .map_err(|error| AddressError::Invalid(error.to_string()))?;
        if url.password().is_some() {
            return Err(AddressError::Password);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(AddressError::QueryOrFragment);
        }
        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| AddressError::MissingHost(lowered.clone()))?
            .to_owned();
        if host.contains(['\\', ' ']) {
            return Err(AddressError::Invalid(
                "the server name contains spaces".into(),
            ));
        }
        let user = Some(url.username())
            .filter(|user| !user.is_empty())
            .map(|user| percent_decode(user));
        let path = match url.path() {
            "" => "/".to_owned(),
            path => percent_decode(path),
        };
        let mut uri = String::from(url.as_str());
        if url.path().is_empty() {
            uri.push('/');
        }
        Ok(Self {
            scheme: kind,
            uri,
            host,
            user,
            port: url.port(),
            path,
        })
    }

    /// A short name for sidebars and bookmarks: `share on server`, or the host.
    pub fn label(&self) -> String {
        let segment = self
            .path
            .split('/')
            .find(|segment| !segment.is_empty())
            .map(str::to_owned);
        let host = match &self.user {
            Some(user) => format!("{user}@{}", self.host),
            None => self.host.clone(),
        };
        match segment {
            Some(segment) => format!("{segment} on {host}"),
            None => host,
        }
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(value) = bytes
                .get(index + 1..index + 3)
                .and_then(|pair| std::str::from_utf8(pair).ok())
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        {
            output.push(value);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

/// Why a connection failed, from the GIO error code. The helper classifies;
/// this module words the message for the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Host not found, refused, unreachable or timed out.
    Unreachable,
    /// Rejected credentials or permission.
    Credentials,
    /// No backend for the scheme.
    Unsupported,
    Other,
}

/// The message shown when connecting to `uri` failed.
pub fn describe_failure(uri: &str, failure: Failure, detail: &str) -> String {
    let scheme = uri.split_once(':').map_or("", |(scheme, _)| scheme);
    let server = url::Url::parse(uri)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| uri.to_owned());
    let detail = detail.trim().trim_end_matches('.');
    match failure {
        Failure::Unreachable => format!(
            "Files cannot reach {server}. Check that the server is on and connected to the network, and that the address is right ({detail})."
        ),
        Failure::Credentials => format!(
            "{server} refused the login. Check the user name, domain and password ({detail})."
        ),
        Failure::Unsupported => missing_backend(scheme),
        Failure::Other if detail.is_empty() => format!("Files could not connect to {server}."),
        Failure::Other => format!("Files could not connect to {server}: {detail}."),
    }
}

/// Where GVFS's FUSE daemon exposes mounted locations as ordinary folders:
/// `$XDG_RUNTIME_DIR/gvfs`, normally `/run/user/UID/gvfs`.
pub fn gvfs_fuse_root(runtime: Option<&Path>) -> PathBuf {
    match runtime.filter(|path| path.is_absolute()) {
        Some(runtime) => runtime.join("gvfs"),
        None => PathBuf::from(format!(
            "/run/user/{}/gvfs",
            rustix::process::getuid().as_raw()
        )),
    }
}

/// The kind of location a URI scheme names, for sidebars and policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum LocationKind {
    #[default]
    Local,
    Network,
    Phone,
    Camera,
}

impl LocationKind {
    pub fn of_scheme(scheme: &str) -> Self {
        match scheme.to_ascii_lowercase().as_str() {
            "file" | "" => Self::Local,
            "mtp" | "afc" => Self::Phone,
            "gphoto2" => Self::Camera,
            "smb" | "sftp" | "ssh" | "ftp" | "ftps" | "ftpis" | "dav" | "davs" | "dav+sd"
            | "davs+sd" | "nfs" | "afp" | "http" | "https" | "google-drive" | "onedrive" => {
                Self::Network
            }
            // Other GVFS backends (admin, recent, computer, ...) are not storage.
            _ => Self::Local,
        }
    }

    pub fn of_uri(uri: &str) -> Self {
        uri.split_once(':')
            .map_or(Self::Local, |(scheme, _)| Self::of_scheme(scheme))
    }

    pub fn remote(self) -> bool {
        self != Self::Local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_normalize_aliases_and_keep_their_parts() {
        let smb = ServerAddress::parse("  SMB://NAS.local/Family Photos  ").unwrap();
        assert_eq!(smb.scheme, Scheme::Smb);
        assert_eq!(smb.uri, "smb://NAS.local/Family%20Photos");
        assert_eq!(smb.path, "/Family Photos");
        assert_eq!(smb.label(), "Family Photos on NAS.local");

        let ssh = ServerAddress::parse("ssh://me@box.example:2222/srv/data").unwrap();
        assert_eq!(ssh.scheme, Scheme::Sftp);
        assert_eq!(ssh.uri, "sftp://me@box.example:2222/srv/data");
        assert_eq!(ssh.user.as_deref(), Some("me"));
        assert_eq!(ssh.port, Some(2222));
        assert_eq!(ssh.label(), "srv on me@box.example");

        let top = ServerAddress::parse("sftp://box").unwrap();
        assert_eq!(top.uri, "sftp://box/");
        assert_eq!(top.path, "/");
        assert_eq!(top.label(), "box");

        let unc = ServerAddress::parse(r"\\server\share\docs").unwrap();
        assert_eq!(unc.uri, "smb://server/share/docs");

        let ipv6 = ServerAddress::parse("davs://[fe80::1]:8443/dav/").unwrap();
        assert_eq!(ipv6.host, "[fe80::1]");
        assert_eq!(ipv6.port, Some(8443));

        let ftp = ServerAddress::parse("ftp://files.example/pub").unwrap();
        assert_eq!(ftp.uri, "ftp://files.example/pub");
        assert_eq!(ftp.port, None, "the default port is implied");
        for (input, scheme) in [
            ("ftps://h/", Scheme::Ftps),
            ("dav://h/", Scheme::Dav),
            ("nfs://h/export", Scheme::Nfs),
        ] {
            assert_eq!(ServerAddress::parse(input).unwrap().scheme, scheme);
        }
    }

    #[test]
    fn invalid_addresses_explain_what_to_change() {
        for (input, error) in [
            ("", AddressError::Empty),
            ("nas.local/share", AddressError::MissingScheme),
            ("smb://", AddressError::MissingHost("smb".into())),
            ("smb:///share", AddressError::MissingHost("smb".into())),
            (
                "gopher://h/",
                AddressError::UnsupportedScheme("gopher".into()),
            ),
            ("https://h/dav", AddressError::WebDavHint("https".into())),
            (
                "file:///home",
                AddressError::UnsupportedScheme("file".into()),
            ),
            ("sftp://me:secret@box/", AddressError::Password),
            ("smb://nas/share?x=1", AddressError::QueryOrFragment),
            ("smb://nas/share#top", AddressError::QueryOrFragment),
        ] {
            assert_eq!(ServerAddress::parse(input), Err(error), "{input}");
        }
        assert!(matches!(
            ServerAddress::parse("sftp://box:99999/"),
            Err(AddressError::Invalid(_))
        ));
        assert!(matches!(
            ServerAddress::parse("smb://nas/a\u{7}b"),
            Err(AddressError::Invalid(_))
        ));
        assert_eq!(
            ServerAddress::parse(&format!("smb://h/{}", "a".repeat(MAX_ADDRESS))),
            Err(AddressError::TooLong)
        );
        assert!(
            !AddressError::Password.to_string().contains("secret"),
            "messages never repeat input"
        );
    }

    #[test]
    fn missing_backends_name_their_package() {
        assert!(missing_backend("smb").contains("gvfs-smb"));
        assert!(missing_backend("mtp").contains("gvfs-mtp"));
        assert!(missing_backend("gphoto2").contains("gvfs-gphoto2"));
        assert!(missing_backend("nfs").contains("gvfs-nfs"));
        assert!(missing_backend("sftp").contains("gvfs package"));
        assert_eq!(Scheme::Smb.package(), "gvfs-smb");
        assert_eq!(Scheme::Dav.package(), "gvfs");
        let unsupported = describe_failure("smb://nas/x", Failure::Unsupported, "");
        assert!(unsupported.contains("gvfs-smb"), "{unsupported}");
        let offline = describe_failure("sftp://box/", Failure::Unreachable, "Timed out.");
        assert!(
            offline.contains("cannot reach box") && offline.contains("(Timed out)"),
            "{offline}"
        );
        assert!(
            describe_failure("smb://nas/x", Failure::Credentials, "Permission denied")
                .starts_with("nas refused the login")
        );
    }

    #[test]
    fn location_kinds_follow_their_scheme() {
        assert_eq!(LocationKind::of_uri("smb://nas/x"), LocationKind::Network);
        assert_eq!(LocationKind::of_uri("mtp://Pixel/"), LocationKind::Phone);
        assert_eq!(LocationKind::of_uri("gphoto2://cam/"), LocationKind::Camera);
        assert_eq!(LocationKind::of_uri("file:///media/x"), LocationKind::Local);
        assert_eq!(LocationKind::of_uri("admin:///etc"), LocationKind::Local);
        assert_eq!(
            gvfs_fuse_root(Some(Path::new("/run/user/7"))),
            Path::new("/run/user/7/gvfs")
        );
    }
}

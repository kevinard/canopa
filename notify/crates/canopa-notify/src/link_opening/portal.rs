use super::{LinkOpeningFuture, LinkOpeningResult, NotificationLinkOpener};
use crate::NotificationLink;
use futures_util::StreamExt;
use std::{collections::HashMap, time::Duration};
use zbus::{
    Connection, MatchRule, MessageStream, Proxy,
    message::Type,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const DESKTOP: &str = "/org/freedesktop/portal/desktop";
const REQUESTS: &str = "/org/freedesktop/portal/desktop/request";
const OPEN_URI: &str = "org.freedesktop.portal.OpenURI";
const REQUEST: &str = "org.freedesktop.portal.Request";
const SETUP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct PortalNotificationLinkOpener {
    connection: Option<Connection>,
    destination: String,
    available: bool,
    response_timeout: Duration,
    cancellation: tokio::sync::watch::Sender<bool>,
}

impl PortalNotificationLinkOpener {
    pub async fn connect_to_session() -> Self {
        match tokio::time::timeout(SETUP_TIMEOUT, Connection::session()).await {
            Ok(Ok(connection)) => {
                Self::connect(connection, DESTINATION.into(), Duration::from_secs(120)).await
            }
            _ => Self {
                connection: None,
                destination: DESTINATION.into(),
                available: false,
                response_timeout: Duration::from_secs(120),
                cancellation: tokio::sync::watch::channel(false).0,
            },
        }
    }

    async fn connect(
        connection: Connection,
        destination: String,
        response_timeout: Duration,
    ) -> Self {
        let available = matches!(
            tokio::time::timeout(SETUP_TIMEOUT, owner(&connection, &destination)).await,
            Ok(Ok(_))
        );
        Self {
            connection: Some(connection),
            destination,
            available,
            response_timeout,
            cancellation: tokio::sync::watch::channel(false).0,
        }
    }

    async fn request(self, uri: NotificationLink, token: Option<String>) -> LinkOpeningResult {
        let Some(connection) = &self.connection else {
            return LinkOpeningResult::Unavailable;
        };
        let mut cancellation = self.cancellation.subscribe();
        if *cancellation.borrow() {
            return LinkOpeningResult::Unavailable;
        }
        let (owner, version) =
            match tokio::time::timeout(SETUP_TIMEOUT, owner(connection, &self.destination)).await {
                Ok(Ok(owner)) => owner,
                _ => return LinkOpeningResult::Unavailable,
            };
        let Some(token_id) = request_token() else {
            return LinkOpeningResult::Failed;
        };
        let sender = connection
            .unique_name()
            .expect("session bus assigns a unique name")
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let predicted = format!("{REQUESTS}/{sender}/{token_id}");
        let setup = async {
            let rule = MatchRule::builder()
                .msg_type(Type::Signal)
                .sender(owner.as_str())?
                .interface(REQUEST)?
                .member("Response")?
                .path_namespace(REQUESTS)?
                .build();
            let responses = MessageStream::for_match_rule(rule, connection, Some(64)).await?;
            let proxy = Proxy::new(connection, owner.as_str(), DESKTOP, OPEN_URI).await?;
            Ok::<_, zbus::Error>((responses, proxy))
        };
        let (mut responses, proxy) = match tokio::time::timeout(SETUP_TIMEOUT, setup).await {
            Ok(Ok(setup)) => setup,
            _ => return LinkOpeningResult::Failed,
        };
        if *cancellation.borrow() {
            return LinkOpeningResult::Cancelled;
        }
        let mut options = HashMap::from([("handle_token", Value::from(token_id.as_str()))]);
        if version >= 4
            && let Some(token) = token.as_deref().filter(|value| !value.is_empty())
        {
            options.insert("activation_token", Value::from(token));
        }
        let body = ("", uri.as_str(), options);
        let call = proxy.call::<_, _, OwnedObjectPath>("OpenURI", &body);
        tokio::pin!(call);
        let deadline = tokio::time::sleep(SETUP_TIMEOUT);
        tokio::pin!(deadline);
        // Consume while awaiting the reply: a full signal queue must never hold
        // up that reply. At most eight requests can be admitted by the runtime.
        let mut early = Vec::new();
        let path = loop {
            tokio::select! {
                result = &mut call => match result {
                    Ok(path) if path.as_str().starts_with(&format!("{REQUESTS}/")) => break path,
                    _ => { close(connection, &owner, &predicted).await; return LinkOpeningResult::Failed; },
                },
                signal = responses.next() => match signal {
                    Some(Ok(signal)) if early.len() < 32 => early.push(signal),
                    _ => { close(connection, &owner, &predicted).await; return LinkOpeningResult::Failed; },
                },
                _ = &mut deadline => { close(connection, &owner, &predicted).await; return LinkOpeningResult::TimedOut; },
                _ = cancellation.changed() => {
                    // A legacy portal can already own a request while its
                    // different handle is still in flight. Give the method a
                    // bounded chance to return before choosing cleanup's path.
                    let returned = tokio::time::timeout(Duration::from_secs(1), &mut call).await.ok().and_then(Result::ok)
                        .filter(|path| path.as_str().starts_with(&format!("{REQUESTS}/")));
                    close(connection, &owner, returned.as_ref().map_or(predicted.as_str(), |path| path.as_str())).await;
                    return LinkOpeningResult::Cancelled;
                },
            }
        };
        for signal in early {
            if signal
                .header()
                .path()
                .is_some_and(|candidate| candidate.as_str() == path.as_str())
            {
                return response_result(&signal);
            }
        }
        let deadline = tokio::time::sleep(self.response_timeout);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                signal = responses.next() => match signal {
                    Some(Ok(signal)) if signal.header().path().is_some_and(|candidate| candidate.as_str() == path.as_str()) => return response_result(&signal),
                    Some(Ok(_)) => {},
                    _ => { close(connection, &owner, path.as_str()).await; return LinkOpeningResult::Failed; },
                },
                _ = &mut deadline => { close(connection, &owner, path.as_str()).await; return LinkOpeningResult::TimedOut; },
                _ = cancellation.changed() => { close(connection, &owner, path.as_str()).await; return LinkOpeningResult::Cancelled; },
            }
        }
    }
}

impl NotificationLinkOpener for PortalNotificationLinkOpener {
    fn available(&self) -> bool {
        self.available && !*self.cancellation.borrow()
    }
    fn open(&self, uri: NotificationLink, token: Option<String>) -> LinkOpeningFuture {
        let opener = self.clone();
        Box::pin(async move { opener.request(uri, token).await })
    }
    fn cancel_pending(&self) {
        self.cancellation.send_replace(true);
    }
}

async fn owner(connection: &Connection, destination: &str) -> zbus::Result<(String, u32)> {
    // This property call may activate the desktop portal. Resolve and pin the
    // unique owner afterwards, then read that exact owner's interface version.
    Proxy::new(connection, destination, DESKTOP, OPEN_URI)
        .await?
        .get_property::<u32>("version")
        .await?;
    let owner = zbus::fdo::DBusProxy::new(connection)
        .await?
        .get_name_owner(destination.try_into()?)
        .await?
        .to_string();
    let version = Proxy::new(connection, owner.as_str(), DESKTOP, OPEN_URI)
        .await?
        .get_property::<u32>("version")
        .await?;
    Ok((owner, version))
}

fn response_result(signal: &zbus::Message) -> LinkOpeningResult {
    match signal
        .body()
        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
    {
        Ok((0, _)) => LinkOpeningResult::Opened,
        Ok((1, _)) => LinkOpeningResult::Cancelled,
        _ => LinkOpeningResult::Failed,
    }
}

async fn close(connection: &Connection, owner: &str, path: &str) {
    let cleanup = async {
        Proxy::new(connection, owner, path, REQUEST)
            .await?
            .call::<_, _, ()>("Close", &())
            .await
    };
    let _ = tokio::time::timeout(Duration::from_secs(1), cleanup).await;
}

fn request_token() -> Option<String> {
    let mut bytes = [0u8; 16];
    let mut remaining = bytes.as_mut_slice();
    while !remaining.is_empty() {
        match rustix::rand::getrandom(&mut *remaining, rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => return None,
            Ok(count) => remaining = &mut remaining[count..],
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return None,
        }
    }
    Some(format!(
        "canopa_{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

#[cfg(test)]
mod tests;

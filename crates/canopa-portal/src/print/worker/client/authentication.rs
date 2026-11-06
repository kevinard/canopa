use crate::print::{
    authentication::{Broker, Challenge, Credentials, Kind, MAX_EXCHANGES, MAX_FRAME},
    worker::Operation,
};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
};

pub(super) async fn serve(
    mut channel: tokio::net::UnixStream,
    broker: Arc<Broker>,
    operation: &Operation,
    cancelled: watch::Receiver<bool>,
    interactive: bool,
    extended: bool,
) -> anyhow::Result<()> {
    let mut next = 1;
    loop {
        let mut length = [0; 4];
        if channel.read(&mut length[..1]).await? == 0 {
            return Ok(());
        }
        channel.read_exact(&mut length[1..]).await?;
        let length = u32::from_be_bytes(length) as usize;
        anyhow::ensure!(
            length > 0 && length <= MAX_FRAME && next <= MAX_EXCHANGES,
            "Printer authentication challenge exceeds limits"
        );
        let mut bytes = vec![0; length];
        channel.read_exact(&mut bytes).await?;
        let mut challenge: Challenge = serde_json::from_slice(&bytes)?;
        challenge.validate()?;
        if let Kind::Server { limits, .. } = challenge.kind {
            anyhow::ensure!(
                extended || limits == crate::print::authentication::ServerLimits::Legacy,
                "Unverified printer credential capacity"
            );
        }
        anyhow::ensure!(
            challenge.id == next,
            "Unexpected printer authentication challenge"
        );
        next += 1;
        if let Kind::Queue {
            destination,
            fields,
        } = &challenge.kind
        {
            anyhow::ensure!(
                matches!(operation, Operation::Submit { destination: expected, .. } if expected == destination)
                    || matches!(operation, Operation::AuthenticateHeldJob { job } if job.binding.as_ref().is_some_and(|binding| &binding.destination == destination && matches!(&binding.requirement, crate::print::authentication::Requirement::Fields { fields: expected } if fields == expected))),
                "Authentication requested for another printer"
            );
        }
        challenge.interactive &= interactive;
        let credentials = broker.request(challenge.clone(), cancelled.clone()).await?;
        let reply = Credentials::encode(&challenge, credentials.as_ref())?;
        channel
            .write_all(&(reply.bytes().len() as u32).to_be_bytes())
            .await?;
        channel.write_all(reply.bytes()).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        print::authentication::{Endpoint, ServerLimits, Transport},
        process::DialogProcess,
    };
    #[tokio::test]
    async fn system_worker_cannot_raise_limits_before_parent_approval() {
        let (mut child, parent) = tokio::net::UnixStream::pair().unwrap();
        let challenge = Challenge {
            version: 1,
            interactive: true,
            id: 1,
            attempt: 1,
            endpoint: Endpoint {
                server: "localhost".into(),
                port: 631,
                transport: Transport::Loopback,
            },
            kind: Kind::Server {
                realm: "Office".into(),
                resource: "/".into(),
                limits: ServerLimits::CanopaV1,
            },
            suggested_user: "user".into(),
        };
        let bytes = serde_json::to_vec(&challenge).unwrap();
        child
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .await
            .unwrap();
        child.write_all(&bytes).await.unwrap();
        let broker = Broker::new(
            DialogProcess {
                executable: "/must-not-execute".into(),
                arguments: vec![],
            },
            crate::print::Context {
                app_id: String::new(),
                parent_window: String::new(),
                title: String::new(),
                modal: false,
            },
        );
        let (_cancel, cancelled) = watch::channel(false);
        let error = serve(parent, broker, &Operation::Printers, cancelled, true, false)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Unverified printer credential capacity")
        );
    }
}

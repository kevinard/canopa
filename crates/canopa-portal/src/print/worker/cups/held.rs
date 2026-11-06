//! Authenticate a freshly checked existing document; never create, send or cancel.
use super::*;
use crate::print::{
    authentication::Requirement,
    live_jobs::{Binding, Job, MAX_JOBS, Status, held_requirement},
};

fn queue(metadata: &identity::Metadata, destination: &Destination) -> anyhow::Result<()> {
    let uri = url::Url::parse(&metadata.queue)?;
    let name = uri
        .path()
        .strip_prefix("/printers/")
        .or_else(|| uri.path().strip_prefix("/classes/"));
    anyhow::ensure!(
        matches!(uri.scheme(), "ipp" | "ipps" | "http" | "https")
            && uri.host_str().is_some()
            && uri.username().is_empty()
            && uri.password().is_none()
            && uri.query().is_none()
            && uri.fragment().is_none()
            && name.is_some_and(|name| recovery::queue_matches(
                name.as_bytes(),
                destination.queue.as_bytes()
            )),
        "Printer job moved to another queue"
    );
    Ok(())
}
fn title(name: Option<&str>) -> String {
    let name = name.unwrap_or("Printed document");
    let name = name
        .strip_prefix("[Canopa:")
        .and_then(|value| value.split_once("] "))
        .map_or(name, |(_, title)| title);
    let mut result = String::new();
    for character in name.chars().filter(|character| !character.is_control()) {
        if result.len() + character.len_utf8() > 255 {
            break;
        }
        result.push(character);
    }
    if result.is_empty() {
        "Printed document".into()
    } else {
        result
    }
}
fn describe(
    metadata: identity::Metadata,
    session: &Session,
    destination: &Destination,
    requirement: &Requirement,
) -> anyhow::Result<Job> {
    queue(&metadata, destination)?;
    anyhow::ensure!(
        metadata.id > 0 && metadata.documents.is_none_or(|n| n >= 0),
        "Invalid printer job"
    );
    let status = match metadata.state {
        3 => Status::Queued,
        4 if metadata.incoming && metadata.documents == Some(0) => Status::Receiving,
        4 if metadata.held_for_authentication && metadata.documents.is_some_and(|n| n > 0) => {
            Status::NeedsAuthentication
        }
        4 => Status::Held,
        5 => Status::Processing,
        6 => Status::Stopped,
        7 => Status::Cancelled,
        8 => Status::Aborted,
        9 => Status::Completed,
        _ => anyhow::bail!("Invalid printer job state"),
    };
    let binding = match (&metadata.name, &metadata.owner, &metadata.uuid) {
        (Some(name), Some(owner), Some(uuid)) => {
            let mut server = session.api.identity()?;
            // This user is immutable source ownership, not the HTTP actor that
            // may later be approved for this action.
            server.user = owner.as_bytes().to_vec();
            let binding = Binding {
                server,
                destination: destination.clone(),
                job: metadata.id,
                name: name.clone(),
                uuid: uuid.clone(),
                protection: session.authentication.protection(),
                requirement: if status == Status::NeedsAuthentication {
                    held_requirement(requirement.clone())
                } else {
                    requirement.clone()
                },
            };
            binding.validate().ok().map(|()| binding)
        }
        _ => None,
    };
    let job = Job {
        job: metadata.id,
        title: title(metadata.name.as_deref()),
        status,
        binding,
    };
    job.validate()?;
    Ok(job)
}
pub(in crate::print::worker) fn list(
    destination: Destination,
    cancel: &Cancellation,
) -> anyhow::Result<Vec<Job>> {
    cancel.check()?;
    let session = Session::open()?;
    let selected = session.destination(&destination, true)?;
    let server = session.api.identity()?;
    let query = identity::Query::new(&selected, &server)?;
    let requirement = query.requirement(&session)?;
    let metadata = query.active(&session)?;
    anyhow::ensure!(
        metadata.len() <= MAX_JOBS,
        "Printer job listing exceeds limits"
    );
    let mut ids = std::collections::HashSet::new();
    let mut result = Vec::new();
    for metadata in metadata {
        cancel.check()?;
        anyhow::ensure!(ids.insert(metadata.id), "Duplicate live printer job");
        result.push(describe(metadata, &session, &destination, &requirement)?);
    }
    cancel.check()?;
    Ok(result)
}
fn profile(api: &Api, binding: &Binding) -> anyhow::Result<()> {
    let current = api.identity()?;
    anyhow::ensure!(
        current.name == binding.server.name
            && current.port == binding.server.port
            && current.encryption == binding.server.encryption,
        "Configured print server changed"
    );
    Ok(())
}
fn verify(metadata: &identity::Metadata, binding: &Binding) -> anyhow::Result<()> {
    queue(metadata, &binding.destination)?;
    anyhow::ensure!(
        metadata.id == binding.job
            && metadata.uuid.as_ref() == Some(&binding.uuid)
            && metadata
                .name
                .as_ref()
                .is_none_or(|name| name == &binding.name)
            && metadata
                .owner
                .as_ref()
                .is_none_or(|owner| owner.as_bytes() == binding.server.user)
            && metadata.state == 4
            && metadata.held_for_authentication
            && metadata.documents.is_some_and(|n| n > 0),
        "Selected printer job changed or is no longer held for sign-in"
    );
    Ok(())
}

pub(in crate::print::worker) fn authenticate(
    job: Job,
    cancel: &Cancellation,
) -> anyhow::Result<()> {
    job.validate()?;
    anyhow::ensure!(
        job.can_authenticate(),
        "Printer job is unavailable for sign-in"
    );
    let binding = job.binding.as_ref().expect("validated sign-in binding");
    let Requirement::Fields { fields } = &binding.requirement else {
        unreachable!()
    };
    cancel.check()?;
    profile(&Api::load()?, binding)?;
    // Preserve the source connection's actual TLS floor before any new metadata
    // request or HTTP credential callback can occur.
    let mut session = Session::open_protected(Some(binding.protection))?;
    let guard = connection::Connections::new()?;
    let mut credentials = None;
    for attempt in 0..crate::print::authentication::MAX_ATTEMPTS {
        cancel.check()?;
        profile(&session.api, binding)?;
        session.authentication.activate();
        let selected = session.destination(&binding.destination, true)?;
        let context = identity::Context::new(&selected, binding.job, &binding.server)?;
        verify(
            &context.inspect(&session, Some(&binding.server.user))?,
            binding,
        )?;
        anyhow::ensure!(
            session.authentication.protection() >= binding.protection
                && held_requirement(context.requirement(&session)?) == binding.requirement,
            "Printer protection or sign-in requirements changed"
        );
        if credentials.is_none() {
            credentials = Some(
                session
                    .authentication
                    .queue_credentials(&binding.destination, fields.clone())?,
            );
        }
        // A user prompt is outside the mutation guard. Re-read source identity
        // and ordered requirements after it, on the actual transmission session.
        cancel.check()?;
        drop(selected);
        let selected = session.destination(&binding.destination, true)?;
        let context = identity::Context::new(&selected, binding.job, &binding.server)?;
        anyhow::ensure!(
            session.authentication.protection() >= binding.protection
                && held_requirement(context.requirement(&session)?) == binding.requirement,
            "Printer sign-in requirements changed"
        );
        // This is the last network query before mutation. Any HTTP login or
        // reconnect during requirement discovery precedes this fresh job proof.
        verify(
            &context.inspect(&session, Some(&binding.server.user))?,
            binding,
        )?;
        let deferred = session.authentication.defer()?;
        unsafe {
            (session.api.protocol.set_header)(session.http.as_ptr(), 26, c"".as_ptr());
        }
        let (acknowledged, changed) = guard.checked(|| {
            cancel.check()?;
            context.authenticate(&session, credentials.as_ref().expect("approved fields"))
        })?;
        let resource = deferred.resource();
        drop(deferred);
        if acknowledged.is_ok() && !changed {
            return Ok(());
        }
        // Metadata200 can reset CUPS's cached-password retry count. On a new
        // rejection it can reuse that password without calling our callback,
        // then overwrite HTTP401 while the guard refuses its reconnect. The
        // operation's freshly received challenge survives that failed reconnect;
        // the header was cleared before this checked action, as in recovery.
        let challenge =
            discovery::string(unsafe { (session.api.protocol.header)(session.http.as_ptr(), 26) })?
                .filter(|value| !value.is_empty());
        let unauthorized = resource.is_some()
            || unsafe { (session.api.http_status)(session.http.as_ptr()) } == 401
            || (changed && challenge.is_some());
        if !unauthorized {
            anyhow::bail!(
                "Printer sign-in outcome is unconfirmed; refresh the job before another attempt"
            );
        }
        anyhow::ensure!(
            attempt + 1 < crate::print::authentication::MAX_ATTEMPTS,
            "Printer sign-in attempts exhausted"
        );
        let challenge =
            challenge.ok_or_else(|| anyhow::anyhow!("Printer sign-in challenge unavailable"))?;
        anyhow::ensure!(
            resource
                .as_ref()
                .is_none_or(|resource| resource.as_bytes() == context.resource().to_bytes()),
            "Printer sign-in resource changed"
        );
        let next = authentication::continue_session(&session, context.resource(), &challenge)?;
        drop(selected);
        session = next;
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (identity::Metadata, Binding) {
        let uuid = "urn:uuid:01234567-89ab-cdef-0123-456789abcdef".to_owned();
        let metadata = identity::Metadata {
            id: 7,
            name: Some("Accepted document".into()),
            owner: Some("owner".into()),
            queue: "ipp://localhost/printers/Fixture".into(),
            uuid: Some(uuid.clone()),
            state: 4,
            documents: Some(1),
            incoming: false,
            held_for_authentication: true,
        };
        let binding = Binding {
            server: crate::print::printer_jobs::Server {
                name: b"localhost".to_vec(),
                port: 631,
                encryption: 0,
                user: b"owner".to_vec(),
            },
            destination: Destination {
                queue: "Fixture".into(),
                instance: None,
            },
            job: 7,
            name: "Accepted document".into(),
            uuid,
            protection: 0,
            requirement: held_requirement(Requirement::None),
        };
        (metadata, binding)
    }
    #[test]
    fn sign_in_requires_the_same_received_held_job_after_credentials() {
        let (mut metadata, binding) = fixture();
        binding.validate().unwrap();
        verify(&metadata, &binding).unwrap();
        metadata.name = None;
        metadata.owner = None;
        verify(&metadata, &binding).unwrap();
        metadata.uuid = Some("urn:uuid:ffffffff-ffff-ffff-ffff-ffffffffffff".into());
        assert!(verify(&metadata, &binding).is_err());
        metadata.uuid = Some(binding.uuid.clone());
        metadata.owner = Some("another-account".into());
        assert!(verify(&metadata, &binding).is_err());
        metadata.owner = None;
        metadata.queue = "ipp://localhost/printers/Fixture%2Fother".into();
        assert!(verify(&metadata, &binding).is_err());
        metadata.queue = "ipp://localhost/printers/Fixture".into();
        metadata.id = 8;
        assert!(verify(&metadata, &binding).is_err());
    }
    #[test]
    fn received_identity_never_authorizes_sign_in_for_an_empty_or_unheld_job() {
        let (mut metadata, binding) = fixture();
        for state in [3, 5, 6, 7, 8, 9] {
            metadata.state = state;
            assert!(verify(&metadata, &binding).is_err());
        }
        metadata.state = 4;
        for documents in [None, Some(0)] {
            metadata.documents = documents;
            assert!(verify(&metadata, &binding).is_err());
        }
        metadata.documents = Some(1);
        metadata.held_for_authentication = false;
        assert!(verify(&metadata, &binding).is_err());
    }
    #[test]
    fn late_default_does_not_replace_explicit_ordered_or_negotiate_requirements() {
        use crate::print::authentication::Field;
        assert_eq!(
            held_requirement(Requirement::None),
            Requirement::Fields {
                fields: vec![Field::Username, Field::Password]
            }
        );
        for requirement in [
            Requirement::Negotiate,
            Requirement::Fields {
                fields: vec![Field::Password, Field::Domain, Field::Username],
            },
        ] {
            assert_eq!(held_requirement(requirement.clone()), requirement);
        }
        let (_, binding) = fixture();
        let mut job = Job {
            job: 7,
            title: "Document".into(),
            status: Status::Queued,
            binding: Some(binding),
        };
        assert!(!job.can_authenticate());
        job.status = Status::NeedsAuthentication;
        assert!(job.can_authenticate());
        job.binding = None;
        assert!(!job.can_authenticate());
    }
}

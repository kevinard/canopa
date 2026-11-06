//! A rejected document can continue only on its checked existing job. Lost
//! acknowledgements never enter this path. CUPS still owns HTTP authentication.
use super::super::Document;
use super::protocol::Message;
use super::*;
use std::os::unix::fs::FileExt;

enum Outcome {
    Accepted,
    Rejected,
    BeforeDocumentReconnect,
}

fn finish(session: &Session, cancel: &Cancellation) -> anyhow::Result<Outcome> {
    let functions = session.api.protocol;
    let http = session.http.as_ptr();
    // StartDestDocument uses CUPS_LENGTH_VARIABLE: terminate its chunked body.
    // GetResponse would authenticate/reconnect here and erase the rejection
    // before the caller can validate the existing job. Read public HTTP/IPP
    // results directly, without that automatic continuation.
    anyhow::ensure!(
        unsafe { (functions.write)(http, c"".as_ptr(), 0) } >= 0,
        "Final print request could not be finished; do not retry automatically"
    );
    let status = loop {
        cancel.check()?;
        let status = unsafe { (functions.update)(http) };
        if status != 100 {
            break status;
        }
    };
    if status == 401 {
        unsafe {
            (functions.flush)(http);
        }
        return Ok(Outcome::Rejected);
    }
    anyhow::ensure!(
        status == 200,
        "Final print acceptance was not acknowledged; do not retry automatically"
    );
    let response = Message::new(unsafe { (functions.new)() }, functions)?;
    loop {
        cancel.check()?;
        match unsafe { (functions.read)(http, response.raw) } {
            3 => break, // IPP_STATE_DATA
            -1 => anyhow::bail!("Invalid final printer response; do not retry automatically"),
            _ => {}
        }
    }
    anyhow::ensure!(
        succeeded(unsafe { (functions.status)(response.raw) }),
        "Printer rejected the document"
    );
    Ok(Outcome::Accepted)
}

fn attempt(
    file: &File,
    document: &Document,
    selected: &Selected<'_>,
    job: i32,
    cancel: &Cancellation,
    guard: &connection::Connections,
) -> anyhow::Result<Outcome> {
    let session = selected.session;
    let http = session.http.as_ptr();
    let deferred = session.authentication.defer()?;
    let mut began = false;
    let (result, changed) = guard.checked(|| -> anyhow::Result<Outcome> {
        cancel.check()?;
        let mime = CString::new(
            document
                .format
                .map_or("application/octet-stream", crate::print::Format::mime),
        )?;
        let status = unsafe {
            (session.api.start)(
                http,
                selected.raw.as_ptr(),
                selected.info.expect("requested capabilities").as_ptr(),
                job,
                c"document".as_ptr(),
                mime.as_ptr(),
                0,
                ptr::null_mut(),
                1,
            )
        };
        if status == 401 || deferred.rejected() {
            return Ok(Outcome::Rejected);
        }
        anyhow::ensure!(status == 100, "Printer did not accept document streaming");
        let mut buffer = [0; 64 * 1024];
        let mut bytes = 0;
        loop {
            cancel.check()?;
            let read = file.read_at(&mut buffer, bytes)?;
            if read == 0 {
                break;
            }
            // A failed write can still send a prefix. Only an explicit401 may
            // continue after this point, never a lost connection/acknowledgement.
            began = true;
            let status = unsafe { (session.api.write)(http, buffer.as_ptr().cast(), read) };
            if status == 401 || deferred.rejected() {
                return Ok(Outcome::Rejected);
            }
            anyhow::ensure!(
                status == 100,
                "Print document streaming failed; do not retry automatically"
            );
            bytes += read as u64;
        }
        anyhow::ensure!(bytes == document.bytes, "Print snapshot length changed");
        finish(session, cancel)
    })?;
    if changed {
        anyhow::ensure!(
            !began,
            "Printer connection changed after document streaming; do not retry automatically"
        );
        return Ok(Outcome::BeforeDocumentReconnect);
    }
    result
}

pub(super) fn send(
    file: &File,
    document: &Document,
    selected: &Selected<'_>,
    options: &JobOptions,
    cancel: &Cancellation,
    job: &mut PartialJob,
) -> anyhow::Result<()> {
    if job.connections.is_none() {
        job.connections = Some(connection::Connections::new()?);
    }
    let guard = job
        .connections
        .as_ref()
        .expect("initialized connection guard");
    let session = selected.session;
    let mut outcome = attempt(file, document, selected, job.id, cancel, guard)?;
    if matches!(outcome, Outcome::Accepted) {
        return Ok(());
    }
    let intent = job.journal.intent();
    let context = identity::Context::new(selected, job.id, &intent.server)?;
    // Inspect as the creation owner before changing HTTP principals. Keep the
    // rejected HTTP object's actual challenge intact on this separate session.
    let inspection = (|| {
        let query = Session::open_protected(Some(session.authentication.protection()))?;
        context
            .inspect(&query, Some(&intent.server.user))?
            .capture(&intent, job.id)
    })();
    // Session callbacks are thread-local. The sibling's destructor installed
    // refusal, so restore this retained callback before any further CUPS IO.
    session.authentication.activate();
    let proof = inspection?;
    job.journal.identified(job.id, proof.uuid.as_deref())?;
    let mut authenticated = Vec::new();
    for _ in 0..2 {
        cancel.check()?;
        let previous = authenticated.last().unwrap_or(session);
        // This is challenge metadata, never Authorization/password data. A new
        // HTTP object's empty CUPS password cache forces the broker to observe
        // this rejection. Metadata200 can otherwise reset libcups's retry count
        // and make it reuse the wrong password indefinitely across attempts.
        let challenge = discovery::string(unsafe {
            (previous.api.protocol.header)(previous.http.as_ptr(), 26)
        })?
        .filter(|value| !value.is_empty());
        if let Some(challenge) = challenge {
            authenticated.push(authentication::continue_session(
                previous,
                context.resource(),
                &challenge,
            )?);
        } else {
            anyhow::ensure!(
                !matches!(outcome, Outcome::Rejected),
                "Printer authentication challenge missing"
            );
        }
        let session = authenticated.last().unwrap_or(session);
        session.authentication.activate();
        let current = session.destination(&intent.destination, true)?;
        validation::selected(&current, options)?;
        context
            .inspect(session, None)?
            .revalidate(&intent, job.id, &proof)?;
        outcome = attempt(file, document, &current, job.id, cancel, guard)?;
        if matches!(outcome, Outcome::Accepted) {
            return Ok(());
        }
    }
    anyhow::bail!(
        "Printer authentication or connection kept changing; document not retried further"
    )
}

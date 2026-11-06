//! Configured-peer job queries keep the recorded owner separate from the HTTP
//! principal. Only previously bound UUIDs can authorize privacy-hidden identity.
use super::protocol::Message;
use super::*;
use crate::print::printer_jobs::{Intent, Receipt, Server, valid_uuid};
use std::{collections::HashMap, ffi::CStr};

pub(super) struct Query {
    resource: CString,
    uri: CString,
    version: (c_int, c_int),
    server: Server,
}
pub(super) struct Context {
    query: Query,
    job_uri: CString,
}
pub(super) struct Proof {
    pub uuid: Option<String>,
}
pub(super) struct Metadata {
    pub(super) id: i32,
    pub(super) name: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) queue: String,
    pub(super) uuid: Option<String>,
    pub(super) state: i32,
    pub(super) documents: Option<i32>,
    pub(super) incoming: bool,
    pub(super) held_for_authentication: bool,
}
impl Query {
    pub fn new(selected: &Selected<'_>, server: &Server) -> anyhow::Result<Self> {
        let uri = discovery::metadata(selected, c"printer-uri-supported")?
            .ok_or_else(|| anyhow::anyhow!("Printer identity URI unavailable"))?;
        let parsed = url::Url::parse(&uri)?;
        anyhow::ensure!(
            matches!(parsed.scheme(), "ipp" | "ipps" | "http" | "https")
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                && parsed.path().len() <= 1024,
            "Invalid printer identity URI"
        );
        let resource = CString::new(parsed.path())?;
        let versions = discovery::versions(selected)?;
        let version = if versions
            .iter()
            .any(|v| matches!(v.as_str(), "2.0" | "2.1" | "2.2"))
        {
            (2, 0)
        } else {
            (1, 1)
        };
        Ok(Self {
            resource,
            uri: CString::new(uri)?,
            version,
            server: server.clone(),
        })
    }
    fn check_endpoint(&self, session: &Session) -> anyhow::Result<()> {
        let current = session.api.identity()?;
        anyhow::ensure!(
            current.name == self.server.name
                && current.port == self.server.port
                && current.encryption == self.server.encryption,
            "Print server changed during authentication"
        );
        Ok(())
    }
    fn request(
        &self,
        session: &Session,
        operation: c_int,
        owner: Option<&[u8]>,
        job: Option<&CStr>,
    ) -> anyhow::Result<Message> {
        self.request_with(session, operation, owner, job, false, None)
    }
    fn request_with(
        &self,
        session: &Session,
        operation: c_int,
        owner: Option<&[u8]>,
        job: Option<&CStr>,
        active: bool,
        credentials: Option<&crate::print::authentication::Credentials>,
    ) -> anyhow::Result<Message> {
        self.check_endpoint(session)?;
        let api = &session.api;
        let f = api.protocol;
        let request = Message::new(unsafe { (f.request)(operation) }, f)?;
        let current_user = unsafe { (api.user)() };
        anyhow::ensure!(!current_user.is_null(), "Printer account unavailable");
        let owner = owner.map(CString::new).transpose()?;
        let user = owner.as_ref().map_or(current_user, |value| value.as_ptr());
        let attributes = [
            c"job-id",
            c"job-name",
            c"job-originating-user-name",
            c"job-printer-uri",
            c"job-uuid",
            c"job-state",
            c"job-state-reasons",
            c"number-of-documents",
        ];
        let job_names = attributes.map(CStr::as_ptr);
        let printer_names = [c"auth-info-required".as_ptr()];
        let names: &[*const c_char] = if operation == 11 {
            &printer_names
        } else {
            &job_names
        };
        // Public constructors copy retained strings. The configured HTTP object
        // carries every request; an IPP URI never chooses another connection.
        unsafe {
            anyhow::ensure!(
                (f.version)(request.raw, self.version.0, self.version.1) != 0,
                "Printer identity version unavailable"
            );
            for (tag, name, value) in [
                (
                    0x45,
                    if job.is_some() {
                        c"job-uri"
                    } else {
                        c"printer-uri"
                    },
                    job.map_or(self.uri.as_ptr(), CStr::as_ptr),
                ),
                (0x42, c"requesting-user-name", user),
            ] {
                anyhow::ensure!(
                    !(f.string)(request.raw, 1, tag, name.as_ptr(), ptr::null(), value).is_null(),
                    "Cannot construct printer identity request"
                );
            }
            if operation == 10 {
                anyhow::ensure!(
                    !(f.boolean)(
                        request.raw,
                        1,
                        c"my-jobs".as_ptr(),
                        if active { 1 } else { 0 }
                    )
                    .is_null(),
                    "Cannot request printer history"
                );
                anyhow::ensure!(
                    !(f.string)(
                        request.raw,
                        1,
                        0x44,
                        c"which-jobs".as_ptr(),
                        ptr::null(),
                        if active {
                            c"not-completed".as_ptr()
                        } else {
                            c"all".as_ptr()
                        }
                    )
                    .is_null(),
                    "Cannot request printer history"
                );
            }
            if operation != 8 && operation != 0x400e {
                anyhow::ensure!(
                    !(f.strings)(
                        request.raw,
                        1,
                        0x44,
                        c"requested-attributes".as_ptr(),
                        names.len() as c_int,
                        ptr::null(),
                        names.as_ptr()
                    )
                    .is_null(),
                    "Cannot request printer identity"
                );
            }
        }
        let values = credentials
            .map(|credentials| {
                credentials
                    .values
                    .iter()
                    .map(|value| {
                        crate::print::authentication::Secret::copy(value.bytes()).c_string()
                    })
                    .collect::<anyhow::Result<Vec<_>>>()
            })
            .transpose()?;
        if let Some(values) = &values {
            let pointers = values
                .iter()
                .map(|value| value.bytes().as_ptr().cast::<c_char>())
                .collect::<Vec<_>>();
            // The public IPP constructor copies ordered literal TEXT fields.
            // Our temporary secret copies are cleared on return.
            anyhow::ensure!(
                !unsafe {
                    (f.strings)(
                        request.raw,
                        2,
                        0x41,
                        c"auth-info".as_ptr(),
                        pointers.len() as c_int,
                        ptr::null(),
                        pointers.as_ptr(),
                    )
                }
                .is_null(),
                "Cannot construct printer job authentication"
            );
        }
        let response = Message::new(
            unsafe {
                (f.perform)(
                    session.http.as_ptr(),
                    request.take(),
                    self.resource.as_ptr(),
                )
            },
            f,
        )?;
        self.check_endpoint(session)?;
        anyhow::ensure!(
            unsafe { (api.http_status)(session.http.as_ptr()) } == 200
                && succeeded(unsafe { (f.status)(response.raw) }),
            "Printer identity was not acknowledged"
        );
        Ok(response)
    }
    fn jobs(
        &self,
        session: &Session,
        owner: Option<&[u8]>,
        active: bool,
    ) -> anyhow::Result<Vec<Metadata>> {
        let response = self.request_with(session, 10, owner, None, active, None)?;
        let api = &session.api;
        let f = api.protocol;
        let mut members = HashMap::new();
        let mut jobs = vec![];
        let mut count = 0;
        let mut raw = unsafe { (f.first)(response.raw) };
        while !raw.is_null() {
            count += 1;
            anyhow::ensure!(count <= 4096 * 32, "Too many printer job attributes");
            let group = unsafe { (f.group)(raw) };
            let name = discovery::string(unsafe { (f.name)(raw) })?;
            if group != 2 || name.is_none() {
                take_job(api, &response, &mut members, &mut jobs)?;
            } else if let Some(name) = name
                && matches!(
                    name.as_str(),
                    "job-id"
                        | "job-name"
                        | "job-originating-user-name"
                        | "job-printer-uri"
                        | "job-uuid"
                        | "job-state"
                        | "job-state-reasons"
                        | "number-of-documents"
                )
            {
                anyhow::ensure!(
                    members.insert(name, raw).is_none(),
                    "Duplicate printer job attribute"
                );
            }
            raw = unsafe { (f.next)(response.raw) };
        }
        take_job(api, &response, &mut members, &mut jobs)?;
        Ok(jobs)
    }
    pub fn active(&self, session: &Session) -> anyhow::Result<Vec<Metadata>> {
        self.jobs(session, None, true)
    }
    pub fn requirement(
        &self,
        session: &Session,
    ) -> anyhow::Result<crate::print::authentication::Requirement> {
        let response = self.request(session, 11, None, None)?;
        let api = &session.api;
        let raw = unsafe { (api.protocol.find)(response.raw, c"auth-info-required".as_ptr(), 0) };
        if raw.is_null() {
            return Ok(crate::print::authentication::Requirement::None);
        }
        let count = unsafe { (api.attribute_count)(raw) };
        anyhow::ensure!(
            unsafe { (api.protocol.group)(raw) } == 4
                && unsafe { (api.attribute_tag)(raw) } == 0x44
                && (1..=3).contains(&count),
            "Invalid live printer sign-in requirements"
        );
        let mut fields = Vec::new();
        for index in 0..count {
            fields.push(
                discovery::string(unsafe { (api.attribute_string)(raw, index, ptr::null_mut()) })?
                    .ok_or_else(|| anyhow::anyhow!("Printer sign-in requirement unavailable"))?,
            );
        }
        crate::print::authentication::Requirement::parse(Some(&fields.join(",")))
    }
    pub fn owned(&self, session: &Session, receipt: &Receipt) -> anyhow::Result<Vec<Receipt>> {
        let intent = &receipt.intent;
        // An immutable owner claim affects visibility, never HTTP credentials.
        let jobs = self.jobs(session, Some(&intent.server.user), false)?;
        let mut owned = vec![];
        for job in jobs {
            if job.name.as_deref() == Some(intent.name.as_str())
                && job
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.as_bytes() == intent.server.user)
            {
                job.matches_job(intent, job.id)?;
                let mut bound = receipt.clone();
                bound.job = Some(job.id);
                bound.uuid = job.uuid.clone();
                bound.validate()?;
                owned.push(bound);
            }
        }
        anyhow::ensure!(
            owned.len() <= crate::print::printer_jobs::MAX_JOBS,
            "Too many jobs share a printer recovery identity"
        );
        let mut ids = std::collections::HashSet::new();
        anyhow::ensure!(
            owned.iter().all(|receipt| ids.insert(receipt.job)),
            "Duplicate printer job identity"
        );
        Ok(owned)
    }
}
fn take_job(
    api: &Api,
    response: &Message,
    members: &mut HashMap<String, *mut c_void>,
    jobs: &mut Vec<Metadata>,
) -> anyhow::Result<()> {
    if !members.is_empty() {
        anyhow::ensure!(jobs.len() < 4096, "Too many printer jobs");
        jobs.push(Metadata::read(Attributes {
            api,
            response,
            members: Some(members),
        })?);
        members.clear();
    }
    Ok(())
}
impl Context {
    pub fn new(selected: &Selected<'_>, job: i32, server: &Server) -> anyhow::Result<Self> {
        let query = Query::new(selected, server)?;
        let mut uri = url::Url::parse(query.uri.to_str()?)?;
        uri.set_path(&format!("/jobs/{job}"));
        Ok(Self {
            query,
            job_uri: CString::new(uri.as_str())?,
        })
    }
    pub fn authenticate(
        &self,
        session: &Session,
        credentials: &crate::print::authentication::Credentials,
    ) -> anyhow::Result<()> {
        self.query.request_with(
            session,
            0x400e,
            None,
            Some(&self.job_uri),
            false,
            Some(credentials),
        )?;
        Ok(())
    }
    pub fn cancel(&self, session: &Session) -> anyhow::Result<()> {
        // An owned public IPP request keeps the challenged resource available
        // even when CUPS masks HTTP401 with a failed authentication reconnect.
        self.query.request(session, 8, None, Some(&self.job_uri))?;
        Ok(())
    }
    pub fn resource(&self) -> &CStr {
        &self.query.resource
    }
    pub fn requirement(
        &self,
        session: &Session,
    ) -> anyhow::Result<crate::print::authentication::Requirement> {
        self.query.requirement(session)
    }
    pub fn inspect(&self, session: &Session, owner: Option<&[u8]>) -> anyhow::Result<Metadata> {
        let response = self.query.request(session, 9, owner, Some(&self.job_uri))?;
        Metadata::read(Attributes {
            api: &session.api,
            response: &response,
            members: None,
        })
    }
}
impl Metadata {
    fn read(values: Attributes<'_>) -> anyhow::Result<Self> {
        let uuid = values.text(c"job-uuid", 0x45)?;
        anyhow::ensure!(
            uuid.as_ref().is_none_or(|uuid| valid_uuid(uuid)),
            "Invalid printer job UUID"
        );
        Ok(Self {
            id: values.integer(c"job-id", 0x21)?,
            name: values.text(c"job-name", 0x42)?,
            owner: values.text(c"job-originating-user-name", 0x42)?,
            queue: values
                .text(c"job-printer-uri", 0x45)?
                .ok_or_else(|| anyhow::anyhow!("Printer job queue unavailable"))?,
            uuid: uuid.map(|uuid| uuid.to_ascii_lowercase()),
            state: values.integer(c"job-state", 0x23)?,
            documents: values.optional_integer(c"number-of-documents", 0x21)?,
            incoming: values.contains(c"job-state-reasons", "job-incoming")?,
            held_for_authentication: values
                .contains(c"job-state-reasons", "cups-held-for-authentication")?,
        })
    }

    fn matches_job(&self, intent: &Intent, id: i32) -> anyhow::Result<()> {
        let uri = url::Url::parse(&self.queue)?;
        let queue = uri
            .path()
            .strip_prefix("/printers/")
            .or_else(|| uri.path().strip_prefix("/classes/"));
        anyhow::ensure!(
            self.id == id
                && id > 0
                && (3..=9).contains(&self.state)
                && self.documents.is_none_or(|count| count >= 0)
                && matches!(uri.scheme(), "ipp" | "ipps" | "http" | "https")
                && uri.host_str().is_some()
                && uri.username().is_empty()
                && uri.password().is_none()
                && uri.query().is_none()
                && uri.fragment().is_none()
                && queue.is_some_and(|queue| recovery::queue_matches(
                    queue.as_bytes(),
                    intent.destination.queue.as_bytes()
                )),
            "Printer job identity does not match"
        );
        Ok(())
    }
    fn pending(&self, intent: &Intent, id: i32) -> anyhow::Result<()> {
        self.matches_job(intent, id)?;
        anyhow::ensure!(
            self.state == 4 && self.documents == Some(0) && self.incoming,
            "Printer job is no longer the empty incoming job"
        );
        Ok(())
    }
    pub fn verify_receipt(&self, receipt: &Receipt, id: i32) -> anyhow::Result<i32> {
        self.matches_job(&receipt.intent, id)?;
        self.identity(
            &receipt.intent,
            &Proof {
                uuid: receipt.uuid.clone(),
            },
        )?;
        Ok(self.state)
    }
    pub fn verify_incomplete(&self, receipt: &Receipt, id: i32) -> anyhow::Result<()> {
        anyhow::ensure!(!receipt.accepted, "Printer job was already acknowledged");
        self.verify_receipt(receipt, id)?;
        self.pending(&receipt.intent, id)
    }
    pub fn capture(&self, intent: &Intent, id: i32) -> anyhow::Result<Proof> {
        self.pending(intent, id)?;
        anyhow::ensure!(
            self.name.as_deref() == Some(intent.name.as_str())
                && self
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.as_bytes() == intent.server.user),
            "Printer job ownership was not confirmed"
        );
        Ok(Proof {
            uuid: self.uuid.clone(),
        })
    }
    pub fn revalidate(&self, intent: &Intent, id: i32, proof: &Proof) -> anyhow::Result<()> {
        self.pending(intent, id)?;
        self.identity(intent, proof)
    }
    fn identity(&self, intent: &Intent, proof: &Proof) -> anyhow::Result<()> {
        if let Some(uuid) = &proof.uuid {
            // CUPS's read-only UUID was bound to exact tag/owner/queue before
            // authentication. Its continuity survives policy hiding name/owner
            // from a different approved HTTP principal. Visible conflicts fail.
            anyhow::ensure!(
                self.uuid.as_ref() == Some(uuid)
                    && self.name.as_ref().is_none_or(|name| name == &intent.name)
                    && self
                        .owner
                        .as_ref()
                        .is_none_or(|owner| owner.as_bytes() == intent.server.user),
                "Printer job identity changed"
            );
        } else {
            anyhow::ensure!(
                self.name.as_deref() == Some(intent.name.as_str())
                    && self
                        .owner
                        .as_ref()
                        .is_some_and(|owner| owner.as_bytes() == intent.server.user),
                "Printer job ownership was not confirmed"
            );
        }
        Ok(())
    }
}

struct Attributes<'a> {
    api: &'a Api,
    response: &'a Message,
    members: Option<&'a HashMap<String, *mut c_void>>,
}
impl Attributes<'_> {
    fn find(&self, name: &CStr, tag: c_int, multiple: bool) -> anyhow::Result<Option<*mut c_void>> {
        let raw = if let Some(members) = self.members {
            members
                .get(name.to_str()?)
                .copied()
                .unwrap_or(ptr::null_mut())
        } else {
            unsafe { (self.api.protocol.find)(self.response.raw, name.as_ptr(), 0) }
        };
        if raw.is_null() {
            return Ok(None);
        }
        let count = unsafe { (self.api.attribute_count)(raw) };
        anyhow::ensure!(
            unsafe { (self.api.protocol.group)(raw) } == 2
                && unsafe { (self.api.attribute_tag)(raw) } == tag
                && if multiple {
                    (1..=32).contains(&count)
                } else {
                    count == 1
                },
            "Invalid printer job attribute"
        );
        Ok(Some(raw))
    }
    fn text(&self, name: &CStr, tag: c_int) -> anyhow::Result<Option<String>> {
        let Some(raw) = self.find(name, tag, false)? else {
            return Ok(None);
        };
        discovery::string(unsafe { (self.api.attribute_string)(raw, 0, ptr::null_mut()) })
    }
    fn optional_integer(&self, name: &CStr, tag: c_int) -> anyhow::Result<Option<i32>> {
        Ok(self
            .find(name, tag, false)?
            .map(|raw| unsafe { (self.api.attribute_integer)(raw, 0) }))
    }
    fn integer(&self, name: &CStr, tag: c_int) -> anyhow::Result<i32> {
        let raw = self
            .find(name, tag, false)?
            .ok_or_else(|| anyhow::anyhow!("Printer job attribute missing"))?;
        Ok(unsafe { (self.api.attribute_integer)(raw, 0) })
    }
    fn contains(&self, name: &CStr, expected: &str) -> anyhow::Result<bool> {
        let Some(raw) = self.find(name, 0x44, true)? else {
            return Ok(false);
        };
        let count = unsafe { (self.api.attribute_count)(raw) };
        let mut found = false;
        for index in 0..count {
            let value = discovery::string(unsafe {
                (self.api.attribute_string)(raw, index, ptr::null_mut())
            })?;
            found |= value.as_deref() == Some(expected);
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Intent, Metadata) {
        let intent = Intent::new(
            Server {
                name: b"localhost".to_vec(),
                port: 631,
                encryption: 0,
                user: b"owner".to_vec(),
            },
            Destination {
                queue: "Fixture".into(),
                instance: None,
            },
            "Document",
        )
        .unwrap();
        let metadata = Metadata {
            id: 7,
            name: Some(intent.name.clone()),
            owner: Some("owner".into()),
            queue: "ipp://localhost/printers/Fixture".into(),
            uuid: Some("urn:uuid:01234567-89ab-cdef-0123-456789abcdef".into()),
            state: 4,
            documents: Some(0),
            incoming: true,
            held_for_authentication: false,
        };
        (intent, metadata)
    }
    #[test]
    fn bound_uuid_survives_hidden_names_but_never_a_changed_job_or_received_document() {
        let (intent, mut metadata) = fixture();
        let proof = metadata.capture(&intent, 7).unwrap();
        metadata.name = None;
        metadata.owner = None;
        metadata.revalidate(&intent, 7, &proof).unwrap();
        metadata.uuid = Some("urn:uuid:11234567-89ab-cdef-0123-456789abcdef".into());
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        metadata.uuid = proof.uuid.clone();
        metadata.documents = Some(1);
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        metadata.documents = Some(0);
        metadata.state = 3;
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        metadata.state = 4;
        metadata.queue = "ipp://localhost/printers/Other".into();
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
    }
    #[test]
    fn initial_owner_and_legacy_proof_require_exact_visible_identity() {
        let (intent, mut metadata) = fixture();
        metadata.owner = Some("other".into());
        assert!(metadata.capture(&intent, 7).is_err());
        metadata.owner = Some("owner".into());
        metadata.uuid = None;
        let proof = metadata.capture(&intent, 7).unwrap();
        metadata.name = None;
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        let (intent, mut metadata) = fixture();
        let proof = metadata.capture(&intent, 7).unwrap();
        metadata.name = Some("changed".into());
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        metadata.name = Some(intent.name.clone());
        metadata.incoming = false;
        assert!(metadata.revalidate(&intent, 7, &proof).is_err());
        assert!(metadata.revalidate(&intent, 8, &proof).is_err());
    }
    #[test]
    fn recovery_uuid_keeps_owner_immutable_across_hidden_fields_and_later_states() {
        let (intent, mut metadata) = fixture();
        let receipt = Receipt {
            intent,
            job: Some(7),
            accepted: false,
            uuid: metadata.uuid.clone(),
        };
        metadata.name = None;
        metadata.owner = None;
        metadata.state = 5;
        metadata.documents = Some(1);
        metadata.incoming = false;
        assert_eq!(metadata.verify_receipt(&receipt, 7).unwrap(), 5);
        assert!(metadata.verify_receipt(&receipt, 8).is_err());
        metadata.uuid = Some("urn:uuid:ffffffff-ffff-ffff-ffff-ffffffffffff".into());
        assert!(metadata.verify_receipt(&receipt, 7).is_err());
        metadata.uuid = receipt.uuid.clone();
        metadata.owner = Some("approved-other-account".into());
        assert!(metadata.verify_receipt(&receipt, 7).is_err());
        metadata.owner = None;
        metadata.queue = "ipp://localhost/printers/Other".into();
        assert!(metadata.verify_receipt(&receipt, 7).is_err());
    }
    #[test]
    fn legacy_recovery_needs_full_identity_and_never_becomes_an_empty_send_proof() {
        let (intent, mut metadata) = fixture();
        let receipt = Receipt {
            intent,
            job: Some(7),
            accepted: false,
            uuid: None,
        };
        metadata.state = 7;
        metadata.documents = None;
        metadata.incoming = false;
        assert_eq!(metadata.verify_receipt(&receipt, 7).unwrap(), 7);
        assert!(metadata.capture(&receipt.intent, 7).is_err());
        metadata.name = None;
        assert!(metadata.verify_receipt(&receipt, 7).is_err());
    }

    #[test]
    fn cleanup_preserves_received_documents_even_without_a_durable_acknowledgement() {
        let (intent, mut metadata) = fixture();
        let mut receipt = Receipt {
            intent,
            job: Some(7),
            accepted: false,
            uuid: metadata.uuid.clone(),
        };
        metadata.name = None;
        metadata.owner = None;
        metadata.verify_incomplete(&receipt, 7).unwrap();
        for state in 3..=6 {
            metadata.state = state;
            metadata.documents = Some(1);
            assert_eq!(metadata.verify_receipt(&receipt, 7).unwrap(), state);
            assert!(metadata.verify_incomplete(&receipt, 7).is_err());
        }
        metadata.state = 4;
        metadata.documents = None;
        assert!(metadata.verify_incomplete(&receipt, 7).is_err());
        metadata.documents = Some(0);
        metadata.incoming = false;
        assert!(metadata.verify_incomplete(&receipt, 7).is_err());
        metadata.incoming = true;
        receipt.accepted = true;
        assert!(metadata.verify_incomplete(&receipt, 7).is_err());
    }
}

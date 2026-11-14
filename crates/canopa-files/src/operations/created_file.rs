//! New Document copies a template, or an empty file, to a new name. It is
//! journalled as an ordinary copied leaf, so an interrupted run is reconciled
//! like a Copy, and Undo removes the document only while it is unchanged.
use super::*;
use std::os::unix::fs::PermissionsExt;

impl OperationEngine {
    pub(super) fn create_file(
        &self,
        path: &Path,
        template: Option<&Path>,
        cancellation: &Cancellation,
    ) -> io::Result<execution::MutationOutcome> {
        validate_target(path)?;
        if let Some(template) = template {
            validate_target(template)?;
        }
        let parent = path.parent().unwrap();
        // The empty document's source lives only for this call.
        let empty = match template {
            Some(_) => None,
            None => Some(empty_source()?),
        };
        let source = template.unwrap_or_else(|| empty.as_ref().unwrap().path());
        check_cancel(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        check_cancel(cancellation)?;
        recover(&mut state, cancellation)?;
        // A taken name is refused before staging, as a Copy reviews its
        // conflicts first: a refused publication keeps its staged data for
        // Recovery, which only a race with another program should need.
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("“{}” already exists.", display_name(path)),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let label = format!("create file {}", display_name(path));
        // Not bound to the request, as for a batch Rename (`rename_batch`).
        let id = state.begin(&label)?;
        let result = (|| {
            let prepared = prepared_copy::prepare_new(source, parent, cancellation, |created| {
                state.register_preparing(id, path, created)
            })?;
            let change = Change::Copied {
                source: None,
                publication: prepared.opened.expected_publication().map(Box::new),
                access_parent: None,
                path: path.to_owned(),
                identity: prepared.identity,
                fingerprint: prepared.fingerprint.clone(),
                retained: None,
            };
            let opened = prepared.opened;
            commit_forward(
                &mut state,
                id,
                change,
                Some((prepared.stage, prepared.temporary)),
                |_| {
                    opened.require_current()?;
                    opened.publish(path)
                },
            )
        })();
        if result.is_err() {
            // A preparation the journal already owns is disposed of now, as
            // the next operation would; anything else stays for Recovery.
            let _ = recover(&mut state, &Cancellation::default());
            if state.pending()?.is_none() {
                state.discard_empty(id)?;
            }
        }
        result?;
        state.trim()?;
        Ok(execution::MutationOutcome {
            summary: OperationSummary {
                id,
                label,
                paths: vec![path.to_owned()],
            },
            confirmation: None,
        })
    }
}

/// A private empty file that a document can be copied from, readable by
/// everyone like an ordinary new file.
fn empty_source() -> io::Result<tempfile::NamedTempFile> {
    let file = tempfile::Builder::new()
        .prefix("canopa-files-empty-")
        .tempfile()?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))?;
    Ok(file)
}

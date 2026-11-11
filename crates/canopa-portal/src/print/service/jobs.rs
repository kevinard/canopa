//! Canopa-private review entry; this adds no method to the XDG Print interface.
use super::*;

#[derive(Clone)]
pub struct PrinterJobs(pub Print);

#[zbus::interface(name = "org.canopa.Portal.PrintJobs")]
impl PrinterJobs {
    async fn review(
        &self,
        handle: OwnedObjectPath,
        parent_window: String,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<u32> {
        if header.primary().body_len() > 8192
            || parent_window.len() > 4096
            || parent_window.contains('\0')
        {
            return Err(super::super::invalid("Invalid printer review request"));
        }
        let print = &self.0;
        let identity_handle = handle.clone();
        let outcome = print
            .0
            .requests
            .run(connection, header, handle, |peer, cancelled| async move {
                let owner = Owner::authenticate(connection, peer, &identity_handle, "").await?;
                let permit = print.begin(false)?;
                let scope = Scope::new(cancelled, owner.cancelled(), permit.cancelled());
                if !owner.confirm().await || !permit.current() {
                    return Ok::<_, fdo::Error>(false);
                }
                print
                    .0
                    .backend
                    .review_jobs(
                        super::super::Context {
                            app_id: "org.canopa.Portal".into(),
                            parent_window,
                            title: fl!("portal-jobs-title"),
                            modal: false,
                        },
                        scope.cancelled(),
                    )
                    .await
                    .map_err(failed)?;
                let current = permit.current() && !*scope.cancelled().borrow();
                Ok(current && owner.confirm().await)
            })
            .await?;
        match outcome {
            Completion::Cancelled => Ok(1),
            Completion::Finished(Ok(true)) => Ok(0),
            Completion::Finished(Ok(false)) => Ok(1),
            Completion::Finished(Err(fdo::Error::Failed(_))) => Ok(2),
            Completion::Finished(Err(error)) => Err(error),
        }
    }
}

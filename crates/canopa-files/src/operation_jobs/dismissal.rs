//! Explicit review releases an ordinary request, never its completed file work.
use super::*;

impl OperationJobs {
    /// Dismiss a failed or restored ordinary mutation or Keep in Recovery request.
    /// Never executes, reverses or deletes its files. Unresolved journals and
    /// active, queued or unsaved work must be settled first.
    pub fn dismiss_request(&self, id: u64) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send_work(Control::DismissRequest(id, reply))
            .map_err(|_| {
                io::Error::other("The operation queue is unavailable or full. Try again.")
            })?;
        Ok(receiver)
    }
}

impl Worker {
    pub(super) fn dismiss_request(&mut self, id: u64) -> io::Result<()> {
        let snapshot = self.state().jobs.iter().find(|row| row.id == id).cloned();
        let snapshot = snapshot
            .filter(|row| row.dismissible)
            .ok_or_else(|| io::Error::other("This request is not ready for dismissal. Finish or cancel active work and save its result first."))?;
        let job = self
            .jobs
            .get(&id)
            .ok_or_else(|| io::Error::other("The request is unavailable."))?;
        if self.active == Some(id)
            || self.queue.contains(&id)
            || !matches!(
                job.saved,
                Saved::Plan {
                    request: Request::Mutation(_) | Request::KeepRecovery { .. },
                    ..
                }
            )
        {
            return Err(io::Error::other(
                "Only a stopped ordinary request can be dismissed.",
            ));
        }
        let mut detail =
            String::from("Request dismissed after review. Completed work and Undo are preserved.");
        for note in [snapshot.detail.as_ref(), snapshot.error.as_ref()]
            .into_iter()
            .flatten()
        {
            detail.push('\n');
            detail.push_str(note);
        }
        let saved = Saved::Completed {
            version: 1,
            title: snapshot.title,
            paths: snapshot.paths.to_vec(),
            detail: Some(detail),
            transfer: None,
        };
        // Do not change the live result before the exact-revision transaction
        // commits. Failure leaves the original row and all reservations intact.
        let record = self.engine.finish_reviewed_request(&job.record, &saved)?;
        let job = self.jobs.get_mut(&id).unwrap();
        job.record = record;
        job.saved = saved;
        job.observed_sources = None;
        job.placement = None;
        job.source_placement = None;
        job.work = None;
        let replacement = job
            .saved
            .snapshot(id, Phase::Finished, job.record.accepted_at())?;
        self.update_job(id, Phase::Finished, None, Some(replacement));
        self.trim();
        Ok(())
    }
}

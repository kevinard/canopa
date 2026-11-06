use super::*;
use crate::print::live_jobs::{Job, MAX_JOBS};

impl Client {
    pub(crate) async fn live_jobs(
        &self,
        destination: Destination,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Vec<Job>>> {
        destination.validate()?;
        let expected = destination.clone();
        match self
            .execute(
                Operation::ListLiveJobs { destination },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None | Some(Event::AuthenticationCancelled) => Ok(None),
            Some(Event::LiveJobs { jobs }) => {
                anyhow::ensure!(jobs.len() <= MAX_JOBS, "Printer job listing exceeds limits");
                let mut ids = std::collections::HashSet::new();
                for job in &jobs {
                    job.validate()?;
                    anyhow::ensure!(
                        ids.insert(job.job)
                            && job
                                .binding
                                .as_ref()
                                .is_none_or(|binding| binding.destination == expected),
                        "Unexpected live printer job identity"
                    );
                }
                Ok(Some(jobs))
            }
            _ => anyhow::bail!("Unexpected printer job listing response"),
        }
    }
    pub(crate) async fn authenticate_held_job(
        &self,
        job: Job,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<()>> {
        job.validate()?;
        anyhow::ensure!(
            job.can_authenticate(),
            "Printer job cannot be authenticated"
        );
        let id = job.job;
        match self
            .execute(
                Operation::AuthenticateHeldJob { job },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None | Some(Event::AuthenticationCancelled) => Ok(None),
            Some(Event::AuthenticatedJob { job }) if job == id => Ok(Some(())),
            _ => anyhow::bail!(
                "Printer sign-in was not acknowledged; refresh the job before another attempt"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stopped_live_job_worker_preserves_the_protocol_failure_reason() {
        let client = Client::new(DialogProcess {
            executable: "/usr/bin/python3".into(),
            arguments: vec!["-c".into(),
                "import json,sys,time; request=json.loads(sys.stdin.buffer.readline()); assert request['operation']['operation']=='list_live_jobs'; print(json.dumps({'event':'failed','message':'Live printer identity fixture failure'}),flush=True); time.sleep(30)".into()],
        });
        let (_cancel, cancelled) = watch::channel(false);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            client.live_jobs(
                Destination {
                    queue: "Fixture".into(),
                    instance: None,
                },
                cancelled,
            ),
        )
        .await
        .unwrap()
        .unwrap_err()
        .to_string();
        assert!(error.contains("Print worker stopped"), "{error}");
        assert!(
            error.contains("Live printer identity fixture failure"),
            "{error}"
        );
        assert_eq!(client.shutdown().await, 0);
    }
}

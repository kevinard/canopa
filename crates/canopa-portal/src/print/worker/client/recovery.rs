use super::*;
use crate::print::file_target::RecoveryBody;

impl Client {
    pub async fn printer_recovery(
        &self,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Vec<super::super::PrinterRecoveryItem>>> {
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        match self
            .execute(
                Operation::ListPrinterRecovery { store },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::PrinterRecovery { items }) => {
                let mut keys = std::collections::HashSet::new();
                anyhow::ensure!(
                    items.len() <= crate::print::printer_jobs::MAX_JOBS,
                    "Printer recovery list exceeds its limit"
                );
                for item in &items {
                    item.validate()?;
                    anyhow::ensure!(keys.insert(&item.id), "Duplicate printer recovery item");
                }
                Ok(Some(items))
            }
            _ => anyhow::bail!("Unexpected printer recovery list response"),
        }
    }

    pub async fn dismiss_printer_recovery(
        &self,
        item: super::super::PrinterRecoveryItem,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<()>> {
        item.validate()?;
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        let id = item.id.clone();
        match self
            .execute(
                Operation::DismissPrinterRecovery { store, item },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::DismissedPrinterRecovery { id: dismissed }) if dismissed == id => {
                Ok(Some(()))
            }
            _ => anyhow::bail!("Printer record removal was not confirmed. Refresh the list."),
        }
    }

    pub async fn recover_snapshot(
        &self,
        id: String,
        body: RecoveryBody,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Recovered>> {
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        let permit = self.documents.clone().try_acquire_owned()?;
        let storage = Arc::new(Storage {
            file: File::from(rustix::fs::memfd_create(
                c"canopa-recovered-print",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )?),
            budget: Arc::new(Budget {
                _permit: permit,
                derived: Arc::new(Semaphore::new(1)),
            }),
            _source: None,
            _derived: None,
        });
        let target = duplicate(&storage.file)?;
        let operation = Operation::SnapshotRecovery {
            store,
            id,
            body,
            target: target.as_raw_fd(),
        };
        match self
            .execute(operation, vec![target], cancelled, Some(storage.clone()))
            .await?
        {
            None => Ok(None),
            Some(Event::Recovered {
                document: reported,
                protection,
            }) => {
                let actual = document::verify_recovery(&storage.file)?;
                anyhow::ensure!(
                    actual.bytes == reported.bytes && actual.format == reported.format,
                    "Invalid recovery snapshot response"
                );
                Ok(Some(Recovered {
                    storage,
                    bytes: actual.bytes,
                    protection,
                }))
            }
            _ => anyhow::bail!("Unexpected recovery snapshot response"),
        }
    }

    pub async fn dismiss_recovery(
        &self,
        id: String,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<()>> {
        let store = self
            .recovery
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print recovery is not configured"))?
            .store
            .clone();
        match self
            .execute(
                Operation::DismissRecovery {
                    store,
                    id: id.clone(),
                },
                vec![],
                cancelled,
                None,
            )
            .await?
        {
            None => Ok(None),
            Some(Event::DismissedRecovery { id: dismissed }) if dismissed == id => Ok(Some(())),
            _ => anyhow::bail!("Recovery dismissal was not confirmed. Refresh the list."),
        }
    }
}

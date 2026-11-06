//! Resolve stable PipeWire identities without handing a reused numeric ID out.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::AtomicBool,
    time::Instant,
};

#[derive(Clone)]
pub(crate) struct Serials {
    slots: Arc<Semaphore>,
}
impl Default for Serials {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(8)),
        }
    }
}
struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
impl Serials {
    pub(crate) async fn resolve(
        &self,
        nodes: Vec<u32>,
        mut cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<HashMap<u32, u64>> {
        let permit = self.slots.clone().try_acquire_owned()?;
        let stop = Stop(Arc::new(AtomicBool::new(false)));
        let stopping = stop.0.clone();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            resolve(nodes, stopping)
        });
        tokio::select! {
            biased;
            _ = async {if !*cancelled.borrow(){let _=cancelled.changed().await;}} => anyhow::bail!("Capture cancelled"),
            result=task => result?,
        }
    }
}
fn resolve(nodes: Vec<u32>, stop: Arc<AtomicBool>) -> anyhow::Result<HashMap<u32, u64>> {
    use pipewire as pw;
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(pw::init);
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry()?;
    let identities = Rc::new(RefCell::new(HashMap::new()));
    let failed = Rc::new(Cell::new(false));
    let done = Rc::new(Cell::new(false));
    let globals = Rc::new(Cell::new(0_usize));
    let wanted: HashSet<_> = nodes.iter().copied().collect();
    anyhow::ensure!(
        !wanted.is_empty() && wanted.len() <= 16 && wanted.len() == nodes.len(),
        "Invalid capture identity request"
    );
    let found = identities.clone();
    let invalid = failed.clone();
    let count = globals.clone();
    let selected = wanted.clone();
    let removed = identities.clone();
    let _registry = registry
        .add_listener_local()
        .global(move |global| {
            count.set(count.get() + 1);
            if count.get() > 4096 {
                invalid.set(true);
                return;
            }
            if !selected.contains(&global.id) {
                return;
            }
            if global.type_ != pw::types::ObjectType::Node {
                invalid.set(true);
                return;
            }
            let serial = global
                .props
                .as_ref()
                .and_then(|p| p.get("object.serial"))
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v != 0);
            match serial {
                Some(serial) => {
                    found.borrow_mut().insert(global.id, serial);
                }
                None => invalid.set(true),
            }
        })
        .global_remove(move |id| {
            removed.borrow_mut().remove(&id);
        })
        .register();
    let pending = core.sync(0)?;
    let complete = done.clone();
    let invalid = failed.clone();
    let _core = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == pw::core::PW_ID_CORE && seq == pending {
                complete.set(true);
            }
        })
        .error(move |_, _, _, _| invalid.set(true))
        .register();
    let end = Instant::now() + Duration::from_secs(5);
    while !done.get() {
        anyhow::ensure!(
            !failed.get() && !stop.load(Ordering::SeqCst) && Instant::now() < end,
            "Capture identities unavailable"
        );
        mainloop.loop_().iterate(Duration::from_millis(20));
    }
    anyhow::ensure!(
        !failed.get() && !stop.load(Ordering::SeqCst) && identities.borrow().len() == wanted.len(),
        "Capture identities unavailable"
    );
    let result = identities.borrow().clone();
    anyhow::ensure!(
        result.values().copied().collect::<HashSet<_>>().len() == result.len(),
        "Duplicate capture serial"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires scripts/test-screencast-identities.py and its private PipeWire daemon"]
    fn native_identity_fixture() {
        use std::io::{BufRead, Write};
        assert_eq!(
            std::env::var("CANOPA_IDENTITY_NATIVE_FIXTURE").as_deref(),
            Ok("1")
        );
        for line in std::io::stdin().lock().lines() {
            let value: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
            if value.get("stop") == Some(&serde_json::Value::Bool(true)) {
                break;
            }
            let nodes: Vec<u32> = serde_json::from_value(value["nodes"].clone()).unwrap();
            let result = match resolve(nodes, Arc::new(AtomicBool::new(false))) {
                Ok(identities) => serde_json::json!({"identities":identities}),
                Err(_) => serde_json::json!({"unavailable":true}),
            };
            println!("IDENTITIES {result}");
            std::io::stdout().flush().unwrap();
        }
    }
}

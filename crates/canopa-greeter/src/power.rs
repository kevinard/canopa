//! Unprivileged logind power requests. Policy is queried again after confirmation.
use std::time::Duration;
use zbus::{Connection, Proxy};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Shutdown,
    Restart,
}
impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shutdown => "Shut down",
            Self::Restart => "Restart",
        }
    }
    fn can(self) -> &'static str {
        match self {
            Self::Shutdown => "CanPowerOff",
            Self::Restart => "CanReboot",
        }
    }
    fn method(self) -> &'static str {
        match self {
            Self::Shutdown => "PowerOff",
            Self::Restart => "Reboot",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub shutdown: bool,
    pub restart: bool,
}
impl Capabilities {
    pub fn allows(self, action: Action) -> bool {
        match action {
            Action::Shutdown => self.shutdown,
            Action::Restart => self.restart,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Menu {
    #[default]
    Closed,
    Choose(Action),
    Confirm(Action),
    Pending(Action),
}
pub enum Update {
    Capabilities(Capabilities),
    Finished(bool),
}
pub struct Worker {
    commands: tokio::sync::mpsc::Sender<Action>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
async fn manager(connection: &Connection) -> zbus::Result<Proxy<'_>> {
    Proxy::new(
        connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await
}
async fn allowed(manager: &Proxy<'_>, action: Action) -> bool {
    // A login screen has no authenticated administrator identity. A "challenge"
    // result is unavailable here; logind still checks policy on the actual call.
    tokio::time::timeout(
        Duration::from_secs(2),
        manager.call::<_, _, String>(action.can(), &()),
    )
    .await
    .is_ok_and(|r| r.is_ok_and(|s| s == "yes"))
}
impl Worker {
    pub fn start(mut notify: impl FnMut(Update) + Send + 'static) -> Self {
        let (commands, mut requests) = tokio::sync::mpsc::channel::<Action>(1);
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                use futures_util::StreamExt;
                loop {
                    let connected=tokio::select! {_=&mut stopped=>return,result=Connection::system()=>result};
                    let Ok(connection)=connected else {
                        notify(Update::Capabilities(Default::default()));
                        tokio::select! {_=&mut stopped=>return,_=tokio::time::sleep(Duration::from_secs(5))=>continue}
                    };
                    let rule=zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.login1").unwrap().path_namespace("/org/freedesktop/login1").unwrap().build();
                    let Ok(mut changes)=zbus::MessageStream::for_match_rule(rule,&connection,Some(64)).await else{return;};
                    let Ok(manager)=manager(&connection).await else{return;};
                    let mut last=None;
                    loop {
                        let capabilities=tokio::select! {_=&mut stopped=>return,result=async {
                            Capabilities{shutdown:allowed(&manager,Action::Shutdown).await,restart:allowed(&manager,Action::Restart).await}
                        }=>result};
                        if last!=Some(capabilities) {notify(Update::Capabilities(capabilities));last=Some(capabilities);}
                        tokio::select! {biased;
                            _=&mut stopped=>return,
                            action=requests.recv()=>{
                                let Some(action)=action else{return;};
                                let permitted=allowed(&manager,action).await;
                                let success=if permitted {
                                    tokio::select! {_=&mut stopped=>return,result=tokio::time::timeout(Duration::from_secs(5),manager.call::<_,_,()>(action.method(),&(false,)))=>result.is_ok_and(|r|r.is_ok())}
                                }else{false};
                                notify(Update::Finished(success));
                            },
                            changed=changes.next()=>if changed.is_none() {break;},
                        }
                    }
                }
            });
        });
        Self {
            commands,
            stop: Some(stop),
            thread: Some(thread),
        }
    }
    pub fn request(&self, action: Action) -> bool {
        self.commands.try_send(action).is_ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

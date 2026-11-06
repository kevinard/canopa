use super::{
    server::Receiver,
    wire::{
        self, CaptureEvent, Frame, Notice, ParentCommand, ParentRequest, ReadMessage, Reader,
        Reply, SourceRequest,
    },
    *,
};
use crate::process::fd;
use std::{
    collections::{BTreeSet, VecDeque},
    os::fd::{AsFd, OwnedFd},
};
use tokio::{io::AsyncWriteExt, net::unix::OwnedWriteHalf, time::Instant};

pub fn run_worker() -> anyhow::Result<()> {
    let socket = UnixStream::from(rustix::io::dup(std::io::stdin().as_fd())?);
    let parent_pid = u32::try_from(
        rustix::net::sockopt::socket_peercred(&socket)?
            .pid
            .as_raw_nonzero()
            .get(),
    )?;
    socket.set_nonblocking(true)?;
    let ready = socket.try_clone()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut control = tokio::net::UnixStream::from_std(socket)?;
            let input: Input = tokio::time::timeout(CALL_TIME, wire::read(&mut control)).await??;
            input.validate()?;
            run(input, parent_pid, control, ready).await
        })
}
async fn stopping(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}
async fn write<T: Serialize>(
    output: &mut OwnedWriteHalf,
    value: &T,
    stop: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    tokio::select! {
        biased;
        _=stopping(stop)=>anyhow::bail!("Capture stopped"),
        result=tokio::time::timeout(Duration::from_secs(2),wire::write(output,value))=>result??,
    }
    Ok(())
}
struct Pending {
    sequence: u64,
    parent: Option<u64>,
    command: Command,
    deadline: Instant,
}
struct Queued {
    request: ParentRequest,
    deadline: Instant,
}
async fn run(
    input: Input,
    parent_pid: u32,
    control: tokio::net::UnixStream,
    ready: UnixStream,
) -> anyhow::Result<()> {
    let connection = Connection::session().await?;
    let (ended, mut stop) = watch::channel(false);
    let _owner = tokio::time::timeout(
        CALL_TIME,
        observe(&connection, &input, parent_pid, ended.clone()),
    )
    .await??;
    let signals = ended.clone();
    let _signals = Watch(tokio::spawn(async move {
        let result = async {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
            tokio::select! {_=term.recv()=>{},_=interrupt.recv()=>{}};
            Ok::<_, std::io::Error>(())
        }
        .await;
        let _ = result;
        signals.send_replace(true);
    }));
    let proxy = proxy(&connection, input.compositor.as_str().try_into()?).await?;
    let opening = async {
        let version = proxy.get_property::<u32>("Version").await?;
        let available = provider_capabilities(
            version,
            proxy.get_property::<u32>("SupportedCapabilities").await?,
        )?;
        anyhow::ensure!(
            input.grants & !available == 0,
            "Capture grant became unavailable"
        );
        let (source, zone_set, zones, keymap, text) = if input.grants & 1 != 0 {
            let value: (zbus::zvariant::OwnedFd, u32, Zones, u32, String) =
                proxy.call("OpenCapture", &(input.grants,)).await?;
            anyhow::ensure!(value.3 != 0, "Invalid capture keymap revision");
            value
        } else {
            let (source, zone_set, zones): (zbus::zvariant::OwnedFd, u32, Zones) =
                proxy.call("OpenPointer", &()).await?;
            (source, zone_set, zones, 0, String::new())
        };
        wire::validate_zones(&zones)?;
        let socket = super::socket(OwnedFd::from(source), input.compositor_pid)?;
        Ok::<_, anyhow::Error>((
            tokio::net::UnixStream::from_std(socket)?,
            zone_set,
            zones,
            keymap,
            text,
        ))
    };
    let (source, zone_set, zones, mut keymap_revision, keymap_text) = tokio::select! {
        biased;_=stopping(&mut stop)=>return Ok(()),
        result=tokio::time::timeout(CALL_TIME,opening)=>result??,
    };
    let (source_read, mut source_write) = source.into_split();
    let mut captures = Reader::<CaptureEvent>::new(source_read, 32);
    let (server, client) = UnixStream::pair()?;
    let mut receiver = Receiver::new(
        server,
        input.grants,
        (input.grants & 1 != 0).then_some(keymap_text.as_str()),
    )?;
    anyhow::ensure!(!*stop.borrow(), "Capture start cancelled");
    fd::send_ready(&ready, &client)?;
    drop(client);
    drop(ready);
    let (parent_read, mut parent_write) = control.into_split();
    let mut requests = Reader::<ParentRequest>::new(parent_read, 8);
    let result=async {
        write(&mut parent_write,&Frame::Ready {zone_set,zones},&mut stop).await?;
        let mut queue=VecDeque::<Queued>::new();let mut pending:Option<Pending>=None;
        let mut parent_sequence=0u64;let mut sequence=0u64;
        let mut connected=false;let mut handshake_deadline=None;
        let mut enabled=false;let mut active=None;let mut last_activation=None;
        let mut barriers=BTreeSet::new();let mut generation=zone_set;let mut zones_valid=true;
        let mut disable_needed=false;
        loop {
            if *stop.borrow() {return Ok(());}
            // Parent RPC is serialized. Internal receiver-device replacement
            // takes priority, so a new device cannot inherit old interception.
            if pending.is_none() {
                let next=if disable_needed {disable_needed=false;Some((None,Command::Disable))}
                    else if let Some(queued)=queue.front() {
                        match &queued.request.command {
                            ParentCommand::Connect=>{
                                let queued=queue.pop_front().unwrap();
                                let reply=if connected {Reply::Failed {message:"EI connection already handed off".into()}}
                                    else {connected=true;handshake_deadline=Some(Instant::now()+CALL_TIME);Reply::Unit};
                                write(&mut parent_write,&Frame::Reply {sequence:queued.request.sequence,reply},&mut stop).await?;None
                            }
                            ParentCommand::Capture {command:Command::Enable} if !connected=>{
                                let queued=queue.pop_front().unwrap();
                                write(&mut parent_write,&Frame::Reply {sequence:queued.request.sequence,reply:Reply::Failed {message:"ConnectToEIS must precede Enable".into()}},&mut stop).await?;None
                            }
                            ParentCommand::Capture {command:Command::Enable} if !receiver.ready()=>None,
                            ParentCommand::Capture {..}=>{
                                let queued=queue.pop_front().unwrap();
                                let ParentCommand::Capture {command}=queued.request.command else {unreachable!()};
                                Some((Some(queued.request.sequence),command))
                            }
                        }
                    } else {None};
                if let Some((parent,command))=next {
                    command.validate()?;
                    sequence=sequence.checked_add(1).ok_or_else(||anyhow::anyhow!("Capture serial exhausted"))?;
                    let request = SourceRequest::new(sequence, command.clone(), (input.grants & 1 != 0).then_some(keymap_revision));
                    write(&mut source_write, &request, &mut stop).await?;
                    pending=Some(Pending {sequence,parent,command,deadline:Instant::now()+CALL_TIME});
                }
            }
            tokio::select! {
                _=stopping(&mut stop)=>return Ok(()),
                _=async {tokio::time::sleep_until(handshake_deadline.unwrap()).await},if connected && !receiver.handshake_done()=>anyhow::bail!("EI receiver handshake timed out"),
                _=async {tokio::time::sleep_until(pending.as_ref().unwrap().deadline).await},if pending.is_some()=>anyhow::bail!("Compositor capture acknowledgement timed out"),
                _=async {tokio::time::sleep_until(queue.front().unwrap().deadline).await},if pending.is_none() && !queue.is_empty()=>{
                    let queued=queue.pop_front().unwrap();
                    write(&mut parent_write,&Frame::Reply {sequence:queued.request.sequence,reply:Reply::Failed {message:"EI receiver device not ready".into()}},&mut stop).await?;
                }
                request=requests.recv()=>match request {
                    Some(ReadMessage::Data {value,..})=>{
                        anyhow::ensure!(parent_sequence.checked_add(1)==Some(value.sequence) && queue.len()<8,"Invalid capture parent command sequence");
                        parent_sequence=value.sequence;
                        if let ParentCommand::Capture {command}=&value.command {command.validate()?;}
                        queue.push_back(Queued {request:value,deadline:Instant::now()+CALL_TIME});
                    }
                    Some(ReadMessage::Eof)|None=>return Ok(()),
                    Some(ReadMessage::Error(error))=>anyhow::bail!(error),
                },
                read=receiver.read()=>{
                    let Some(outcome)=read? else {return Ok(());};
                    if outcome.ready_changed {
                        write(&mut parent_write,&Frame::Notice {notice:Notice::ReceiverReady {ready:receiver.ready()}},&mut stop).await?;
                    }
                    disable_needed|=outcome.disable;
                }
                event=captures.recv()=>{
                    let event=match event {
                        Some(ReadMessage::Data {value,..})=>value,
                        Some(ReadMessage::Error(error))=>anyhow::bail!(error),
                        _=>anyhow::bail!("Compositor capture lease ended"),
                    };
                    match event {
                        CaptureEvent::Reply {sequence,error,failed_barriers}=>{
                            let request=pending.take().ok_or_else(||anyhow::anyhow!("Unexpected compositor reply"))?;
                            anyhow::ensure!(sequence==request.sequence,"Compositor reply sequence mismatch");
                            let reply=if let Some(message)=error {
                                anyhow::ensure!(message.len()<=256 && request.parent.is_some(),"Internal capture disable failed");
                                Reply::Failed {message}
                            } else {
                                match request.command {
                                    Command::GetZones=>anyhow::bail!("Missing capture zones"),
                                    Command::SetBarriers {zone_set,barriers:requested}=>{
                                        anyhow::ensure!(failed_barriers.len()<=128 && zone_set==generation && zones_valid,"Invalid compositor barrier result");
                                        barriers=requested.into_iter().map(|(id,_)|id).filter(|id|!failed_barriers.contains(id)).collect();
                                        enabled=false;Reply::Barriers {failed_barriers}
                                    }
                                    Command::Enable=>{enabled=true;Reply::Unit}
                                    Command::Disable=>{enabled=false;Reply::Unit}
                                    Command::Release {..}=>Reply::Unit,
                                }
                            };
                            if let Some(parent)=request.parent {write(&mut parent_write,&Frame::Reply {sequence:parent,reply},&mut stop).await?;}
                        }
                        CaptureEvent::Zones {sequence,zone_set,zones}=>{
                            let request=pending.take().ok_or_else(||anyhow::anyhow!("Unexpected compositor zones"))?;
                            anyhow::ensure!(request.sequence==sequence && matches!(request.command,Command::GetZones),"Invalid compositor zones reply");
                            wire::validate_zones(&zones)?;generation=zone_set;zones_valid=true;
                            write(&mut parent_write,&Frame::Reply {sequence:request.parent.expect("requested zones"),reply:Reply::Zones {zone_set,zones}},&mut stop).await?;
                        }
                        CaptureEvent::Activated {activation,barrier,position}=>{
                            anyhow::ensure!(enabled && active.is_none() && receiver.ready() && zones_valid && barriers.contains(&barrier)
                                && [position.0,position.1].into_iter().all(f64::is_finite),"Unapproved capture activation");
                            if let Some(previous)=last_activation {
                                let delta=activation.wrapping_sub(previous);
                                anyhow::ensure!(delta!=0 && delta<1<<31,"Stale capture activation ID");
                            }
                            receiver.begin(activation)?;active=Some(activation);last_activation=Some(activation);
                            write(&mut parent_write,&Frame::Notice {notice:Notice::Activated {activation,barrier,position}},&mut stop).await?;
                        }
                        CaptureEvent::Deactivated {activation}=>{
                            anyhow::ensure!(active==Some(activation),"Mismatched capture deactivation");
                            receiver.deactivate()?;active=None;
                            write(&mut parent_write,&Frame::Notice {notice:Notice::Deactivated {activation}},&mut stop).await?;
                        }
                        CaptureEvent::Disabled=>{
                            anyhow::ensure!(active.is_none(),"Capture disabled before deactivation");enabled=false;
                            write(&mut parent_write,&Frame::Notice {notice:Notice::Disabled},&mut stop).await?;
                        }
                        CaptureEvent::KeymapChanged {keymap}=>{
                            anyhow::ensure!(input.grants & 1 != 0 && !enabled && active.is_none() && keymap != 0,"Unapproved keymap transition");
                            let delta = keymap.wrapping_sub(keymap_revision);
                            if delta != 0 && delta < 1 << 31 {
                                let fetching = proxy.call::<_,_,(u32,String)>("KeyboardSnapshot", &());
                                let (revision,text) = tokio::select! {
                                    biased; _=stopping(&mut stop)=>return Ok(()),
                                    result=tokio::time::timeout(CALL_TIME,fetching)=>result??,
                                };
                                anyhow::ensure!(revision != 0 && revision.wrapping_sub(keymap) < 1 << 31,"Stale capture keymap snapshot");
                                receiver.replace_keymap(&text)?;
                                keymap_revision = revision;
                                write(&mut parent_write,&Frame::Notice {notice:Notice::ReceiverReady {ready:receiver.ready()}},&mut stop).await?;
                            }
                        }
                        CaptureEvent::ZonesChanged {zone_set}=>{
                            anyhow::ensure!(!enabled && active.is_none() && (!zones_valid || zone_set==generation),"Invalid capture topology transition");
                            barriers.clear();zones_valid=false;
                            write(&mut parent_write,&Frame::Notice {notice:Notice::ZonesChanged {zone_set}},&mut stop).await?;
                        }
                        data=>{
                            anyhow::ensure!(active.is_some(),"Captured data outside an activation");
                            anyhow::ensure!(receiver.active().is_none() || receiver.active()==active,"EI/source activation mismatch");
                            receiver.input(&data)?;
                        }
                    }
                }
            }
            tokio::task::yield_now().await;
        }
    }.await;
    // Half-close only the control write direction and drain until Niri itself
    // acknowledges retirement with EOF. Bound both time and discarded bytes.
    let shutdown = source_write.shutdown().await;
    receiver.finish();
    let cleanup = tokio::time::timeout(Duration::from_secs(2), async {
        shutdown?;
        let mut bytes = 0usize;
        loop {
            match captures.recv().await {
                Some(ReadMessage::Data { bytes: count, .. }) => {
                    bytes += count;
                    anyhow::ensure!(bytes <= 1024 * 1024, "Capture cleanup exceeded budget");
                }
                Some(ReadMessage::Eof) => return Ok::<_, anyhow::Error>(()),
                Some(ReadMessage::Error(error)) => anyhow::bail!(error),
                None => anyhow::bail!("Capture cleanup unconfirmed"),
            }
        }
    })
    .await;
    drop(captures);
    drop(source_write);
    drop(requests);
    drop(parent_write);
    cleanup??;
    result
}

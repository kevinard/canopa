use super::*;
use crate::process::DialogProcess;
use anyhow::{Result, anyhow, ensure};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::watch,
    task::JoinHandle,
};

pub struct Production {
    process: Arc<DialogProcess>,
}
impl Production {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self { process }
    }
}
struct Dialog {
    events: Option<mpsc::Receiver<Decision>>,
    feedback: mpsc::Sender<Feedback>,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}
impl Drop for Dialog {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
impl Editor for Dialog {
    fn events(&mut self) -> Option<mpsc::Receiver<Decision>> {
        self.events.take()
    }
    fn update(&self, feedback: Feedback) -> Result<()> {
        self.feedback
            .try_send(feedback)
            .map_err(|_| anyhow!("Shortcut editor stopped or its update queue is full"))
    }
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        Box::pin(async move {
            self.stop.send_replace(true);
            if let Some(mut task) = self.task.take()
                && tokio::time::timeout(Duration::from_secs(7), &mut task)
                    .await
                    .is_err()
            {
                task.abort();
                let _ = task.await;
                anyhow::bail!("Shortcut editor did not retire");
            }
            Ok(())
        })
    }
}
struct Reader(JoinHandle<Result<()>>);
impl Drop for Reader {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Presenter for Production {
    fn open(&self, request: Request) -> BoxFuture<'_, Result<Box<dyn Editor>>> {
        Box::pin(async move {
            let mut child = self.process.spawn(
                "--global-shortcuts-dialog",
                request.activation_token.as_deref(),
            )?;
            let mut input = child.stdin.take().expect("piped input");
            let output = child.stdout.take().expect("piped output");
            let sending =
                tokio::time::timeout(Duration::from_secs(5), write(&mut input, &request)).await;
            if !matches!(sending, Ok(Ok(()))) {
                let _ = child.kill().await;
                let _ = child.wait().await;
                sending??;
            }
            let (events, receive) = mpsc::channel(4);
            let (feedback, mut updates) = mpsc::channel(4);
            let (stop, mut stopped) = watch::channel(false);
            let task = tokio::spawn(async move {
                let mut reader = Reader(tokio::spawn(async move {
                    let mut output = BufReader::new(output);
                    loop {
                        let mut bytes = Vec::new();
                        (&mut output)
                            .take(MAX_FRAME as u64 + 1)
                            .read_until(b'\n', &mut bytes)
                            .await?;
                        ensure!(
                            bytes.len() <= MAX_FRAME && bytes.last() == Some(&b'\n'),
                            "Invalid shortcut editor frame"
                        );
                        let decision: Decision = serde_json::from_slice(&bytes)?;
                        let cancel = matches!(decision, Decision::Cancel);
                        events
                            .try_send(decision)
                            .map_err(|_| anyhow!("Shortcut decision queue is full or stopped"))?;
                        if cancel {
                            return Ok(());
                        }
                    }
                }));
                loop {
                    tokio::select! {biased;
                        _=async{if !*stopped.borrow(){let _=stopped.changed().await;}}=>break,
                        _=child.wait()=>break,
                        _=&mut reader.0=>break,
                        update=updates.recv()=>{
                            let Some(update)=update else{break;};
                            if !matches!(tokio::time::timeout(Duration::from_secs(5),write(&mut input,&update)).await,Ok(Ok(()))){break;}
                        }
                    }
                }
                let _ = child.kill().await;
                let _ = child.wait().await;
            });
            Ok(Box::new(Dialog {
                events: Some(receive),
                feedback,
                stop,
                task: Some(task),
            }) as Box<dyn Editor>)
        })
    }
}
async fn write(output: &mut tokio::process::ChildStdin, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() < MAX_FRAME, "Shortcut editor message too large");
    bytes.push(b'\n');
    output.write_all(&bytes).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global_shortcuts::{Action, Actions, Binding};
    fn request() -> Request {
        Request {
            app_id: "example.App".into(),
            parent_window: String::new(),
            activation_token: None,
            configuring: false,
            actions: Actions(vec![Action {
                id: "record".into(),
                description: "Record".into(),
                preferred_trigger: Some("F9".into()),
            }]),
            defaults: vec![Binding {
                id: "record".into(),
                trigger: "F9".into(),
            }],
            current: Vec::new(),
            generation: 4,
        }
    }
    fn presenter(directory: &std::path::Path, script: &str) -> Production {
        let path = directory.join("editor.py");
        std::fs::write(&path, script).unwrap();
        Production::new(Arc::new(DialogProcess {
            executable: "/usr/bin/python3".into(),
            arguments: vec![path.into_os_string()],
        }))
    }
    #[tokio::test]
    async fn live_helper_receives_conflict_feedback_without_reopening_and_stop_reaps_it() {
        let directory = tempfile::tempdir().unwrap();
        let presenter = presenter(
            directory.path(),
            r#"import json,sys
request=json.loads(sys.stdin.readline())
print(json.dumps({'event':'apply','generation':request['generation'],'bindings':[{'id':'record','trigger':'F9'}]}),flush=True)
feedback=json.loads(sys.stdin.readline())
assert feedback['error']=='conflict' and feedback['generation']==7
print(json.dumps({'event':'cancel'}),flush=True)
"#,
        );
        let mut editor = presenter.open(request()).await.unwrap();
        let mut events = editor.events().unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap(),
            Some(Decision::Apply { generation: 4, .. })
        ));
        editor
            .update(Feedback {
                generation: 7,
                current: Vec::new(),
                error: Some("conflict".into()),
                failed: vec!["record".into()],
                busy: false,
            })
            .unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap(),
            Some(Decision::Cancel)
        ));
        editor.stop().await.unwrap();
    }
    #[tokio::test]
    async fn oversized_helper_output_closes_the_bounded_decision_stream() {
        let directory = tempfile::tempdir().unwrap();
        let presenter = presenter(
            directory.path(),
            "import sys,time\nsys.stdin.readline()\nsys.stdout.write('x'*200000+'\\n');sys.stdout.flush()\ntime.sleep(60)\n",
        );
        let mut editor = presenter.open(request()).await.unwrap();
        let mut events = editor.events().unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .is_none()
        );
        editor.stop().await.unwrap();
    }
    #[tokio::test]
    async fn stopping_a_silent_helper_does_not_wait_for_output_or_leave_a_process() {
        let directory = tempfile::tempdir().unwrap();
        let presenter = presenter(
            directory.path(),
            "import sys,time\nsys.stdin.readline()\ntime.sleep(60)\n",
        );
        let editor = presenter.open(request()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), editor.stop())
            .await
            .unwrap()
            .unwrap();
    }
}

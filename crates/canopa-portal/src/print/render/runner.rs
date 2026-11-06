use super::{Mode, Request};
use crate::print::{
    Format,
    worker::{Cancellation, Document, document},
};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::process::CommandExt,
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

const STDERR_BYTES: usize = 1024 * 1024;
const TEMP_BYTES: u64 = 256 * 1024 * 1024;
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn stage() -> anyhow::Result<File> {
    Ok(File::from(rustix::fs::memfd_create(
        c"canopa-print-render",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?))
}
fn reopen(file: &File) -> anyhow::Result<File> {
    // A new open-file description prevents a converter advancing the cursor
    // shared by other holders of the frozen source. This is an owned FD path.
    Ok(File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))?)
}

fn sandbox() -> Command {
    let mut command = Command::new("/usr/bin/bwrap");
    command.env_clear().args([
        "--unshare-all",
        "--unshare-user",
        "--disable-userns",
        "--die-with-parent",
        "--as-pid-1",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--setenv",
        "PATH",
        "/usr/bin",
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "TMPDIR",
        "/tmp",
        "--setenv",
        "LANG",
        "C.UTF-8",
    ]);
    for path in ["/usr", "/lib", "/lib64"] {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
    command.args([
        "--dir",
        "/etc",
        "--ro-bind-try",
        "/etc/fonts",
        "/etc/fonts",
        "--ro-bind-try",
        "/etc/ld.so.cache",
        "/etc/ld.so.cache",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--size",
        &TEMP_BYTES.to_string(),
        "--tmpfs",
        "/tmp",
        "--chdir",
        "/tmp",
    ]);
    command
}

fn run(
    mut command: Command,
    source: &File,
    target: &File,
    cancel: &Cancellation,
    inherit: &[&File],
) -> anyhow::Result<()> {
    cancel.check()?;
    command
        .stdin(Stdio::from(reopen(source)?))
        .stdout(Stdio::from(target.try_clone()?))
        .stderr(Stdio::piped());
    let parent = rustix::process::getpid();
    let inherited = inherit
        .iter()
        .map(|file| file.as_raw_fd())
        .collect::<Vec<_>>();
    // SAFETY: only syscalls and captured numeric values are used after fork.
    // The converter becomes PID1 in its own PID namespace: its death retires
    // the whole namespace, including children which change their process group.
    unsafe {
        command.pre_exec(move || {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
            if rustix::process::getppid() != Some(parent) {
                return Err(rustix::io::Errno::CHILD.into());
            }
            // Helpers received raw IPC descriptors from their parent. Close
            // those on exec too: converters get stdio and explicit file pins
            // only, even when another caller left an FD inheritable.
            if libc::close_range(3, u32::MAX, libc::CLOSE_RANGE_CLOEXEC as _) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            for fd in &inherited {
                let fd = BorrowedFd::borrow_raw(*fd);
                let flags = rustix::io::fcntl_getfd(fd)?;
                rustix::io::fcntl_setfd(fd, flags & !rustix::io::FdFlags::CLOEXEC)?;
            }
            for (resource, limit) in [
                (libc::RLIMIT_CORE, 0),
                (libc::RLIMIT_FSIZE, document::MAX_DOCUMENT_BYTES),
                (libc::RLIMIT_AS, 1024 * 1024 * 1024),
                (libc::RLIMIT_CPU, 30),
            ] {
                let value = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                if libc::setrlimit(resource, &value) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            Ok(())
        });
    }
    let mut child = Running(command.spawn()?);
    let mut errors = child.0.stderr.take().expect("piped converter diagnostics");
    let flags = rustix::fs::fcntl_getfl(&errors)?;
    rustix::fs::fcntl_setfl(&errors, flags | rustix::fs::OFlags::NONBLOCK)?;
    let mut count = 0;
    let mut buffer = [0; 4096];
    loop {
        cancel.check()?;
        loop {
            match errors.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes) => {
                    count += bytes;
                    anyhow::ensure!(
                        count <= STDERR_BYTES,
                        "Print converter diagnostics exceed limits"
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(status) = child.0.try_wait()? {
            anyhow::ensure!(status.success(), "Print converter failed ({status})");
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cancel.check()?;
    Ok(())
}

pub(crate) fn transform(
    source: File,
    target: File,
    request: Request,
    cancel: &Cancellation,
) -> anyhow::Result<Document> {
    request.validate()?;
    // Pin the executable inode, including after an upgrade has unlinked its
    // old pathname. Reopening current_exe() could load a different executable.
    let executable = File::open("/proc/self/exe")?;
    let input = document::verify(&source)?;
    anyhow::ensure!(
        target.metadata()?.len() == 0 && rustix::fs::fcntl_get_seals(&target)?.is_empty(),
        "Print render target is not a fresh sealable file"
    );
    let normalized = stage()?;
    let mut command = sandbox();
    match input.format {
        Some(Format::Pdf | Format::Ps) => {
            command.args([
                "--",
                "/usr/bin/gs",
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-dAutoRotatePages=/None",
                // Rust reconstructs pages from their content streams. Flatten
                // printable annotations first so their appearances survive;
                // screen-only annotations are not part of a printed page.
                "-dPrinted",
                "-dPreserveAnnots=false",
                "-sDEVICE=pdfwrite",
                "-sstdout=%stderr",
                "-sOutputFile=%stdout",
                "-f",
                "-",
            ]);
        }
        Some(Format::Svg) => {
            command.args(["--", "/usr/bin/rsvg-convert", "--format=pdf"]);
        }
        None => anyhow::bail!("The document format cannot be rendered"),
    }
    run(command, &source, &normalized, cancel, &[])?;
    let normalized_document = document::finish(&normalized)?;
    anyhow::ensure!(
        normalized_document.format == Some(Format::Pdf),
        "Print converter did not produce a PDF"
    );
    let pdf = if request.format == Format::Pdf {
        target.try_clone()?
    } else {
        stage()?
    };
    let mut command = sandbox();
    let mut inherited = vec![&executable];
    if request.mode == Mode::Copies && input.format == Some(Format::Pdf) {
        // Ghostscript flattens print appearances but rounds MediaBox to .01pt.
        // The same isolated qpdf worker reads the frozen original's geometry;
        // it receives no caller path or additional application authority.
        command.args([
            "--setenv",
            "CANOPA_PRINT_ORIGINAL_PDF_FD",
            &source.as_raw_fd().to_string(),
        ]);
        inherited.push(&source);
    }
    command.args([
        OsString::from("--setenv"),
        "CANOPA_PRINT_LAYOUT".into(),
        serde_json::to_string(&request)?.into(),
        "--".into(),
        // Exec the retained inode directly. bind-fd resolves its pathname and
        // cannot mount it after unlink; proc's FD magic link remains usable.
        format!("/proc/self/fd/{}", executable.as_raw_fd()).into(),
        "--print-layout-worker".into(),
    ]);
    run(command, &normalized, &pdf, cancel, &inherited)?;
    let result = document::finish(&pdf)?;
    anyhow::ensure!(
        result.format == Some(Format::Pdf),
        "PDF layout did not produce a PDF"
    );
    if request.format != Format::Pdf {
        let mut command = sandbox();
        match request.format {
            Format::Ps => {
                command.args([
                    "--",
                    "/usr/bin/gs",
                    "-q",
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-dAutoRotatePages=/None",
                    "-sDEVICE=ps2write",
                    "-sstdout=%stderr",
                    "-sOutputFile=%stdout",
                    "-f",
                    "-",
                ]);
            }
            Format::Svg => {
                command.args(["--", "/usr/bin/pdftocairo", "-svg", "/proc/self/fd/0", "-"]);
            }
            Format::Pdf => unreachable!(),
        }
        run(command, &pdf, &target, cancel, &[])?;
        let result = document::finish(&target)?;
        anyhow::ensure!(
            result.format == Some(request.format),
            "Print converter returned another format"
        );
        return Ok(result);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

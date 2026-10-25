//! Private-file feasibility proof; built by scripts/prove-uutils-copy.py.
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStringExt,
            fs::{MetadataExt, PermissionsExt, symlink},
        },
    },
    path::PathBuf,
    time::Instant,
};
fn transfer(input: &mut File, output: &mut File, cancel: bool) -> io::Result<u64> {
    let mut total = 0;
    uucore::buf_copy::copy_with_progress(input, output, |n| {
        total += n;
        if cancel && total >= 65536 {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            Ok(())
        }
    })
}
fn main() -> io::Result<()> {
    let root = PathBuf::from(std::env::args_os().nth(1).unwrap());
    if std::env::args().any(|a| a == "hold") {
        let mut input = File::open(root.join("large"))?;
        let mut output = File::create(root.join("killed-output"))?;
        return uucore::buf_copy::copy_with_progress(&mut input, &mut output, |n| {
            if n > 0 {
                fs::write(root.join("written"), b"ready")?;
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            Ok(())
        })
        .map(|_| ());
    }
    fs::create_dir(root.join("parent"))?;
    uucore::error::set_exit_code(73);
    let raw = std::ffi::OsString::from_vec(b"raw-\xff".to_vec());
    let source = root.join("parent").join(raw);
    fs::write(&source, b"selected bytes")?;
    let mut input = File::open(&source)?;
    let mut output = File::create(root.join("copied"))?;
    fs::rename(root.join("parent"), root.join("held-parent"))?;
    fs::create_dir(root.join("parent"))?;
    fs::write(&source, b"unrelated bytes")?;
    fs::set_permissions(root.join("held-parent"), fs::Permissions::from_mode(0o000))?;
    assert_eq!(transfer(&mut input, &mut output, false)?, 14);
    assert_eq!(fs::read(root.join("copied"))?, b"selected bytes");
    assert_eq!(fs::read(&source)?, b"unrelated bytes");
    fs::set_permissions(root.join("held-parent"), fs::Permissions::from_mode(0o700))?;
    // The path cp API treats a descriptor spelling as a symlink by default.
    let proc_source = PathBuf::from(format!("/proc/self/fd/{}", input.as_raw_fd()));
    let proc_dest = root.join("cp-proc");
    uu_cp::copy(&[proc_source], &proc_dest, &uu_cp::Options::default()).unwrap();
    assert!(proc_dest.symlink_metadata()?.is_symlink());
    symlink(
        std::ffi::OsString::from_vec(b"missing-\xfe".to_vec()),
        root.join("link"),
    )?;
    assert_eq!(
        fs::read_link(root.join("link"))?
            .as_os_str()
            .as_encoded_bytes(),
        b"missing-\xfe"
    );
    let mut large = File::create(root.join("large"))?;
    let chunk = vec![0x61; 1024 * 1024];
    for _ in 0..128 {
        large.write_all(&chunk)?;
    }
    drop(large);
    let mut input = File::open(root.join("large"))?;
    let mut output = File::create(root.join("cancelled"))?;
    let start = Instant::now();
    assert_eq!(
        transfer(&mut input, &mut output, true).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    let cancel_micros = start.elapsed().as_micros();
    assert!(output.metadata()?.len() > 0 && output.metadata()?.len() <= 65536);
    // A failed output and a cancelled call must not contaminate the next call.
    let mut readonly = File::open(root.join("copied"))?;
    assert!(transfer(&mut input, &mut readonly, false).is_err());
    let mut input = File::open(root.join("large"))?;
    let mut output = File::create(root.join("large-output"))?;
    let start = Instant::now();
    let mut updates = 0;
    let copied = uucore::buf_copy::copy_with_progress(&mut input, &mut output, |n| {
        if n > 0 {
            updates += 1;
        }
        Ok(())
    })?;
    let copy_micros = start.elapsed().as_micros();
    assert_eq!(copied, 128 * 1024 * 1024);
    assert!(updates > 1);
    let sparse = File::create(root.join("sparse"))?;
    sparse.set_len(8 * 1024 * 1024)?;
    let mut input = File::open(root.join("sparse"))?;
    let mut output = File::create(root.join("sparse-output"))?;
    transfer(&mut input, &mut output, false)?;
    let allocated = output.metadata()?.blocks() * 512;
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    File::open(root.join("sparse-output"))?.read_to_end(&mut bytes)?;
    assert!(bytes.iter().all(|b| *b == 0));
    assert_eq!(uucore::error::get_exit_code(), 73);
    println!(
        "{{\"bytes\":{copied},\"progress_updates\":{updates},\"copy_us\":{copy_micros},\"cancel_us\":{cancel_micros},\"sparse_allocated\":{allocated},\"cp_proc_is_symlink\":true}}"
    );
    Ok(())
}

//! Packaged, display-free check of the linked backend using private files only.
use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{fs, io};

pub(super) fn run() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let output = root.path().join("output");
    let moved = root.path().join("moved");
    for path in [&source, &output, &moved] {
        fs::create_dir(path)?;
    }
    fs::write(source.join("first"), b"completed prefix")?;
    fs::write(source.join("large"), vec![0x61; 256 * 1024])?;
    let history = root.path().join("history");
    let request = TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![source.join("first"), source.join("large")],
        destination: output.clone(),
    };
    let engine = OperationEngine::open(&history)?;
    let result = engine.transfer(&request, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { path, bytes, .. } if path.ends_with("large") && bytes > 16) {
            TransferReply::Cancel
        } else {
            TransferReply::Continue
        }
    })?;
    require(result.cancelled && result.completed == 1 && result.failures.is_empty())?;
    require(output.join("first").is_file() && !output.join("large").exists())?;
    drop(engine);
    let engine = OperationEngine::open(&history)?;
    require(engine.undo_label().is_some())?;
    engine.undo(&Cancellation::default())?;
    require(!output.join("first").exists())?;
    let result = engine.transfer(&request, &Cancellation::default(), |_| {
        TransferReply::Continue
    })?;
    require(result.completed == 2 && result.failures.is_empty())?;
    require(fs::read(source.join("large"))? == fs::read(output.join("large"))?)?;
    let result = engine.transfer(
        &TransferRequest {
            kind: TransferKind::Move,
            sources: vec![source.join("first")],
            destination: moved.clone(),
        },
        &Cancellation::default(),
        |_| TransferReply::Continue,
    )?;
    require(result.completed == 1 && !source.join("first").exists())?;
    engine.undo(&Cancellation::default())?;
    require(fs::read(source.join("first"))? == b"completed prefix")?;
    engine.undo(&Cancellation::default())?;
    require(fs::read_dir(&output)?.next().is_none())?;
    drop(engine);
    require(OperationEngine::open(history)?.undo_label().is_none())?;
    println!(
        "Private linked copy backend: Copy, Move, in-file cancellation, reopen and Undo passed"
    );
    Ok(())
}

fn require(condition: bool) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other("private copy backend check failed"))
    }
}

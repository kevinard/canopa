//! Private-file before/after measurements, including the unchanged SHA-256 receipt.
use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    time::Instant,
};
fn main() {
    let root = tempfile::tempdir().unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let source = root.path().join("large");
    let mut file = fs::File::create(&source).unwrap();
    let block = vec![0x61; 1024 * 1024];
    for _ in 0..128 {
        file.write_all(&block).unwrap();
    }
    drop(file);
    let output = root.path().join("output");
    fs::create_dir(&output).unwrap();
    let start = Instant::now();
    let mut updates = 0;
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: output.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Progress { .. }) {
                    updates += 1;
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    assert_eq!(result.completed, 1);
    let large_us = start.elapsed().as_micros();
    let small = root.path().join("small");
    fs::create_dir(&small).unwrap();
    for n in 0..100 {
        fs::write(small.join(format!("{n:04}")), b"small file").unwrap();
    }
    let start = Instant::now();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![small],
                destination: output.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
    let small_us = start.elapsed().as_micros();
    let cancel_output = root.path().join("cancel-output");
    fs::create_dir(&cancel_output).unwrap();
    let cancel = Cancellation::default();
    let start = Instant::now();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: cancel_output,
            },
            &cancel,
            |event| match event {
                TransferEvent::Conflict { .. } => TransferReply::Resolve {
                    choice: canopa_files::transfers::ConflictChoice::KeepBoth,
                    apply_to_remaining: false,
                },
                TransferEvent::Progress { bytes, .. } if bytes > 0 => TransferReply::Cancel,
                _ => TransferReply::Continue,
            },
        )
        .unwrap();
    assert!(result.cancelled);
    let cancel_us = start.elapsed().as_micros();
    // Component observations on the same private source and filesystem. These
    // are separate warm-cache samples, not a subtraction from the engine run.
    let mut input = fs::File::open(root.path().join("large")).unwrap();
    let mut output = fs::File::create(root.path().join("component-output")).unwrap();
    let start = Instant::now();
    uucore::buf_copy::copy_with_progress(&mut input, &mut output, |_| Ok(())).unwrap();
    let mover_us = start.elapsed().as_micros();
    let start = Instant::now();
    output.sync_all().unwrap();
    let sync_us = start.elapsed().as_micros();
    let hash_file = |path| {
        let mut input = fs::File::open(path).unwrap();
        let start = Instant::now();
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        std::hint::black_box(hash.finalize());
        start.elapsed().as_micros()
    };
    let source_hash_us = hash_file(root.path().join("large"));
    let output_hash_us = hash_file(root.path().join("component-output"));
    println!(
        "{{\"large_128mib_us\":{large_us},\"small_100_us\":{small_us},\"cancel_us\":{cancel_us},\"updates\":{updates},\"mover_us\":{mover_us},\"source_hash_us\":{source_hash_us},\"output_hash_us\":{output_hash_us},\"sync_us\":{sync_us}}}"
    );
}

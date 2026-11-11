//! Headless Copy throughput probe for `scripts/measure-files-copy.py`.
//!
//! `copy_throughput HISTORY DESTINATION SOURCE...` copies the sources with the
//! production engine and prints one JSON line. The destination must be a private
//! benchmark folder; the history folder is created when missing.
use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{path::PathBuf, time::Instant};

fn main() {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let (Some(history), Some(destination)) = (args.next(), args.next()) else {
        eprintln!("usage: copy_throughput HISTORY DESTINATION SOURCE...");
        std::process::exit(2);
    };
    let sources: Vec<_> = args.collect();
    assert!(!sources.is_empty(), "at least one source is required");
    let opened = Instant::now();
    let engine = OperationEngine::open(&history).expect("open operation history");
    let open_us = opened.elapsed().as_micros();
    let (mut events, mut bytes, mut completed) = (0u64, 0u64, 0usize);
    let start = Instant::now();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources,
                destination,
            },
            &Cancellation::default(),
            |event| {
                if let TransferEvent::Progress {
                    completed: done,
                    bytes: total,
                    ..
                } = event
                {
                    events += 1;
                    bytes = total;
                    completed = done;
                }
                TransferReply::Continue
            },
        )
        .expect("transfer");
    let elapsed_us = start.elapsed().as_micros();
    assert!(
        result.failures.is_empty(),
        "failures: {:?}",
        result.failures
    );
    assert!(!result.cancelled);
    println!(
        "{{\"open_us\":{open_us},\"elapsed_us\":{elapsed_us},\"completed\":{},\"progress_completed\":{completed},\"bytes\":{bytes},\"events\":{events}}}",
        result.completed
    );
}

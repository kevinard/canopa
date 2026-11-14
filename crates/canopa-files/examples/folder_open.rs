//! Large-folder opening probe (ADR0177).
//!
//! `folder_open create COUNT FOLDER` fills a new private FOLDER with COUNT
//! empty files. `folder_open read FOLDER [ROUNDS]` times the browser's folder
//! snapshot (read, per-entry metadata and sort) and a filename search over it,
//! printing one JSON line per round. Run it on a release build.
use canopa_files::{
    Cancellation,
    browse::Directory,
    search::{SearchRequest, SearchScope},
};
use std::{path::PathBuf, time::Instant};

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|mode| mode.to_str()) {
        Some("create") if args.len() == 3 => {
            let count: usize = args[1]
                .to_str()
                .and_then(|count| count.parse().ok())
                .expect("COUNT is a number");
            let folder = PathBuf::from(&args[2]);
            std::fs::create_dir(&folder).expect("FOLDER must not exist");
            for index in 0..count {
                std::fs::File::create(folder.join(format!("item-{index:06}.txt")))
                    .expect("create item");
            }
        }
        Some("read") if args.len() >= 2 => {
            let folder = PathBuf::from(&args[1]);
            let rounds: usize = args
                .get(2)
                .and_then(|rounds| rounds.to_str()?.parse().ok())
                .unwrap_or(3);
            for round in 0..rounds {
                let start = Instant::now();
                let directory = Directory::read_cancellable(&folder, &Cancellation::default())
                    .expect("read folder");
                let read_us = start.elapsed().as_micros();
                let start = Instant::now();
                let results = SearchRequest::new(&folder, "item-00", SearchScope::CurrentFolder)
                    .run(&Cancellation::default());
                let search_us = start.elapsed().as_micros();
                println!(
                    "{{\"round\":{round},\"entries\":{},\"read_us\":{read_us},\"search_matches\":{},\"search_us\":{search_us}}}",
                    directory.entries.len(),
                    results.entries.len()
                );
            }
        }
        _ => {
            eprintln!("usage: folder_open create COUNT FOLDER | folder_open read FOLDER [ROUNDS]");
            std::process::exit(2);
        }
    }
}

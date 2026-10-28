//! Native (non-browser) benchmarks for the pure logic of Log Lens.
//!
//! Input files are produced by `tools/logs_gen.py` (seed 42, deterministic) so the
//! numbers are comparable between machines and with the browser benchmarks.
//! The easiest way to run everything is:
//!
//!     python tools/bench_native.py
//!
//! which generates the inputs, runs `cargo bench --bench native` and writes the
//! results table into README.md.

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use log_lens::filter::Criteria;
use log_lens::parser::{LogEntry, PathTable, parse_all};
use log_lens::stats::{hourly, summarize, top_endpoints};

/// Number of log lines per input file.
const SIZES: [usize; 3] = [10_000, 100_000, 500_000];

fn input_path(lines: usize) -> PathBuf {
    let dir = std::env::var("LOG_LENS_BENCH_DIR").unwrap_or_else(|_| "samples".into());
    PathBuf::from(dir).join(format!("bench_{lines}.log"))
}

fn load(lines: usize) -> String {
    let path = input_path(lines);
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "{} not found. Generate it with:\n  python tools/logs_gen.py {lines} > {}\n\
             or simply run: python tools/bench_native.py",
            path.display(),
            path.display()
        )
    })
}

/// Same logic as the `filtered` Memo in app.rs: indices of matching entries.
fn filter_indices(entries: &[LogEntry], paths: &PathTable, criteria: &Criteria) -> Vec<usize> {
    let matcher = criteria.matcher(paths);
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| matcher.matches(e))
        .map(|(i, _)| i)
        .collect()
}

fn benches(c: &mut Criterion) {
    // A typical "investigate an incident" filter: 5xx responses on checkout.
    let criteria = Criteria {
        status_class: Some(5),
        method: None,
        path_contains: "checkout".into(),
        min_ms: None,
    };
    // What typing into the path search box does: a substring test on every entry.
    let path_only = Criteria {
        path_contains: "checkout".into(),
        ..Default::default()
    };

    for &lines in &SIZES {
        let text = load(lines);
        let parsed = parse_all(&text);
        let (entries, paths) = (&parsed.entries, &parsed.paths);
        assert_eq!(entries.len(), lines, "unexpected number of parsed lines");

        let mut group = c.benchmark_group(format!("{lines}"));
        group.throughput(Throughput::Elements(lines as u64));
        // Large inputs take long per iteration; fewer samples keep the run short.
        group.sample_size(if lines >= 500_000 { 10 } else { 30 });

        group.bench_with_input(BenchmarkId::from_parameter("parse"), &text, |b, t| {
            b.iter(|| parse_all(black_box(t)))
        });
        group.bench_with_input(BenchmarkId::from_parameter("filter"), entries, |b, e| {
            b.iter(|| filter_indices(black_box(e), paths, black_box(&criteria)))
        });
        group.bench_with_input(BenchmarkId::from_parameter("filter_path"), entries, |b, e| {
            b.iter(|| filter_indices(black_box(e), paths, black_box(&path_only)))
        });
        group.bench_with_input(BenchmarkId::from_parameter("summarize"), entries, |b, e| {
            b.iter(|| summarize(black_box(e).iter()))
        });
        group.bench_with_input(BenchmarkId::from_parameter("hourly"), entries, |b, e| {
            b.iter(|| hourly(black_box(e).iter()))
        });
        group.bench_with_input(BenchmarkId::from_parameter("top_endpoints"), entries, |b, e| {
            b.iter(|| top_endpoints(black_box(e).iter(), paths, 8))
        });
        group.finish();
    }
}

criterion_group!(native, benches);
criterion_main!(native);

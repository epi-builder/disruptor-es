//! External-process HTTP benchmark lane.
//!
//! This benchmark measures real client-to-`app serve` overhead, not in-process
//! runtime composition or ring-only handoff.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use tokio::runtime::Runtime;

fn external_process_http_smoke(criterion: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");

    // Criterion stays as a short smoke comparison lane; `app http-stress`
    // remains the authoritative Phase 13 steady-state report path.
    criterion.bench_function("external_process_http_smoke", |bench| {
        bench.iter(|| {
            let report = runtime
                .block_on(app::http_stress::run_external_process_http_stress(
                    app::http_stress::HttpStressConfig::from_profile(
                        app::http_stress::HttpStressProfile::Smoke,
                    ),
                ))
                .expect("external-process HTTP stress run");
            black_box(report.throughput_per_second);
        });
    });
}

criterion_group!(external_process_http, external_process_http_smoke);
criterion_main!(external_process_http);

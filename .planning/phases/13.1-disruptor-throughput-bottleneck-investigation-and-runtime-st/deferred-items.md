# Deferred Items

- `2026-04-26`: `cargo test -p app serve -- --nocapture` currently fails outside this plan's scope because [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:1285) initializes `StressReport` without the newer metrics fields. Verified this plan's `serve` wiring with `cargo check -p app --bin app` instead.

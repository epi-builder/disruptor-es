---
status: resolved
trigger: "Investigate issue: prometheus-scrape-failures"
created: 2026-04-27T00:00:00+09:00
updated: 2026-04-27T09:02:12+09:00
---

## Current Focus

hypothesis: confirmed and fixed; the Prometheus exporter now starts its listener and the harness treats missing metrics readiness as a hard failure
test: record orchestrator confirmation and archive the resolved session
expecting: persisted session state and repo history reflect the verified fix without including unrelated user changes
next_action: move this session into `.planning/debug/resolved/`, append the knowledge base entry, and commit only the Prometheus scrape fix artifacts

## Symptoms

expected: External-process `app http-stress` should start `app serve` with `APP_PROMETHEUS_LISTEN`, verify the metrics endpoint is reachable, scrape `/metrics` during the measurement window, and report `metrics_scrape_successes > 0` for baseline live HTTP artifacts.
actual: Regenerated Phase 13.1 artifacts report `metrics_scrape_successes = 0` and all scrape attempts failed. Examples: `live-http-unique.json`, `live-http-shard-1.json`, and `live-http-shard-8.json` each have `metrics_scrape_failures = 121`, `metrics_sample_count = 121`, `ingress_depth_max = null`, and live HTTP commands still succeeded.
errors: No detailed scrape error is preserved in the JSON artifacts. Code currently increments failure counts and discards the error in `record_metrics_scrape_result`; `wait_for_metrics_body(...).await.ok()` also suppresses baseline scrape failure.
reproduction: Run `PHASE13_1_COMPARE_MODE=baseline bash scripts/compare-stress-layers.sh` or a shorter direct command such as `cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16`, then inspect `metrics_scrape_successes` in the JSON output.
started: The problem was found after Phase 13.1 gap closure evidence regeneration on 2026-04-27. Earlier fix separated observed metrics from estimated fallbacks, which made the zero-scrape problem visible instead of hiding it behind synthetic queue-depth values.

## Eliminated

## Evidence

- timestamp: 2026-04-27T00:00:00+09:00
  checked: target/phase-13.1/layer-comparison/live-http-unique.json, live-http-shard-1.json, live-http-shard-8.json
  found: all cited baseline artifacts report `metrics_scrape_successes = 0`, `metrics_scrape_failures = metrics_sample_count = 121`, and scrape-derived fields remain null while command traffic succeeds
  implication: the failure is specific to the metrics observation path, not the main HTTP command path

- timestamp: 2026-04-27T00:00:00+09:00
  checked: crates/app/src/http_stress.rs, crates/app/src/serve.rs, crates/app/src/observability.rs
  found: the child process is started with a separate `APP_PROMETHEUS_LISTEN` address, readiness waits only on `/healthz`, baseline metrics readiness is downgraded with `.ok()`, and scrape failures are counted but their error details are discarded
  implication: the harness can proceed even when the Prometheus listener is broken or never became reachable, and current artifacts cannot explain why

- timestamp: 2026-04-27T08:55:19+09:00
  checked: `cargo run -q -p app -- http-stress --profile smoke ...` and local `metrics-exporter-prometheus 0.18.1` docs/source
  found: the smoke repro still reports `metrics_scrape_successes = 0` and `metrics_scrape_failures = 9`; the exporter crate documents and implements `install_recorder()` as recorder-only, while `install()`/`build()` are the APIs that spawn the HTTP listener and upkeep task
  implication: the Prometheus endpoint was never started in `app serve`; the harness symptoms are a direct consequence of using the wrong exporter bootstrap API, not just a readiness race

- timestamp: 2026-04-27T09:00:36+09:00
  checked: `cargo test -p app prometheus_listener_uses_exporter_install_path`, `cargo test -p app metrics_scrape_failures_are_counted`, `cargo test -p app zero_successful_metrics_scrapes_return_an_error`, `cargo test -p app external_process_http_stress_smoke`
  found: targeted regression tests passed, including the external-process smoke test that now asserts successful metrics scraping
  implication: the code fix restored the `/metrics` listener in the real child-process path and the harness no longer silently accepts zero-success scrape runs

- timestamp: 2026-04-27T09:00:36+09:00
  checked: `cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16`
  found: the CLI now returns `metrics_scrape_successes = 9`, `metrics_scrape_failures = 0`, `ingress_depth_max = 1`, `projection_lag = 0`, and `outbox_lag = 0`
  implication: the original zero-scrape symptom is fixed in the direct external-process HTTP path

- timestamp: 2026-04-27T09:02:12+09:00
  checked: orchestrator human-verify checkpoint response for `cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16`
  found: independent rerun was confirmed fixed with `metrics_scrape_successes = 9`, `metrics_scrape_failures = 0`, and `ingress_depth_max = 1`
  implication: the fix holds in the orchestrator-controlled verification flow, so the session can be finalized and archived

## Resolution

root_cause: `crates/app/src/observability.rs` installs only a Prometheus recorder via `PrometheusBuilder::install_recorder()`. That API does not start the HTTP scrape listener, so `APP_PROMETHEUS_LISTEN` is configured but `/metrics` is never served. The HTTP stress harness then masks startup diagnosis by treating the baseline metrics probe as optional and by discarding scrape errors.
fix: switched observability bootstrap from `PrometheusBuilder::install_recorder()` to `install()`, added harness verification that `/metrics` becomes ready before measurement, and captured the last scrape error so zero-success runs fail loudly instead of silently generating misleading artifacts
verification:
  - `cargo test -p app prometheus_listener_uses_exporter_install_path -- --nocapture`
  - `cargo test -p app metrics_scrape_failures_are_counted -- --nocapture`
  - `cargo test -p app zero_successful_metrics_scrapes_return_an_error -- --nocapture`
  - `cargo test -p app external_process_http_stress_smoke -- --nocapture`
  - `cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16` now reports `metrics_scrape_successes = 9` and `metrics_scrape_failures = 0`
  - orchestrator reran `cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16` and confirmed `metrics_scrape_successes = 9`, `metrics_scrape_failures = 0`, and `ingress_depth_max = 1`
files_changed:
  - crates/app/src/observability.rs
  - crates/app/src/serve.rs
  - crates/app/src/http_stress.rs

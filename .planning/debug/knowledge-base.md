# GSD Debug Knowledge Base

Resolved debug sessions. Used by `gsd-debugger` to surface known-pattern hypotheses at the start of new investigations.

---

## prometheus-scrape-failures — Prometheus metrics endpoint never started for external-process HTTP stress runs
- **Date:** 2026-04-27
- **Error patterns:** prometheus scrape failures, metrics_scrape_successes = 0, metrics_scrape_failures, ingress_depth_max = null, APP_PROMETHEUS_LISTEN, /metrics never served
- **Root cause:** `crates/app/src/observability.rs` used `PrometheusBuilder::install_recorder()`, which installs only the recorder and does not start the HTTP listener. The external-process stress harness then hid the startup problem by treating metrics readiness as optional and discarding scrape errors.
- **Fix:** Switched observability bootstrap to `PrometheusBuilder::install()`, required `/metrics` readiness before measurement, and recorded the last scrape error so zero-success scrape windows fail loudly.
- **Files changed:** crates/app/src/observability.rs, crates/app/src/serve.rs, crates/app/src/http_stress.rs
---

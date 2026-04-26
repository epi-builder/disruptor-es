#!/usr/bin/env bash
set -euo pipefail

MODE="${PHASE13_1_COMPARE_MODE:-smoke}"
OUTPUT_DIR="target/phase-13.1/layer-comparison"

case "$MODE" in
  smoke|baseline) ;;
  *)
    printf 'PHASE13_1_COMPARE_MODE must be smoke or baseline, got: %s\n' "$MODE" >&2
    exit 1
    ;;
esac

mkdir -p "$OUTPUT_DIR"

run_text_lane() {
  local output_file="$1"
  shift
  "$@" >"$output_file" 2>&1
}

run_json_lane() {
  local output_file="$1"
  shift
  "$@" >"$output_file"
}

run_text_lane "$OUTPUT_DIR/ring-only.txt" \
  cargo bench --bench ring_only -- --sample-size 10

run_text_lane "$OUTPUT_DIR/adapter-only.txt" \
  cargo bench --bench adapter_only -- --sample-size 10

run_text_lane "$OUTPUT_DIR/storage-only.txt" \
  cargo bench --bench storage_only -- --sample-size 10

run_json_lane "$OUTPUT_DIR/in-process-runtime.json" \
  cargo run -q -p app -- stress-smoke

if [ "$MODE" = "smoke" ]; then
  run_json_lane "$OUTPUT_DIR/live-http-unique.json" \
    cargo run -q -p app -- http-stress --profile smoke --workload-shape unique --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16

  run_json_lane "$OUTPUT_DIR/live-http-single-hot-key.json" \
    cargo run -q -p app -- http-stress --profile smoke --workload-shape single-hot-key --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 8 --shard-count 2 --ingress-capacity 8 --ring-size 16
else
  run_json_lane "$OUTPUT_DIR/live-http-unique.json" \
    cargo run -q -p app -- http-stress --profile baseline --workload-shape unique --warmup-seconds 5 --measure-seconds 30 --concurrency 8 --shard-count 8 --ingress-capacity 256 --ring-size 256

  run_json_lane "$OUTPUT_DIR/live-http-single-hot-key.json" \
    cargo run -q -p app -- http-stress --profile hot-key --workload-shape single-hot-key --warmup-seconds 3 --measure-seconds 20 --concurrency 16 --shard-count 8 --ingress-capacity 128 --ring-size 256
fi

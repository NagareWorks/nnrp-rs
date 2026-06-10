# Preview4 Release-Prep Benchmark - 2026-06-10

This run records the Rust-owned Preview4 benchmark entrypoint before publishing `1.0.0-preview.4.0`.
Preview3 did not publish a Rust-owned repeatable benchmark entrypoint in this repository; its coarse FFI baseline is preserved by keeping the Preview3 artifact line and implementation notes intact.

## Environment

- Host: Windows development workstation
- Build profile: Cargo dev profile
- Command:

```bash
cargo run -p nnrp-conformance --bin nnrp-preview4-benchmarks -- --iterations 100000 --transport-iterations 1000
```

## Results

| Case | Iterations | Operations | Elapsed ms | Ops/s |
| --- | ---: | ---: | ---: | ---: |
| Control frame hot path | 100000 | 400000 | 323.6435 | 1235927.80 |
| Runtime object declare/ref/release | 100000 | 500000 | 629.3854 | 794425.80 |
| IPC loopback | 1000 | 1000 | 236.4383 | 4229.43 |
| WebSocket loopback | 1000 | 1000 | 379.0496 | 2638.18 |

## Notes

- This is a Rust release-prep smoke benchmark, not a downstream SDK threshold result.
- Preview4 benchmark coverage is additive and does not overwrite the Preview3 coarse FFI artifact line.
- Downstream SDK benchmark comparisons are tracked as Python and JavaScript release gates, using the published Preview4 artifact set they consume.

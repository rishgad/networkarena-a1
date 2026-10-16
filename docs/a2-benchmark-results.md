# Congestion and fairness benchmark

**20/20 scenarios passed:** five topologies at 0.5×, 1×, 1.5×, and 2× offered load.

Customer share is the lowest scored one-second goodput as a percentage of that customer’s benchmark-defined max-min fair share. Aggregate share is summed goodput divided by summed fair shares in each scored window; it is not raw link utilization. Warmup, unsettled bursts, and unscored partial windows follow the simulator’s rules.

| Topology | Load | Status | Min customer share | Min aggregate share | Looping packets | False link-downs |
|---|---:|---|---:|---:|---:|---:|
| a2p1-dumb-004 | 0.5× | PASS | 76.80% | 89.30% | 0 | 0 |
| a2p1-dumb-004 | 1× | PASS | 87.52% | 90.71% | 0 | 0 |
| a2p1-dumb-004 | 1.5× | PASS | 87.12% | 91.28% | 0 | 0 |
| a2p1-dumb-004 | 2× | PASS | 87.04% | 91.72% | 0 | 0 |
| a2p1-grid-009 | 0.5× | PASS | 94.52% | 95.54% | 0 | 0 |
| a2p1-grid-009 | 1× | PASS | 93.53% | 95.70% | 0 | 0 |
| a2p1-grid-009 | 1.5× | PASS | 88.53% | 95.85% | 0 | 0 |
| a2p1-grid-009 | 2× | PASS | 74.13% | 96.06% | 0 | 0 |
| a2p1-ring-020 | 0.5× | PASS | 96.13% | 97.34% | 0 | 0 |
| a2p1-ring-020 | 1× | PASS | 87.60% | 93.68% | 0 | 0 |
| a2p1-ring-020 | 1.5× | PASS | 71.35% | 81.88% | 0 | 0 |
| a2p1-ring-020 | 2× | PASS | 48.00% | 69.61% | 0 | 0 |
| a2p2-dumb-003 | 0.5× | PASS | 83.07% | 89.71% | 0 | 0 |
| a2p2-dumb-003 | 1× | PASS | 76.53% | 84.76% | 0 | 0 |
| a2p2-dumb-003 | 1.5× | PASS | 71.20% | 82.74% | 0 | 0 |
| a2p2-dumb-003 | 2× | PASS | 71.20% | 82.71% | 0 | 0 |
| a2p2-grid-006 | 0.5× | PASS | 80.15% | 84.27% | 0 | 0 |
| a2p2-grid-006 | 1× | PASS | 74.67% | 85.26% | 0 | 0 |
| a2p2-grid-006 | 1.5× | PASS | 83.48% | 95.44% | 0 | 0 |
| a2p2-grid-006 | 2× | PASS | 78.52% | 93.26% | 0 | 0 |

## Reproduction

```sh
cargo build --locked --release --bin competitive_net_sim
cargo build --release --target wasm32-unknown-unknown --manifest-path examples/my_switch/Cargo.toml
python3 scripts/a2_benchmark.py
```

Raw JSON/text cards and input provenance are saved under `benchmark-artifacts/a2/` (ignored by Git). Both executables were rebuilt from source for this run.

- Host: Darwin arm64
- Router SHA-256: `6b16f902a12dbe6ac4c7707421fe5a52c3771b14417e97d4a44ed1c1b53fb362`
- Simulator SHA-256: `fd077273cbc4be185d2c262821ecfe843cf6985bae4c138374ac8cacc6547424`

The suite uses supplied topologies, known customer contracts, and static path placement matched to the reference fair-share policy. It does not establish adaptive traffic engineering, arbitrary-topology fairness, or loop-free recovery after failures.

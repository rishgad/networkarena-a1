# Router simulation results

**20/20 scenarios passed.** Worst recorded per-event recovery: **196 ms** of simulated time.

Five steady-state topologies and fifteen scheduled-failure scenarios. Delivery percentages use the simulator’s scored window, excluding its configured warmup and final in-flight window. Recovery measures the time until affected traffic stops experiencing loss after each failure or restoration. These are simulated-network results, not wall-clock throughput benchmarks.

| Topology | Failure schedule | Status | Delivery | Worst recovery | Looping packets |
|---|---|---|---:|---:|---:|
| practice-dumb-006 | none | PASS | 100.00% | n/a | 0 |
| practice-grid-003 | none | PASS | 100.00% | n/a | 0 |
| practice-hub-009 | none | PASS | 100.00% | n/a | 0 |
| practice-ring-002 | none | PASS | 100.00% | n/a | 0 |
| practice-ring-010 | none | PASS | 100.00% | n/a | 0 |
| part2-dumb-006 | part2-dumb-006-f001 | PASS | 99.30% | 190 ms | 0 |
| part2-dumb-006 | part2-dumb-006-f002 | PASS | 99.46% | 187 ms | 2 |
| part2-dumb-006 | part2-dumb-006-f003 | PASS | 99.34% | 193 ms | 4 |
| part2-grid-003 | part2-grid-003-f001 | PASS | 99.18% | 180 ms | 0 |
| part2-grid-003 | part2-grid-003-f002 | PASS | 99.42% | 190 ms | 0 |
| part2-grid-003 | part2-grid-003-f003 | PASS | 99.34% | 185 ms | 0 |
| part2-hub-009 | part2-hub-009-f001 | PASS | 99.67% | 179 ms | 0 |
| part2-hub-009 | part2-hub-009-f002 | PASS | 99.62% | 163 ms | 0 |
| part2-hub-009 | part2-hub-009-f003 | PASS | 99.61% | 127 ms | 0 |
| part2-ring-002 | part2-ring-002-f001 | PASS | 99.32% | 190 ms | 0 |
| part2-ring-002 | part2-ring-002-f002 | PASS | 99.39% | 172 ms | 0 |
| part2-ring-002 | part2-ring-002-f003 | PASS | 99.49% | 176 ms | 0 |
| part2-ring-010 | part2-ring-010-f001 | PASS | 99.40% | 175 ms | 19 |
| part2-ring-010 | part2-ring-010-f002 | PASS | 99.48% | 195 ms | 8 |
| part2-ring-010 | part2-ring-010-f003 | PASS | 99.37% | 196 ms | 0 |

## Reproduction

```sh
cargo build --release --bin competitive_net_sim
cargo build --release --target wasm32-unknown-unknown --manifest-path examples/my_switch/Cargo.toml
python3 scripts/benchmark.py
```

Host: Darwin arm64. The recorded run rebuilt the router from source and used the existing local simulator executable. The exact executable hashes are recorded below.

- Router SHA-256: `588185732af11f3821212970cc8a20eaf1b1c187f9b5fff4407c940e5aa17548`
- Simulator SHA-256: `dab2d4ad463f50ba663c0cd1282c4a9806afaa9dfc45a8e4b036360553153fbb`

Transient loops are included in the table even when the delivery and recovery criteria pass. Results apply to these topologies and failure schedules; they do not establish behavior under arbitrary congestion, loss, or unequal link costs.

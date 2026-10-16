# Rust Link-State Router and Fair Queuing

[![Router tests](https://github.com/rishgad/networkarena-a1/actions/workflows/ci.yml/badge.svg)](https://github.com/rishgad/networkarena-a1/actions/workflows/ci.yml)

A Rust/WebAssembly switch program combining distributed link-state routing with per-customer packet queues, deficit round-robin scheduling, and capacity-aware path placement. It runs inside the supplied NetworkArena simulator.

The router implementation is [`examples/my_switch/src/lib.rs`](examples/my_switch/src/lib.rs). The simulator, SDK, traffic models, and supplied topology suites are course infrastructure, distinct from the routing and queue-configuration code.

## Implementation

- **Distributed topology:** HELLO neighbor discovery, sequence-numbered link-state advertisements, and deterministic shortest-hop forwarding over bidirectional links.
- **Traffic isolation:** source-prefix classification selects a separate queue for each customer, with buffer caps tuned separately for AIMD and rate-matching senders.
- **Fair scheduling:** configures the simulator's deficit round-robin scheduler with equal customer weights and extra weight for control traffic. The scheduling engine itself belongs to the simulator.
- **Path placement:** considers simple paths up to two hops longer than the shortest and greedily places customers using link capacity and contract demand. This matches the supplied benchmark's fair-share reference policy.
- **Coordinated activation:** each customer's ingress switch announces its chosen path and a future activation time through LSAs; switches install the corresponding source-prefix forwarding entries.

Placement uses known customer contracts and holds paths fixed after activation. It does not measure live demand or dynamically rebalance traffic. See [design tradeoffs](docs/a2-design.md).

## Results and reproduction

[Congestion results](docs/a2-benchmark-results.md) cover five mixed-traffic topologies at 0.5×, 1×, 1.5×, and 2× offered load. [Failure-recovery regression results](docs/a1-benchmark-results.md) cover five steady-state topologies and fifteen failure schedules. Metrics describe simulated networks, not host throughput.

```sh
git clone https://github.com/rishgad/networkarena-a1.git
cd networkarena-a1
rustup target add wasm32-unknown-unknown
cargo build --locked --release --bin competitive_net_sim
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path examples/my_switch/Cargo.toml
python3 scripts/a1_benchmark.py
python3 scripts/a2_benchmark.py
```

Each runner exits unsuccessfully for a failed scenario. The congestion runner also saves JSON report cards, input hashes, and executable hashes under its output directory. GitHub Actions rebuilds both executables, runs simulator/SDK tests and both suites, and includes the result tables in its run summary.

The [original routing baseline](docs/benchmark-results.md) is retained for comparison: 20/20 scenarios passed with 196 ms worst per-event recovery and transient loops in four failure scenarios. Congestion tests and link-failure tests exercise different conditions; zero loops in the former does not establish loop-free failure recovery.

## Repository guide

| Path | Purpose |
|---|---|
| `examples/my_switch/` | Router, queue configuration, and path placement |
| `scripts/a1_benchmark.py` | Failure-recovery regression runner |
| `scripts/a2_benchmark.py` | Congestion and fairness load sweep |
| `docs/a2-design.md` | Queue sizing, scheduling, placement, and limitations |
| `docs/a2-benchmark-results.md` | Recorded congestion results |
| `worlds/a1/`, `worlds/practice/` | Supplied routing and congestion scenarios |
| `src/`, `switch_program_sdk/`, `switch_program_types/` | Simulator and SDK infrastructure |
| `STARTER_KIT.md`, `STARTER_KIT_A2.md` | Original starter-kit instructions |

Generated builds, simulation traces, and JSON benchmark artifacts are ignored by Git. Raw course handouts and local study cards are excluded from this update.

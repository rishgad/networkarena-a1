# Rust Link-State Router

A Rust routing program that discovers neighbors, floods topology updates, and installs shortest-hop forwarding routes in the NetworkArena simulator. It detects silent link failures with periodic heartbeats and updates forwarding entries when routes change.

**Baseline:** 100% scored delivery across five steady-state topologies, 99.18–99.67% delivery across fifteen failure scenarios, and **196 ms worst per-event recovery** in simulated time. See the [complete results](docs/benchmark-results.md), including transient loops.

## Implementation

The completed router is [`examples/my_switch/src/lib.rs`](examples/my_switch/src/lib.rs). The simulator, SDK, example programs, and supplied topology suite are supporting infrastructure from the provided NetworkArena starter kit; they are distinct from the router implementation.

- **Neighbor discovery:** HELLO messages every 50 ms identify the switch behind each port.
- **Failure detection:** 150 ms without a neighbor heartbeat expires the link.
- **Topology dissemination:** sequence-numbered link-state advertisements suppress duplicate and outdated updates; periodic refreshes reannounce local state.
- **Route computation:** deterministic BFS selects shortest-hop paths over bidirectional links. The evaluated networks use equal link costs.
- **Forwarding updates:** only changed entries are installed or withdrawn. Outdated entries are deleted before replacements are installed to avoid stale forwarding.

## Build

Requires Rust with Cargo, the WebAssembly target, and Python 3 for the benchmark runner.

```sh
git clone https://github.com/rishgad/networkarena-a1.git
cd networkarena-a1
rustup target add wasm32-unknown-unknown
cargo build --release --bin competitive_net_sim
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path examples/my_switch/Cargo.toml
```

## Run a failure scenario

```sh
./target/release/competitive_net_sim run-world \
  worlds/practice/part2-ring-002.toml \
  --failures worlds/practice/failures/part2-ring-002-f001.toml \
  --program examples/my_switch/target/wasm32-unknown-unknown/release/my_switch.wasm \
  --score
```

## Reproduce the benchmark

```sh
python3 scripts/benchmark.py --output docs/benchmark-current.md
```

The runner evaluates five steady-state scenarios and fifteen scheduled-failure scenarios, then writes delivery, recovery, status, and loop counts to a Markdown table. It exits unsuccessfully if a scenario fails or cannot be evaluated. `--simulator` and `--program` accept alternative executable paths.

The checked-in [baseline report](docs/benchmark-results.md) records the exact router and simulator executable hashes. Recovery measures when affected traffic stops experiencing loss after a link failure or restoration. It is not a measurement of host execution speed or throughput.

## Known limitations

- Transient routing loops occurred in four failure scenarios; their packet counts remain visible in the results.
- BFS optimizes hop count, not unequal link weights.
- Heartbeat timing trades recovery latency against control traffic and tolerance of missed messages.
- The baseline evaluates the supplied topologies and failure schedules, not arbitrary congestion, packet loss, or large-network scale.

## Repository guide

| Path | Purpose |
|---|---|
| `examples/my_switch/` | Completed, buildable router |
| `scripts/benchmark.py` | Reproducible simulation benchmark runner |
| `docs/benchmark-results.md` | Recorded baseline |
| `worlds/practice/` | Supplied topology and failure scenarios |
| `worlds/stress/` | Additional local scenarios, outside the recorded baseline |
| `reference/` | Reference implementation, preserved draft, and analysis notes |
| `src/`, `switch_program_sdk/`, `switch_program_types/` | Simulator and SDK infrastructure |
| `viewer/` | Simulation replay viewer |

The original [starter-kit instructions](STARTER_KIT.md) and [simulator documentation](docs/README.md) are retained for reference. Generated build artifacts and simulation logs are ignored by Git.

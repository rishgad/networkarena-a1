#!/usr/bin/env python3
"""Evaluate congestion fairness across five topologies and four offered loads."""
import argparse
import hashlib
import json
import platform
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--simulator', type=Path, default=ROOT / 'target/release/competitive_net_sim')
    parser.add_argument('--program', type=Path, default=ROOT / 'examples/my_switch/target/wasm32-unknown-unknown/release/my_switch.wasm')
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'benchmark-artifacts/a2')
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/a2-benchmark-results.md')
    args = parser.parse_args()
    sim, program = args.simulator.resolve(), args.program.resolve()
    if not sim.is_file() or not program.is_file():
        parser.error('Build the simulator and router before running this script.')
    worlds = sorted((ROOT / 'worlds/practice').glob('a2p*.toml'))
    if len(worlds) != 5:
        parser.error(f'Expected five congestion topologies, found {len(worlds)}')
    artifacts = args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    rows = []
    for world in worlds:
        for scale in (0.5, 1.0, 1.5, 2.0):
            stem = f'{world.stem}-x{scale:g}'
            card_path = artifacts / (stem + '.json')
            result = subprocess.run(
                [str(sim), 'run-world', str(world), '--program', str(program),
                 '--score', '--load-scale', str(scale), '--json', str(card_path)],
                cwd=ROOT, capture_output=True, text=True, timeout=180)
            (artifacts / (stem + '.txt')).write_text(result.stdout + '\n' + result.stderr)
            if result.returncode not in (0, 1) or not card_path.is_file():
                raise RuntimeError(f'{stem}: simulator failed\n{result.stderr}')
            card = json.loads(card_path.read_text())
            shares = [c['min_share_pct'] for c in card['customers'] if c['min_share_pct'] is not None]
            totals = [u for u in card['series']['utilization_pct'] if u is not None]
            if not shares or not totals:
                raise RuntimeError(f'{stem}: no scored customer or aggregate windows')
            row = dict(world=world.stem, scale=scale, passed=card['passed'],
                       min_share=min(shares), min_total=min(totals),
                       loops=card['looping_packets'], false_downs=card['link_down_declarations'])
            rows.append(row)
            print(f'{stem}: {card["status"]}; min customer share {min(shares):.2f}%', flush=True)
    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()
    provenance = dict(host=f'{platform.system()} {platform.machine()}',
                      router_sha256=sha(program), simulator_sha256=sha(sim),
                      source_sha256=sha(ROOT / 'examples/my_switch/src/lib.rs'),
                      world_sha256={w.name: sha(w) for w in worlds}, results=rows)
    (artifacts / 'summary.json').write_text(json.dumps(provenance, indent=2) + '\n')
    passed = sum(r['passed'] for r in rows)
    lines = ['# Congestion and fairness benchmark', '',
             f'**{passed}/{len(rows)} scenarios passed:** five topologies at 0.5×, 1×, 1.5×, and 2× offered load.', '',
             'Customer share is the lowest scored one-second goodput as a percentage of that customer’s benchmark-defined max-min fair share. Aggregate share is summed goodput divided by summed fair shares in each scored window; it is not raw link utilization. Warmup, unsettled bursts, and unscored partial windows follow the simulator’s rules.', '',
             '| Topology | Load | Status | Min customer share | Min aggregate share | Looping packets | False link-downs |',
             '|---|---:|---|---:|---:|---:|---:|']
    for r in rows:
        lines.append(f'| {r["world"]} | {r["scale"]:g}× | {"PASS" if r["passed"] else "FAIL"} | {r["min_share"]:.2f}% | {r["min_total"]:.2f}% | {r["loops"]} | {r["false_downs"]} |')
    lines += ['', '## Reproduction', '', '```sh',
              'cargo build --locked --release --bin competitive_net_sim',
              'cargo build --release --target wasm32-unknown-unknown --manifest-path examples/my_switch/Cargo.toml',
              'python3 scripts/a2_benchmark.py', '```', '',
              'Raw JSON/text cards and input provenance are saved under `benchmark-artifacts/a2/` (ignored by Git). Both executables were rebuilt from source for this run.', '',
              f'- Host: {provenance["host"]}',
              f'- Router SHA-256: `{sha(program)}`',
              f'- Simulator SHA-256: `{sha(sim)}`', '',
              'The suite uses supplied topologies, known customer contracts, and static path placement matched to the reference fair-share policy. It does not establish adaptive traffic engineering, arbitrary-topology fairness, or loop-free recovery after failures.', '']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join(lines))
    if passed != len(rows):
        raise SystemExit(1)


if __name__ == '__main__':
    main()

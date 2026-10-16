#!/usr/bin/env python3
"""Run the supplied topology/failure suite and save simulated-network results."""
import argparse, hashlib, platform, re, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--simulator', type=Path, default=ROOT / 'target/release/competitive_net_sim')
p.add_argument('--program', type=Path, default=ROOT / 'examples/my_switch/target/wasm32-unknown-unknown/release/my_switch.wasm')
p.add_argument('--output', type=Path, default=ROOT / 'docs/a1-benchmark-results.md')
a = p.parse_args()
sim, program = a.simulator.resolve(), a.program.resolve()
if not sim.is_file() or not program.is_file():
    p.error('Build the simulator and router before running this script.')
cases = [(w, None) for w in sorted((ROOT / 'worlds/a1/practice').glob('practice-*.toml'))]
for f in sorted((ROOT / 'worlds/a1/practice/failures').glob('*.toml')):
    world_name = re.sub(r'-f\d+$', '', f.stem)
    cases.append((ROOT / 'worlds/a1/practice' / (world_name + '.toml'), f))
rows=[]
for world, failure in cases:
    cmd=[str(sim), 'run-world', str(world), '--program', str(program), '--score']
    if failure: cmd.extend(['--failures',str(failure)])
    r=subprocess.run(cmd,cwd=ROOT,text=True,capture_output=True,timeout=120)
    text=r.stdout+'\n'+r.stderr
    status=re.search(r'OVERALL\s+(PASS|FAIL)',text)
    delivery=re.search(r'C1 delivery\s+\w+\s+([\d.]+)%',text)
    worst=re.search(r'worst (\d+) ms',text)
    loops=re.search(r'C3 loops\s+\w+\s+(\d+) packets revisited',text)
    if status is None or delivery is None or loops is None:
        raise RuntimeError(f'Could not read report for {world.name}:\n{text}')
    if r.returncode not in (0,1):raise RuntimeError(text)
    rows.append((world.stem, failure.stem if failure else 'none',status[1],float(delivery[1]),int(worst[1]) if worst else None,int(loops[1])))
    print(world.stem,failure.stem if failure else 'none',status[1],flush=True)
passed=sum(x[2]=='PASS' for x in rows)
recovery=max(x[4] for x in rows if x[4] is not None)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
out=['# Router simulation results','',f'**{passed}/{len(rows)} scenarios passed.** Worst recorded per-event recovery: **{recovery} ms** of simulated time.','',
'Five steady-state topologies and fifteen scheduled-failure scenarios. Delivery percentages use the simulator’s scored window, excluding its configured warmup and final in-flight window. Recovery measures the time until affected traffic stops experiencing loss after each failure or restoration. These are simulated-network results, not wall-clock throughput benchmarks.','',
'| Topology | Failure schedule | Status | Delivery | Worst recovery | Looping packets |','|---|---|---|---:|---:|---:|']
for world,failure,status,delivery,worst,loops in rows:
    out.append(f'| {world} | {failure} | {status} | {delivery:.2f}% | {str(worst)+" ms" if worst is not None else "n/a"} | {loops} |')
out.extend(['','## Reproduction','', '```sh','cargo build --release --bin competitive_net_sim','cargo build --release --target wasm32-unknown-unknown --manifest-path examples/my_switch/Cargo.toml','python3 scripts/a1_benchmark.py','```','',
f'Host: {platform.system()} {platform.machine()}. Both executables were rebuilt from source for this run. Their exact hashes are recorded below.','',f'- Router SHA-256: `{sha(program)}`',f'- Simulator SHA-256: `{sha(sim)}`','',
'Transient loops are included in the table even when the delivery and recovery criteria pass. Results apply to these topologies and failure schedules; they do not establish behavior under arbitrary congestion, loss, or unequal link costs.',''])
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text('\n'.join(out))
if passed != len(rows):raise SystemExit(1)

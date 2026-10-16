#!/usr/bin/env python3
"""Reconstruct a switch's T_ROUTE table at a moment in a run.
usage: table_at.py <simlog> <switch> <time_ms>"""
import subprocess, sys, re, os
log, sw, t_ms = sys.argv[1], sys.argv[2], float(sys.argv[3])
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
out = subprocess.run([os.path.join(root, "target/release/simlog_dump"), log, sw],
                     capture_output=True, text=True).stdout
tbl = {}
for line in out.splitlines():
    m = re.match(r'\s*(\d+) ns  S(\d+)  (install|delete)', line)
    if not m or m.group(2) != sw or int(m.group(1)) / 1e6 > t_ms: continue
    if m.group(3) == "install":
        k = re.search(r'key=0x([0-9a-f]+)/(\d+).*PortIdW\((\d+)\)', line)
        tbl[k.group(1)] = k.group(3)
    else:
        tbl.pop(format(int(re.search(r'entry id=(\d+)', line).group(1)), 'x'), None)
print(f"S{sw} forwarding table at t={t_ms:.0f} ms")
for k in sorted(tbl):
    v = int(k, 16)
    print(f"  key=0x{k.zfill(8)}/24  (10.{(v>>16)&255}.{(v>>8)&255}.0)  -> port {tbl[k]}")

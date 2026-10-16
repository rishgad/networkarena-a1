# A2 design notes: Make It Fair

The switch is `examples/my_switch/src/lib.rs`: the A1 link-state router,
plus queues (Part 1) and per-customer paths (Part 2). These notes give
the reasons behind each choice, with the measurements behind them.

## Results

Practice worlds, `--score`, after the final change:

| world | ×1 | ×1.5 | worst customer (×1.5) |
|---|---|---|---|
| a2p1-dumb-004 | PASS | PASS | 87% of fair share |
| a2p1-grid-009 | PASS | PASS | 89% |
| a2p1-ring-020 | PASS | PASS | 71% |
| a2p2-dumb-003 | PASS | PASS | 71% |
| a2p2-grid-006 | PASS | PASS | 83% |

F1 needs 40%, F2 needs 55% (lowest seen: 82%), F3 needs 0 link-down
declarations (0 everywhere). All 20 runs from ×0.5 to ×2 pass; the
weakest is a2p1-ring-020 at ×2 (Netflix, 48%). The A1 regression suite
(`worlds/a1`, `scripts/a1_benchmark.py`) still passes 20/20.

Starting point, the A1 router unchanged: 0/10. It used one FIFO per
port, so whoever filled the buffer won.

## 1. The queue: one per customer

Every port has queue 0 for control and acks, then one queue per customer
in the contract (customer *i* is queue *i*+1, up to 15). A stage in the
TinyVM pipeline looks the source address up in `T_CLASS` and sets the
queue.

- **Isolation.** A customer can only fill its own queue. Steam cannot
  push Jane Street's packets out of a shared buffer.
- **AIMD and rate-matching never share a queue.** The handout warns
  about it, and the per-customer layout rules it out by construction.
- **Buffer.** The caps add up to more than the 40-packet pool. Only
  queues with traffic hold packets, and a port carries a few customers
  at most, so oversubscribing lets a busy queue use space a quiet one
  is not using.

### Queue depth: measured, not guessed

| sender | cap | why |
|---|---|---|
| rate matching | 3 packets | a deep queue *traps* it (below) |
| AIMD | 10 packets | absorbs the halving so the link stays busy |
| control (q0) | 16, DRR weight 4 | HELLOs and LSAs are never starved |

**The rate-matching trap** (found in a2p1-grid-009 ×1.5, Steam at 9%).
`src/flow.rs` declares a packet lost after 2 × smoothed RTT. Steam
starts at 10 Mbit/s into a 2 Mbit/s link, and an 8-packet queue holds it
32 ms, far more than the ~5 ms base round trip. Packets still waiting
are declared lost, and acks for packets already given up on do not count
toward the delivered rate. The sender measures almost nothing delivered
and falls to its floor of one packet per 100 ms window. At that floor
the window holds one acked packet: it measures 0.08 Mbit/s, adds 25%,
and the faster rate still yields one packet per window. It never climbs
out. Caps of 2, 3 and 4 all avoid it, and 3 is the middle.

**AIMD** wants the opposite. A drop halves it, and the queue is what
keeps the link busy while it climbs back. Caps of 3–4 cost Amazon its
share; 10 beats 6 at ×1.5 (ring-020 Netflix 60% → 71%).

## 2. The schedule: deficit round robin

All queues on a port are served by DRR with equal weights (control gets
4). DRR gives every backlogged queue an equal slice of the wire, and a
customer that asks for less than its slice (Jane Street, Fortnite) is
served in full, with the rest shared among the others. That is
max-min fairness on each link, which is exactly what F1 measures.

Strict priority was the alternative, but it starves lower queues, and a
port can run only one discipline. Control traffic does not need it: its
packets are tiny, and DRR visits queue 0 every round.

## 3. The path: placed once, by the customer's own switch

**Routing by source.** Each customer sends to one peer, so a `T_STEER`
entry keyed on the *source* prefix pins a whole customer to one path.
It overrides the A1 destination route. Acks and receivers keep using
the A1 destination routes.

**What path.** The grader's fair share in Part 2 comes from a greedy
placement over every route within 2 hops of the shortest
(`floors::assign_paths`). It goes in a fixed order: fewest routes, then
biggest demand (unlimited first), then contract order. Each customer
takes the route whose thinnest link, divided among the customers already
on it, leaves the most. The switch runs that same algorithm, with the
menu built node for node like `floors::path_menu`. Each customer then
sits on the route the grader measures it against.

**Load information.** The switch has none beyond the contract.
Placement uses the contract (demands), the topology, and link capacities
carried in the LSAs, and needs no drop reports. In this assignment every
customer keeps its contract, so the demand is known in advance, and the
picture is the same at every switch, with no measurement lag.

**Loop safety.** The rule is one decision per customer, made once.

- Only the switch the customer is attached to chooses its path, and it
  floods the path in its LSA with an activation time 100 ms ahead.
  Every switch on the path installs its hop at that instant (a one-shot
  timer). With one authority per customer, no two switches can disagree
  about a customer's next hop.
- A path is never changed after it goes live, so no route ever flaps
  and there are no mixed old/new states.
- Paths announced in the same round are recomputed by every switch
  rather than taken as fixed, so all switches compute the same plan.
  Only paths already live count as fixed, for a customer that shows up
  late.

**When.** Placement happens at 1.5 s, once the map has been quiet for
200 ms. Amazon starts up to 1 s in (200–1000 ms in the practice worlds).
Placing earlier parked Netflix on the very link Amazon needed (Amazon
at 44%). Part 2 grading starts at 3 s; before placement, customers ride
the A1 shortest paths.

**Learning where a customer sends.** The contract has no peers. Until
some switch claims a customer prefix, every switch punts that prefix's
packets. The customer's own switch thus sees a packet on a port that has
never sent a HELLO, and learns the prefix and its destination. It keeps
punting until it sees a destination other than the customer's own
prefix, because Amazon's first packet is addressed to itself.

**Route changes.** None, by design. Placement assumes every customer is
sending, which is pessimistic for the seconds Amazon is off. Moving
customers when Amazon starts and stops would track the grader's
per-second placement more closely. But every move is a window in which
old and new routes mix, and a single loop fails the whole run. The
measured margins (71%+ against a 40% floor) did not justify the risk.

## 4. Protecting control traffic (F3)

The switch never calls `link_down`. HELLOs and LSAs ride queue 0, which
DRR serves every round, so congestion does not delay them past the
150 ms silence timeout. F3 is 0 in every run.

## Known limits / A3

- Paths are fixed. In A3, links fail, and a fixed path through a dead
  link stays dead. Re-placing needs the loop-safe transition this design
  avoided.
- Placement trusts the contract's demand. In A3 the switch is told
  nothing about customers, so demand has to come from measurement
  (`port_drops`, as `examples/load_report` does).

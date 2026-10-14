//! my_switch — a link-state routing program.
//!
//! One program, installed on every switch. Each copy knows only its own
//! switch_id and which ports have cables. Everything else is learned.
//!
//! The design, in one paragraph:
//!
//!   Every switch shouts HELLO down every cable every 50ms. Hearing a HELLO
//!   on port p teaches you "neighbour N is on port p", and the *time* you
//!   heard it is how you later notice N has gone silent. Every switch then
//!   describes itself -- "I am 3, I can hear {1,2,5}, my customer is
//!   10.0.226.0/24" -- in an LSA, and floods it. Sequence numbers stop the
//!   flood from looping. Once you hold everyone's LSA you hold the whole
//!   map, so you run a shortest-path search on it and program the data
//!   plane with a next-hop port per destination prefix.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use switch_program_sdk::*;

// ---------------------------------------------------------------- constants

/// Table ids. These must match between `declare_table`, the pipeline text
/// ("t1"/"t2"), and every `install_route` call.
const T_PROTO: u32 = 1;
const T_ROUTE: u32 = 2;

/// Our control traffic rides on this IP protocol number. Customer traffic
/// uses proto 0, so this cleanly separates "mine" from "theirs".
const PROTO_CTRL: u8 = 89;

/// Destination address for control packets. Nothing routes this; the proto
/// table catches these packets and punts them to us.
const CTRL_IP: u32 = 0xe000_0005; // 224.0.0.5

/// How often we wake up: send HELLOs, check for silence, maybe reflood.
const TICK_NS: u64 = 50_000_000; // 50 ms

/// Silence longer than this and we declare the neighbour gone. Three missed
/// HELLOs, so a single dropped packet does not cost us a route flap.
///
/// This number *is* our recovery time. The budget is 1000ms; we spend at
/// most 150ms noticing, and the flood + recompute after it is ~1ms.
const DEAD_NS: u64 = 150_000_000; // 150 ms

/// Re-announce ourselves this often even when nothing changed, so a lost
/// flood cannot leave someone holding a stale map forever.
const LSA_REFRESH_NS: u64 = 1_000_000_000; // 1 s

/// Message type tags, the first byte of every control payload.
const MSG_HELLO: u8 = 1;
const MSG_LSA: u8 = 2;

/// Every customer prefix is a /24.
const PREFIX_LEN: u8 = 24;
const PREFIX_MASK: u32 = 0xffff_ff00;

// --------------------------------------------------------------- the LSA

/// One switch's description of itself. This is the only thing that ever
/// travels the network, and a switch only ever originates its *own*.
#[derive(Clone, Default)]
struct Lsa {
    /// Version counter. Higher wins; equal or lower is discarded. This is
    /// what terminates the flood.
    seq: u64,
    /// Switch ids this switch could hear when it built the LSA.
    neighbors: Vec<u32>,
    /// Its customer prefix, already masked to /24. 0 means "none known yet".
    prefix: u32,
}

// ------------------------------------------------------------ the program

pub struct Sw {
    switch_id: u32,
    local_ports: Vec<u16>,
    /// Simulator time of the last event we saw. The only clock we get.
    now: u64,

    // -- learned from HELLOs --
    /// port -> neighbour switch id
    neighbor_on_port: BTreeMap<u16, u32>,
    /// neighbour switch id -> port (the reverse lookup, for routing)
    port_of_neighbor: BTreeMap<u32, u16>,
    /// port -> when we last heard a HELLO on it. Failure detection.
    last_heard: BTreeMap<u16, u64>,

    // -- learned from our customer's own traffic --
    app_port: Option<u16>,
    my_prefix: u32,

    // -- the map --
    /// switch id -> the newest LSA we hold from it. Includes our own.
    lsdb: BTreeMap<u32, Lsa>,
    my_seq: u64,
    last_lsa_sent: u64,

    // -- what the data plane currently believes --
    /// prefix -> egress port we last installed for it
    installed: BTreeMap<u32, u16>,

    /// Set whenever something we know changed, so the next tick refloods
    /// and recomputes.
    self_changed: bool,
}

// --------------------------------------------------------- byte helpers
//
// Hand-rolled so the wire format is small and obvious. Everything is
// little-endian. Every read is bounds-checked: a panic inside the sandbox
// kills the program for the rest of the run.

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put_u64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn get_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn get_u64(b: &[u8], off: usize) -> Option<u64> {
    let s = b.get(off..off + 8)?;
    Some(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

impl Sw {
    // ------------------------------------------------------ wire format

    /// HELLO: [tag][sender id]
    fn encode_hello(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(5);
        v.push(MSG_HELLO);
        put_u32(&mut v, self.switch_id);
        v
    }

    /// LSA: [tag][origin][seq][prefix][count][count x neighbour id]
    fn encode_lsa(&self, origin: u32, lsa: &Lsa) -> Vec<u8> {
        let mut v = Vec::with_capacity(18 + lsa.neighbors.len() * 4);
        v.push(MSG_LSA);
        put_u32(&mut v, origin);
        put_u64(&mut v, lsa.seq);
        put_u32(&mut v, lsa.prefix);
        v.push(lsa.neighbors.len() as u8);
        for n in &lsa.neighbors {
            put_u32(&mut v, *n);
        }
        v
    }

    fn decode_lsa(b: &[u8]) -> Option<(u32, Lsa)> {
        let origin = get_u32(b, 1)?;
        let seq = get_u64(b, 5)?;
        let prefix = get_u32(b, 13)?;
        let count = *b.get(17)? as usize;
        let mut neighbors = Vec::with_capacity(count);
        for i in 0..count {
            neighbors.push(get_u32(b, 18 + i * 4)?);
        }
        Some((origin, Lsa { seq, neighbors, prefix }))
    }

    // -------------------------------------------------- neighbour state

    /// Neighbours we have heard from recently enough to still trust.
    fn alive_neighbors(&self) -> Vec<(u32, u16)> {
        self.neighbor_on_port
            .iter()
            .filter(|(port, _)| {
                self.last_heard
                    .get(*port)
                    .is_some_and(|t| self.now.saturating_sub(*t) < DEAD_NS)
            })
            .map(|(port, id)| (*id, *port))
            .collect()
    }

    /// Drop neighbours that have gone quiet. Returns true if anything changed,
    /// which is what triggers a reflood and a recompute.
    fn expire_dead(&mut self) -> bool {
        let dead: Vec<u16> = self
            .neighbor_on_port
            .keys()
            .copied()
            .filter(|port| {
                self.last_heard
                    .get(port)
                    .is_none_or(|t| self.now.saturating_sub(*t) >= DEAD_NS)
            })
            .collect();

        if dead.is_empty() {
            return false;
        }
        for port in dead {
            if let Some(id) = self.neighbor_on_port.remove(&port) {
                self.port_of_neighbor.remove(&id);
            }
        }
        true
    }

    /// Our own LSA, rebuilt from current state.
    fn my_lsa(&self) -> Lsa {
        let mut neighbors: Vec<u32> = self.alive_neighbors().iter().map(|(id, _)| *id).collect();
        neighbors.sort_unstable();
        Lsa {
            seq: self.my_seq,
            neighbors,
            prefix: self.my_prefix,
        }
    }

    // ------------------------------------------------------- forwarding

    /// Build the graph from the LSDB and find, for every switch we can
    /// reach, which neighbour is the first hop toward it.
    ///
    /// All links in these worlds are identical (100us, 1Gbps), so shortest
    /// path == fewest hops, and a breadth-first search is exactly right.
    ///
    /// The bidirectional check matters: we only believe an edge u-v when
    /// *both* u and v claim it. After a link dies, one side notices first,
    /// and for a few milliseconds the two LSAs disagree. Trusting a
    /// one-sided edge during that window routes traffic into a hole.
    fn first_hops(&self) -> BTreeMap<u32, u32> {
        let edge = |u: u32, v: u32| -> bool {
            let a = self.lsdb.get(&u).is_some_and(|l| l.neighbors.contains(&v));
            let b = self.lsdb.get(&v).is_some_and(|l| l.neighbors.contains(&u));
            a && b
        };

        let mut first: BTreeMap<u32, u32> = BTreeMap::new();
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut queue: VecDeque<u32> = VecDeque::new();

        seen.insert(self.switch_id);

        // Seed with our live neighbours: the first hop toward a neighbour
        // is the neighbour itself.
        for (id, _port) in self.alive_neighbors() {
            if seen.insert(id) {
                first.insert(id, id);
                queue.push_back(id);
            }
        }

        while let Some(u) = queue.pop_front() {
            let Some(lsa) = self.lsdb.get(&u) else { continue };
            let via = *first.get(&u).unwrap_or(&u);
            for &v in &lsa.neighbors {
                if seen.contains(&v) || !edge(u, v) {
                    continue;
                }
                seen.insert(v);
                first.insert(v, via);
                queue.push_back(v);
            }
        }
        first
    }

    /// Recompute every route and emit only the entries that actually
    /// changed. Returning unchanged entries would burn the 4096-byte
    /// action budget for nothing.
    fn recompute(&mut self) -> Vec<Action> {
        let first = self.first_hops();
        let mut want: BTreeMap<u32, u16> = BTreeMap::new();

        // Our own customer hangs off our app port.
        if self.my_prefix != 0 {
            if let Some(p) = self.app_port {
                want.insert(self.my_prefix, p);
            }
        }

        // Everyone else's customer is reached through the first hop toward
        // the switch that announced it.
        for (sw, lsa) in &self.lsdb {
            if *sw == self.switch_id || lsa.prefix == 0 {
                continue;
            }
            if let Some(via) = first.get(sw) {
                if let Some(port) = self.port_of_neighbor.get(via) {
                    want.insert(lsa.prefix, *port);
                }
            }
        }

        let mut out = Vec::new();

        // Install and update.
        for (prefix, port) in &want {
            if self.installed.get(prefix) != Some(port) {
                // The simulator's `Table::install` APPENDS; it does not
                // replace an entry that has the same id. Both entries then
                // sit in the table, `lookup` returns the first match, and
                // the sort (prefix_len desc, priority desc) is stable -- so
                // among equal /24s the ORIGINAL entry stays first and keeps
                // winning. An update without a delete is accepted, logged,
                // and then silently ignored by the data plane.
                //
                // So: delete first, then install. Same effect as an
                // overwrite, and the only version the data plane will obey.
                if self.installed.contains_key(prefix) {
                    out.push(actions::delete_entry(T_ROUTE, *prefix as u64));
                }
                out.push(actions::install_route(
                    T_ROUTE,
                    *prefix as u64,
                    *prefix as u64,
                    PREFIX_LEN,
                    *port,
                ));
            }
        }

        // Withdraw anything we can no longer reach, so the data plane stops
        // forwarding into a hole.
        for prefix in self.installed.keys() {
            if !want.contains_key(prefix) {
                out.push(actions::delete_entry(T_ROUTE, *prefix as u64));
            }
        }

        self.installed = want;
        out
    }

    /// Send one control payload out a set of ports. TTL 1: every hop is a
    /// fresh injection by that switch's controller, so these never travel
    /// further than the next switch on their own.
    fn send(&self, ports: impl Iterator<Item = u16>, payload: &[u8]) -> Vec<Action> {
        ports
            .map(|p| {
                actions::inject_packet(
                    p,
                    0xa9fe_0000 | (self.switch_id & 0xffff),
                    CTRL_IP,
                    PROTO_CTRL,
                    1,
                    payload.to_vec(),
                )
            })
            .collect()
    }
}

impl SwitchProgram for Sw {
    fn init(switch_id: u32, local_ports: Vec<u16>) -> (Self, ProgramSetup) {
        let mut setup = ProgramSetup::new();

        setup.declare_table(T_PROTO, MatchKind::Exact, 8);
        setup.declare_table(T_ROUTE, MatchKind::Lpm, 256);

        // Stage 0 catches our own control traffic and punts it.
        // Stage 1 is the real forwarding lookup on the destination address.
        setup.set_program(
            text::parse_tiny_program(
                "
                stage alu=4 mem=1
                    load    r0, ip_proto
                    table   t1, r0 -> m0
                stage alu=4 mem=1
                    load    r1, ip_dst
                    table   t2, r1 -> m1
                ",
            )
            .unwrap(),
        );

        // Without this entry a HELLO arriving at a neighbour would fall
        // through to the route lookup and be forwarded as ordinary traffic,
        // and no switch would ever learn anything.
        setup.add_entry(
            T_PROTO,
            TableEntry {
                id: wire::EntryIdW(1),
                key: PROTO_CTRL as u64,
                prefix_len: 8,
                priority: 0,
                action: TableAction::Punt {
                    reason: PuntReason::Custom(PROTO_CTRL as u32),
                },
            },
        );

        let me = Sw {
            switch_id,
            local_ports,
            now: 0,
            neighbor_on_port: BTreeMap::new(),
            port_of_neighbor: BTreeMap::new(),
            last_heard: BTreeMap::new(),
            app_port: None,
            my_prefix: 0,
            lsdb: BTreeMap::new(),
            my_seq: 0,
            last_lsa_sent: 0,
            installed: BTreeMap::new(),
            self_changed: true,
        };
        (me, setup)
    }

    fn on_punt(&mut self, ev: PuntEvent) -> Vec<Action> {
        self.now = ev.now_ns;
        let mut out = Vec::new();

        if ev.ip_proto == PROTO_CTRL {
            // ---- one of ours ----
            let Some(&tag) = ev.payload.first() else {
                return out;
            };

            match tag {
                MSG_HELLO => {
                    let Some(sender) = get_u32(&ev.payload, 1) else {
                        return out;
                    };

                    // This port faces a switch, so it is not our customer.
                    // Undo an earlier guess if we made one.
                    if self.app_port == Some(ev.ingress_port) {
                        self.app_port = None;
                        self.my_prefix = 0;
                        self.self_changed = true;
                    }

                    let known = self.neighbor_on_port.get(&ev.ingress_port);
                    if known != Some(&sender) {
                        if let Some(old) = known.copied() {
                            self.port_of_neighbor.remove(&old);
                        }
                        self.neighbor_on_port.insert(ev.ingress_port, sender);
                        self.port_of_neighbor.insert(sender, ev.ingress_port);
                        self.self_changed = true;
                    }
                    // The timestamp is the point of the whole exercise.
                    self.last_heard.insert(ev.ingress_port, self.now);
                }

                MSG_LSA => {
                    let Some((origin, lsa)) = Self::decode_lsa(&ev.payload) else {
                        return out;
                    };
                    // Never let anyone else tell us about ourselves.
                    if origin == self.switch_id {
                        return out;
                    }

                    let fresh = self.lsdb.get(&origin).is_none_or(|have| lsa.seq > have.seq);
                    if !fresh {
                        // Already seen. Dropping it here is what stops the
                        // flood from circulating forever.
                        return out;
                    }

                    let payload = self.encode_lsa(origin, &lsa);
                    self.lsdb.insert(origin, lsa);
                    // NOTE: deliberately *not* setting self_changed here.
                    // Someone else's news changes our map, so we re-run the
                    // shortest-path search below -- but nothing about *us*
                    // changed, so we must not re-originate our own LSA.
                    // Doing that meant every switch published a fresh
                    // sequence number every tick, each one flooding the
                    // whole network and provoking the next round: a control
                    // storm that never settles.

                    // Pass it on, everywhere except where it came from.
                    let onward: Vec<u16> = self
                        .alive_neighbors()
                        .into_iter()
                        .map(|(_, p)| p)
                        .filter(|p| *p != ev.ingress_port)
                        .collect();
                    out.extend(self.send(onward.into_iter(), &payload));

                    out.extend(self.recompute());
                }

                _ => {}
            }
        } else {
            // ---- customer traffic with no matching route ----
            //
            // If it arrived on a port where we have never heard a HELLO,
            // that port does not face a switch. It faces our customer --
            // and the source address tells us which prefix they are.
            let is_switch_port = self.neighbor_on_port.contains_key(&ev.ingress_port);
            if !is_switch_port && ev.ip_src != 0 {
                let prefix = ev.ip_src & PREFIX_MASK;
                if self.my_prefix != prefix || self.app_port != Some(ev.ingress_port) {
                    self.my_prefix = prefix;
                    self.app_port = Some(ev.ingress_port);
                    self.self_changed = true;
                    out.extend(self.recompute());
                }
            }
        }

        out
    }

    fn on_timer(&mut self, ev: TimerEvent) -> Vec<Action> {
        self.now = ev.now_ns;
        let mut out = Vec::new();

        // 1. Has anyone gone quiet?
        if self.expire_dead() {
            self.self_changed = true;
        }

        // 2. Greet everyone, on every cable, including the one facing our
        //    customer -- never hearing a reply there is precisely how we
        //    know it is a customer and not a switch.
        let hello = self.encode_hello();
        out.extend(self.send(self.local_ports.iter().copied(), &hello));

        // 3. Re-announce ourselves when something changed, or periodically
        //    as a safety net against a flood that got lost.
        let due = self.now.saturating_sub(self.last_lsa_sent) >= LSA_REFRESH_NS;
        if self.self_changed || due {
            self.my_seq += 1;
            let mine = self.my_lsa();
            let payload = self.encode_lsa(self.switch_id, &mine);
            self.lsdb.insert(self.switch_id, mine);
            self.last_lsa_sent = self.now;

            let ports: Vec<u16> = self.alive_neighbors().into_iter().map(|(_, p)| p).collect();
            out.extend(self.send(ports.into_iter(), &payload));

            out.extend(self.recompute());
            self.self_changed = false;
        }

        // 4. Ask for the next tick. Forget this and we wake up once, ever.
        out.push(actions::schedule_timer(TICK_NS));
        out
    }
}

switch_program!(Sw);

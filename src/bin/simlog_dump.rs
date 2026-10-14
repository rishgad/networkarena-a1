use competitive_net_sim::sim_log::{self, LogEvent};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).cloned().unwrap_or_else(|| "/tmp/routing.simlog".into());
    let switch_filter: Option<u32> = args.get(2).and_then(|s| s.parse().ok());
    let bytes = std::fs::read(&path).expect("read log");
    let frames = sim_log::writer::decode_all(&bytes).expect("decode");
    println!("# {} frames in {}", frames.len(), path);
    for f in &frames {
        match &f.event {
            LogEvent::TableEntryInstalled { switch, table, entry } => {
                if switch_filter.map_or(true, |s| s == *switch) {
                    println!(
                        "{:>10} ns  S{}  install  t{}  entry id={} key=0x{:x}/{} -> {:?}",
                        f.at_ns, switch, table, entry.id.0, entry.key, entry.prefix_len, entry.action
                    );
                }
            }
            LogEvent::TableEntryDeleted { switch, table, entry_id } => {
                if switch_filter.map_or(true, |s| s == *switch) {
                    println!(
                        "{:>10} ns  S{}  delete   t{}  entry id={}",
                        f.at_ns, switch, table, entry_id
                    );
                }
            }
            LogEvent::LinkFailed { id } => {
                println!("{:>10} ns  link L{} FAILED", f.at_ns, id);
            }
            LogEvent::LinkRestored { id } => {
                println!("{:>10} ns  link L{} restored", f.at_ns, id);
            }
            LogEvent::ProgramInstalled { switch, .. } => {
                if switch_filter.map_or(true, |s| s == *switch) {
                    println!("{:>10} ns  S{}  PROGRAM INSTALLED", f.at_ns, switch);
                }
            }
            LogEvent::PacketIngress { switch, port, packet } if packet.ip_proto != 254 => {
                if switch_filter.map_or(true, |s| s == *switch) {
                    println!(
                        "{:>10} ns  S{}  IN       port={} proto={} src=0x{:x} dst=0x{:x} ttl={} packet={}",
                        f.at_ns, switch, port, packet.ip_proto, packet.ip_src,
                        packet.ip_dst, packet.ip_ttl, packet.id
                    );
                }
            }
            LogEvent::PacketEgress { switch, port, packet_id, .. } => {
                if switch_filter.map_or(true, |s| s == *switch) {
                    println!(
                        "{:>10} ns  S{}  OUT      port={} packet={}",
                        f.at_ns, switch, port, packet_id
                    );
                }
            }
            LogEvent::PacketDropped { at, reason, packet_id } => {
                let (kind, id) = match at {
                    competitive_net_sim::sim_log::NodeRefW::Switch(s) => ("S", *s),
                    competitive_net_sim::sim_log::NodeRefW::App(a) => ("A", *a),
                };
                if switch_filter.map_or(true, |s| kind == "S" && s == id) {
                    println!(
                        "{:>10} ns  {}{}  DROP  {:?}  packet={}",
                        f.at_ns, kind, id, reason, packet_id
                    );
                }
            }
            LogEvent::PacketIngress { packet, .. } => {
                if packet.ip_proto == 254 {
                    let dst = packet.ip_dst;
                    let stale_dst = (dst >> 16) & 0xff;
                    let from_sw = (dst >> 8) & 0xff;
                    let fh = dst & 0xff;
                    let port = packet.ip_src & 0xffff;
                    if switch_filter.map_or(true, |s| s == from_sw) {
                        let fh_str = if fh == 0xff { "none".into() } else { format!("S{}", fh) };
                        let port_str = if port == 0xffff { "none".into() } else { port.to_string() };
                        println!(
                            "{:>10} ns  S{}  DIAG  delete-S{}; first_hop={} port={}",
                            f.at_ns, from_sw, stale_dst, fh_str, port_str,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

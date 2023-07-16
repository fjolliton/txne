use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    thread,
};

use axum::{extract::State, routing::get, Router};
use clap::Parser;
use pcap::{Active, Capture, Linktype};

/// Prometheus node exporter with per IP traffic statistics
#[derive(Parser, Debug)]
struct Args {
    /// Interface to listen
    #[arg(short, long)]
    interface: String,

    /// Exporter listen address (use "0.0.0.0" or "::" to bind on
    /// every interfaces, but this is not recommended)
    #[arg(short, long)]
    bind: String,

    /// Exporter port
    #[arg(short, long)]
    port: u16,

    /// Subnet(s) to consider as local
    #[arg(short, long)]
    subnets: String,

    /// Subnet(s) to ignore
    #[arg(short, long)]
    exclude: Option<String>,

    /// Maximum number of IP to track
    #[arg(short, long, default_value_t = 1024)]
    max: usize,
}

const ETHER_IPV4: u16 = 0x0800;

#[derive(Clone, Copy)]
enum Protocol {
    Icmp,
    Tcp,
    Udp,
    Other,
}

impl Protocol {
    fn to_str(&self) -> &'static str {
        match self {
            Protocol::Icmp => "icmp",
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
            Protocol::Other => "other",
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Inbound,
    Outbound,
}

impl Direction {
    fn to_str(&self) -> &'static str {
        match self {
            Direction::Inbound => "inbound",
            Direction::Outbound => "outbound",
        }
    }
}

#[derive(Clone, Copy)]
enum ValueType {
    Packets,
    Bytes,
}

impl ValueType {
    fn to_str(&self) -> &'static str {
        match self {
            ValueType::Packets => "packets",
            ValueType::Bytes => "bytes",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BaseCounters {
    pkts: u64,
    bytes: u64,
}

#[derive(Debug, Clone, Default)]
struct DirectionCounters {
    inbound: BaseCounters,
    outbound: BaseCounters,
}

#[derive(Debug, Clone, Default)]
struct ProtocolCounters {
    icmp: DirectionCounters,
    tcp: DirectionCounters,
    udp: DirectionCounters,
    other: DirectionCounters,
}

type Stats = HashMap<Option<u32>, ProtocolCounters>;

#[derive(Clone)]
struct ServerState {
    stats: Arc<Mutex<Stats>>,
}

fn run(
    mut cap: Capture<Active>,
    is_local: impl Fn(u32) -> bool,
    is_excluded: Option<impl Fn(u32) -> bool>,
    max_tracking: usize,
    out_stats: Arc<Mutex<Stats>>,
) {
    let mut stats = Stats::default();
    let mut sync_remaining = 0usize;
    loop {
        if sync_remaining == 0 {
            *out_stats.lock().unwrap() = stats.clone();
            sync_remaining = 64;
        }
        sync_remaining -= 1;

        let pkt = cap.next_packet().ok();
        if let Some(pkt) = pkt {
            if pkt.header.caplen >= 14 + 20 {
                let data = pkt.data;
                let eth_proto = u16::from_be_bytes(data[12..14].try_into().unwrap());
                if eth_proto == ETHER_IPV4 {
                    let ip = &data[14..];
                    let ip_proto = ip[9];
                    let ip_source = u32::from_be_bytes(ip[12..16].try_into().unwrap());
                    let ip_dest = u32::from_be_bytes(ip[16..20].try_into().unwrap());
                    if let Some(is_excluded) = &is_excluded {
                        if is_excluded(ip_source) || is_excluded(ip_dest) {
                            continue;
                        }
                    }
                    let from_local = is_local(ip_source);
                    let to_local = is_local(ip_dest);
                    if from_local != to_local {
                        let ip_entry = if from_local { ip_source } else { ip_dest };
                        let ip_entry = if !stats.contains_key(&Some(ip_entry))
                            && stats.len() >= max_tracking
                        {
                            None
                        } else {
                            Some(ip_entry)
                        };
                        let entry = stats.entry(ip_entry);
                        let entry = entry.or_insert(ProtocolCounters::default());
                        let item = match ip_proto {
                            1 => &mut entry.icmp,
                            6 => &mut entry.tcp,
                            17 => &mut entry.udp,
                            _ => &mut entry.other,
                        };
                        let mut item = if from_local {
                            &mut item.outbound
                        } else {
                            &mut item.inbound
                        };
                        item.pkts += 1;
                        item.bytes += pkt.header.len as u64;
                    }
                }
            }
        }
    }
}

async fn metrics(State(state): State<ServerState>) -> String {
    let mut result = String::new();

    let stats = state.stats.lock().unwrap().clone();

    let mut ips = stats.keys().collect::<Vec<_>>();
    ips.sort_by(|ip_a, ip_b| ip_a.cmp(ip_b));

    let add_desc = |result: &mut String, direction: Direction, value_type: ValueType| {
        let dir_name = match direction {
            Direction::Inbound => "entering",
            Direction::Outbound => "leaving",
        };
        let type_name = match value_type {
            ValueType::Packets => "Packets",
            ValueType::Bytes => "Bytes",
        };
        let direction = direction.to_str();
        let value_type = value_type.to_str();
        result.push_str(&format!(
            "# HELP {direction}_{value_type}_total {type_name} {dir_name} the network\n",
        ));
        result.push_str(&format!("# TYPE {direction}_{value_type}_total counter\n",));
    };

    let add_metric = |result: &mut String,
                      stats: &Stats,
                      direction: Direction,
                      value_type: ValueType,
                      ip: Option<u32>,
                      protocol: Protocol| {
        let counter = {
            let entry = {
                let entry = {
                    let entry = stats.get(&ip).unwrap();
                    match protocol {
                        Protocol::Icmp => &entry.icmp,
                        Protocol::Tcp => &entry.tcp,
                        Protocol::Udp => &entry.udp,
                        Protocol::Other => &entry.other,
                    }
                };
                match direction {
                    Direction::Inbound => &entry.inbound,
                    Direction::Outbound => &entry.outbound,
                }
            };
            match value_type {
                ValueType::Packets => &entry.pkts,
                ValueType::Bytes => &entry.bytes,
            }
        };
        let field = match direction {
            Direction::Inbound => "ip_dest",
            Direction::Outbound => "ip_source",
        };
        let direction = direction.to_str();
        let value_type = value_type.to_str();
        let protocol = protocol.to_str();
        let ip = ip.map(|ip| {
            let ip = ip.to_be_bytes();
            format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
        });
        let ip = ip.as_deref().unwrap_or("other");
        result.push_str(&format!(
            "{direction}_{value_type}_total{{ip_version=\"4\",{field}=\"{ip}\",protocol=\"{protocol}\"}} {counter}\n",
        ));
    };

    for direction in [Direction::Inbound, Direction::Outbound] {
        for value_type in [ValueType::Packets, ValueType::Bytes] {
            add_desc(&mut result, direction, value_type);

            for ip in ips.iter() {
                for protocol in [
                    Protocol::Icmp,
                    Protocol::Tcp,
                    Protocol::Udp,
                    Protocol::Other,
                ] {
                    add_metric(&mut result, &stats, direction, value_type, **ip, protocol);
                }
            }
            result.push_str("\n");
        }
    }
    result
}

/// Parse a comma separated list of IPv4 subnets
fn parse_subnets(subnets: &str) -> Option<Vec<(u32, u32)>> {
    let mut result = Vec::new();
    for part in subnets.split(",") {
        let (address, size) = part.split_once("/").unwrap_or((part, "32"));
        let address: Ipv4Addr = address.parse().ok()?;
        let address = u32::from_be_bytes(address.octets());
        let size = u8::from_str_radix(size, 10).ok()?;
        if size > 32 {
            return None;
        }
        let mask = if size == 32 { !0 } else { !(!0 >> size) };
        result.push((address & mask, mask));
    }
    Some(result)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let subnets = parse_subnets(&args.subnets).unwrap_or_else(|| {
        println!("Invalid subnets");
        std::process::exit(1);
    });
    let is_local = move |ip: u32| subnets.iter().any(|(addr, mask)| ip & mask == *addr);

    let excluded_subnets = args.exclude.map(|s| {
        parse_subnets(&s).unwrap_or_else(|| {
            println!("Invalid subnets");
            std::process::exit(1);
        })
    });
    let is_excluded = excluded_subnets.map(|excluded_subnets| {
        move |ip: u32| {
            excluded_subnets
                .iter()
                .any(|(addr, mask)| ip & mask == *addr)
        }
    });

    let device = pcap::Device::list()
        .expect("device lookup failed")
        .into_iter()
        .find(|dev| dev.name == args.interface)
        .expect("device not found");
    println!("Using device {}", device.name);

    let cap = pcap::Capture::from_device(device)
        .unwrap()
        .immediate_mode(true)
        .snaplen(64)
        .open()
        .unwrap();

    let link = cap.get_datalink();
    if link != Linktype::ETHERNET {
        println!(
            "Interface not supported. {:?} is not an Ethernet interface.",
            args.interface
        );
        std::process::exit(1);
    }

    let stats = Arc::new(Mutex::new(Stats::default()));
    let state = ServerState {
        stats: stats.clone(),
    };

    let thread_stats = stats.clone();
    thread::spawn(move || {
        run(cap, is_local, is_excluded, args.max, thread_stats);
    });

    let app = Router::new()
        .route("/metrics", get(metrics))
        .with_state(state);

    let bind_ip: IpAddr = args.bind.parse().unwrap();

    axum::Server::bind(&SocketAddr::new(bind_ip, args.port))
        .serve(app.into_make_service())
        .await
        .unwrap();
}

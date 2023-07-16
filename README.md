# TXNE — Per IP Metrics Exporter for Prometheus

A node exporter to get metrics per IP for all the traffic entering or
leaving the network described by the given subnets.

For example, when monitoring `192.168.0.0/16`, anything from this
subnet to something outside this subnet will be reported as
"outbound", while the opposite traffic will be reported as "inbound".
Traffic between the given subnets will not be reported, and traffic
between subnets outside those will not be reported either.

The metrics are reachable through the standard `/metrics` path at the
configured IP address (`--bind`) and port (`--port`).

The `--subnets` option accept a list of comma separated network
specification. For example `--subnets
10.0.0.0/8,192.168.0.0/16,172.16.200.0/24`.

To exclude traffic from the reporting use the `--exclude` option. For
example to exclude multicast on the local network, use `--exclude
224.0.0.0/24`.

## Limitations

 - This only supports Ethernet interfaces. This means that this will
   not work for the `any` interface.
 - This only works for IPv4 traffic. Metrics for IPv6 are not supported.

## Setup

You will need the Rust compiler, and the PCAP library installed.

Run `cargo build --release` to compile the program. The result will be
located in `target/release/txne`.

## Command line

```
Prometheus node exporter with per IP traffic statistics

Usage: txne [OPTIONS] --interface <INTERFACE> --bind <BIND> --port <PORT> --subnets <SUBNETS>

Options:
  -i, --interface <INTERFACE>  Interface to listen
  -b, --bind <BIND>            Exporter listen address (use "0.0.0.0" or "::" to bind on every interfaces, but this is not recommended)
  -p, --port <PORT>            Exporter port
  -s, --subnets <SUBNETS>      Subnet(s) to consider as local
  -e, --exclude <EXCLUDE>      Subnet(s) to ignore
  -m, --max <MAX>              Maximum number of IP to track [default: 1024]
  -h, --help                   Print help
```

## Output example

When querying the exporter with HTTP, you will get the Prometheus
metrics that looks like this:

```
# HELP inbound_packets_total Packets entering the network
# TYPE inbound_packets_total counter
inbound_packets_total{ip_version="4",ip_dest="192.168.0.100",protocol="icmp"} 51
inbound_packets_total{ip_version="4",ip_dest="192.168.0.100",protocol="tcp"} 1597
inbound_packets_total{ip_version="4",ip_dest="192.168.0.100",protocol="udp"} 155
inbound_packets_total{ip_version="4",ip_dest="192.168.0.100",protocol="other"} 0
inbound_packets_total{ip_version="4",ip_dest="192.168.0.215",protocol="icmp"} 0
inbound_packets_total{ip_version="4",ip_dest="192.168.0.215",protocol="tcp"} 0
inbound_packets_total{ip_version="4",ip_dest="192.168.0.215",protocol="udp"} 0
inbound_packets_total{ip_version="4",ip_dest="192.168.0.215",protocol="other"} 0

# HELP inbound_bytes_total Bytes entering the network
# TYPE inbound_bytes_total counter
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.100",protocol="icmp"} 4998
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.100",protocol="tcp"} 854269
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.100",protocol="udp"} 28922
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.100",protocol="other"} 0
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.215",protocol="icmp"} 0
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.215",protocol="tcp"} 0
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.215",protocol="udp"} 0
inbound_bytes_total{ip_version="4",ip_dest="192.168.0.215",protocol="other"} 0

# HELP outbound_packets_total Packets leaving the network
# TYPE outbound_packets_total counter
outbound_packets_total{ip_version="4",ip_source="192.168.0.100",protocol="icmp"} 51
outbound_packets_total{ip_version="4",ip_source="192.168.0.100",protocol="tcp"} 1531
outbound_packets_total{ip_version="4",ip_source="192.168.0.100",protocol="udp"} 155
outbound_packets_total{ip_version="4",ip_source="192.168.0.100",protocol="other"} 0
outbound_packets_total{ip_version="4",ip_source="192.168.0.215",protocol="icmp"} 0
outbound_packets_total{ip_version="4",ip_source="192.168.0.215",protocol="tcp"} 0
outbound_packets_total{ip_version="4",ip_source="192.168.0.215",protocol="udp"} 1
outbound_packets_total{ip_version="4",ip_source="192.168.0.215",protocol="other"} 0

# HELP outbound_bytes_total Bytes leaving the network
# TYPE outbound_bytes_total counter
outbound_bytes_total{ip_version="4",ip_source="192.168.0.100",protocol="icmp"} 4998
outbound_bytes_total{ip_version="4",ip_source="192.168.0.100",protocol="tcp"} 479360
outbound_bytes_total{ip_version="4",ip_source="192.168.0.100",protocol="udp"} 16964
outbound_bytes_total{ip_version="4",ip_source="192.168.0.100",protocol="other"} 0
outbound_bytes_total{ip_version="4",ip_source="192.168.0.215",protocol="icmp"} 0
outbound_bytes_total{ip_version="4",ip_source="192.168.0.215",protocol="tcp"} 0
outbound_bytes_total{ip_version="4",ip_source="192.168.0.215",protocol="udp"} 132
outbound_bytes_total{ip_version="4",ip_source="192.168.0.215",protocol="other"} 0
```

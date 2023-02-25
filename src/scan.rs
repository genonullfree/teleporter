use crate::errors::TeleportError;
use ipnetwork::IpNetwork;
use pnet_datalink::interfaces;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;

use crate::ScanOpt;

pub fn run(opt: ScanOpt) -> Result<(), TeleportError> {
    let ifs = interfaces();
    let localv4 = IpNetwork::V4("127.0.0.1/8".parse().unwrap());

    for i in ifs {
        if !i.ips.is_empty() {
            if i.ips.contains(&localv4) {
                continue;
            }
            for v in &i.ips {
                if v.is_ipv4() {
                    scan_network(&v, opt.port);
                }
            }
        }
    }

    Ok(())
}

fn scan_network(network: &IpNetwork, port: u16) {
    for i in network.iter() {
        let sa = format!("{}:{port}", i);
        let socket = sa.to_socket_addrs().unwrap();
        println!("{socket:?}");
    }
}

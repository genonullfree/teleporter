use crate::errors::TeleportError;
use crate::teleport::{
    TeleportAction, TeleportFeatures, TeleportInit, TeleportInitAck, TeleportStatus,
};
use crate::utils;
use ipnetwork::IpNetwork;
use pnet_datalink::interfaces;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::time::Duration;

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
                    scan_network(v, opt.port)?;
                }
            }
        }
    }

    Ok(())
}

fn scan_network(network: &IpNetwork, port: u16) -> Result<(), TeleportError> {
    for i in network.iter().skip(1) {
        let sa = format!("{}:{port}", i);
        let socket = sa.to_socket_addrs().unwrap();
        for s in socket {
            if let Ok(ack) = ping(&s) {
                println!("Teleporter v{} detected on {sa}", ack.version.to_string());
            };
        }
    }

    Ok(())
}

fn ping(ip_addr: &SocketAddr) -> Result<TeleportInitAck, TeleportError> {
    let stream = TcpStream::connect_timeout(ip_addr, Duration::new(0, 50000))?;
    query(stream)
}

fn query(mut stream: TcpStream) -> Result<TeleportInitAck, TeleportError> {
    let header = TeleportInit::new(TeleportFeatures::Ping);

    utils::send_packet(
        &mut stream,
        TeleportAction::Ping,
        &None,
        header.serialize()?,
    )?;

    let packet = utils::recv_packet(&mut stream, &None)?;
    let mut ack = TeleportInitAck::default();
    ack.deserialize(&packet.data)?;
    if ack.status != TeleportStatus::Pong as u8 {
        Err(TeleportError::InvalidStatusCode)
    } else {
        Ok(ack)
    }
}

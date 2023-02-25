use crate::errors::TeleportError;
use pnet_datalink::interfaces;
use ipnetwork::IpNetwork;
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
                    println!("{:?}", i.ips);
                }
            }
        }
    }

    Ok(())
}

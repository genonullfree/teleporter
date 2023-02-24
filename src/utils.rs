use crate::errors::TeleportError;
use crate::teleport::{TeleportAction, TeleportEnc, TeleportHeader, TeleportInit};
use crate::PROTOCOL;
use byteorder::{LittleEndian, ReadBytesExt};
use rand::prelude::*;
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;

pub fn print_updates(received: f64, header: &TeleportInit) {
    let units = UpdateUnit::update(received, header.filesize as f64);
    print!(
        "\r => {:>8.03}{} of {:>8.03}{} ({:02.02}%)",
        units.partial.value, units.partial.unit, units.total.value, units.total.unit, units.percent
    );
    io::stdout().flush().expect("Fatal IO error");
}

struct UpdateUnit {
    partial: SizeUnit,
    total: SizeUnit,
    percent: f64,
}

impl UpdateUnit {
    pub fn update(partial: f64, total: f64) -> Self {
        let percent: f64 = (partial / total) * 100f64;
        let p = SizeUnit::identify(partial);
        let t = SizeUnit::identify(total);

        UpdateUnit {
            partial: p,
            total: t,
            percent,
        }
    }
}

struct SizeUnit {
    value: f64,
    unit: char,
}

impl SizeUnit {
    pub fn identify(mut value: f64) -> Self {
        let unit = ['B', 'K', 'M', 'G', 'T'];

        let mut count = 0;
        loop {
            if (value / 1024.0) > 1.0 {
                count += 1;
                value /= 1024.0;
            } else {
                break;
            }
            if count == unit.len() - 1 {
                break;
            }
        }

        SizeUnit {
            value,
            unit: unit[count],
        }
    }
}

pub fn send_packet(
    sock: &mut TcpStream,
    action: TeleportAction,
    enc: &Option<TeleportEnc>,
    data: Vec<u8>,
) -> Result<(), TeleportError> {
    let mut header = TeleportHeader::new(action);

    // If encryption is enabled
    if let Some(ctx) = enc {
        // Use random IV
        let mut rng = StdRng::from_entropy();
        let mut iv: [u8; 12] = [0; 12];
        rng.fill(&mut iv);

        header.action |= TeleportAction::Encrypted as u8;

        // Encrypt the data array
        header.data = ctx.encrypt(&iv, &data)?;

        // Set the IV in the header
        header.iv = Some(iv);
    } else {
        header.data = data;
    }

    // Serialize the message
    let message = header.serialize()?;

    // Send the packet
    sock.write_all(&message)?;
    sock.flush()?;

    Ok(())
}

pub fn recv_packet(
    sock: &mut TcpStream,
    dec: &Option<TeleportEnc>,
) -> Result<TeleportHeader, TeleportError> {
    let mut initbuf: [u8; 13] = [0; 13];
    loop {
        let len = sock.peek(&mut initbuf)?;
        if len == 13 {
            break;
        }
    }

    let mut init: &[u8] = &initbuf;
    let protocol = init.read_u64::<LittleEndian>()?;
    if protocol != PROTOCOL {
        return Err(TeleportError::InvalidProtocol);
    }

    let packet_len = init.read_u32::<LittleEndian>()?;
    let action = init.read_u8()?;

    // Include IV size in length
    let mut total_len = 13 + packet_len as usize;
    let encrypted = action & TeleportAction::Encrypted as u8 == TeleportAction::Encrypted as u8;
    if encrypted {
        total_len += 12;
    }

    let mut buf = Vec::<u8>::new();
    buf.resize(total_len, 0);

    sock.read_exact(&mut buf)?;

    let mut out = TeleportHeader::new(TeleportAction::Init);
    out.deserialize(buf)?;

    if encrypted {
        out.action ^= TeleportAction::Encrypted as u8;
        if let Some(ctx) = dec {
            out.data = ctx.decrypt(&out.iv.expect("Fatal decrypt error"), &out.data)?;
        }
    }

    Ok(out)
}

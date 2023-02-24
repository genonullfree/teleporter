use crate::errors::TeleportError;
use crate::teleport;
use crate::teleport::{
    TeleportAction, TeleportEnc, TeleportFeatures, TeleportHeader, TeleportInit,
};
use crate::PROTOCOL;
use byteorder::{LittleEndian, ReadBytesExt};
use rand::prelude::*;
use std::fs::File;
use std::hash::Hasher;
use std::io;
use std::io::{Error, Read, Seek, Write};
use std::net::TcpStream;
use xxhash_rust::xxh3;

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

fn gen_chunk_size(file_size: u64) -> usize {
    let mut chunk = 1024;
    loop {
        if file_size / chunk > 2048 {
            chunk *= 2;
        } else {
            break;
        }
    }

    if chunk > u32::MAX as u64 {
        u32::MAX as usize
    } else {
        chunk as usize
    }
}

pub fn add_feature(opt: &mut Option<u32>, add: TeleportFeatures) -> Result<(), Error> {
    if let Some(o) = opt {
        *o |= add as u32;
        *opt = Some(*o);
    } else {
        *opt = Some(add as u32);
    }

    Ok(())
}

pub fn check_feature(opt: &Option<u32>, check: TeleportFeatures) -> bool {
    if let Some(o) = opt {
        if o & check as u32 == check as u32 {
            return true;
        }
    }

    false
}

// Called from server
pub fn calc_delta_hash(mut file: &File) -> Result<teleport::TeleportDelta, TeleportError> {
    let meta = file.metadata()?;
    let file_size = meta.len();

    file.rewind()?;
    let mut buf = Vec::<u8>::new();
    buf.resize(gen_chunk_size(meta.len()), 0);
    let mut whole_hasher = xxh3::Xxh3::new();
    let mut chunk_hash = Vec::<u64>::new();

    loop {
        let mut hasher = xxh3::Xxh3::new();
        // Read a chunk of the file
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(TeleportError::Io(s)),
        };
        if len == 0 {
            break;
        }

        hasher.write(&buf);
        chunk_hash.push(hasher.finish());

        whole_hasher.write(&buf);
    }

    let mut out = teleport::TeleportDelta::new();
    out.filesize = file_size;
    out.chunk_size = buf.len().try_into()?;
    out.hash = whole_hasher.finish();
    out.chunk_hash = chunk_hash;

    file.rewind()?;

    Ok(out)
}

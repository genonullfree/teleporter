use crate::teleport::TeleportInit;
use crate::teleport::{TeleportAction, TeleportEnc, TeleportHeader};
use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use rand::prelude::*;

struct SizeUnit {
    value: f64,
    unit: char,
}

struct UpdateUnit {
    partial: SizeUnit,
    total: SizeUnit,
    percent: f64,
}

pub fn print_updates(received: f64, header: &TeleportInit) {
    let units = update_units(received as f64, header.filesize as f64);
    print!(
        "\r => {:>8.03}{} of {:>8.03}{} ({:02.02}%)",
        units.partial.value, units.partial.unit, units.total.value, units.total.unit, units.percent
    );
    io::stdout().flush().unwrap();
}

fn update_units(partial: f64, total: f64) -> UpdateUnit {
    let percent: f64 = (partial as f64 / total as f64) * 100f64;
    let p = identify_unit(partial);
    let t = identify_unit(total);

    UpdateUnit {
        partial: p,
        total: t,
        percent,
    }
}

fn identify_unit(mut value: f64) -> SizeUnit {
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

pub fn send_packet(
    sock: &mut TcpStream,
    action: TeleportAction,
    enc: Option<TeleportEnc>,
    data: Vec<u8>,
) -> Result<(), Error> {
    let mut header = TeleportHeader::new(action);

    // If encryption is enabled
    if let Some(ctx) = enc {
        // Use random IV
        let mut rng = StdRng::from_entropy();
        let mut iv: [u8; 12] = [0; 12];
        rng.fill(&mut iv);

        // Encrypt the data array
        header.data = ctx.encrypt(&iv, &data)?;

        // Set the IV in the header
        header.iv = Some(iv);
    } else {
        header.data = data;
    }

    // Serialize the message
    let message = header.serialize();

    // Send the packet
    sock.write_all(&message)?;
    sock.flush()?;

    Ok(())
}

pub fn recv_packet(
    sock: &mut TcpStream,
    dec: Option<TeleportEnc>,
) -> Result<TeleportHeader, Error> {
    let mut initbuf: [u8; 13] = [0; 13];
    sock.peek(&mut initbuf)?;

    let mut init: &[u8] = &initbuf;
    let protocol = init.read_u64::<LittleEndian>().unwrap();
    if protocol != PROTOCOL {
        println!("protocol recv: {:08x?}", protocol);
        return Err(Error::new(ErrorKind::InvalidData, "Invalid protocol"));
    }

    let packet_len = init.read_u32::<LittleEndian>().unwrap();
    let action = init.read_u8().unwrap();

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
        if let Some(ctx) = dec {
            out.data = ctx.decrypt(&out.iv.unwrap(), &out.data)?;
        }
    }

    Ok(out)
}

fn gen_chunk_size(file_size: u64) -> usize {
    let mut chunk = 1024;
    loop {
        if file_size / chunk > 150 {
            chunk *= 2;
        } else {
            break;
        }
    }

    chunk as usize
}

pub fn calc_file_hash(filename: String) -> Result<Hash, Error> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = Vec::<u8>::new();

    let mut file = File::open(filename)?;
    let meta = file.metadata()?;

    buf.resize(gen_chunk_size(meta.len()), 0);

    file.seek(SeekFrom::Start(0))?;

    loop {
        // Read a chunk of the file
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(s),
        };
        if len == 0 {
            break;
        }

        hasher.update(&buf);
    }

    file.seek(SeekFrom::Start(0))?;

    Ok(hasher.finalize())
}

pub fn calc_delta_hash(mut file: &File) -> Result<teleport::TeleportDelta, Error> {
    let meta = file.metadata()?;
    let file_size = meta.len();

    file.seek(SeekFrom::Start(0))?;
    let mut buf = Vec::<u8>::new();
    buf.resize(gen_chunk_size(meta.len()), 0);
    let mut hasher = blake3::Hasher::new();
    let mut whole_hasher = blake3::Hasher::new();
    let mut delta_checksum = Vec::<Hash>::new();

    loop {
        // Read a chunk of the file
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(s),
        };
        if len == 0 {
            break;
        }

        hasher.update(&buf);
        delta_checksum.push(hasher.finalize());
        hasher.reset();

        whole_hasher.update(&buf);
    }

    let mut out = teleport::TeleportDelta::new();
    out.filesize = file_size as u64;
    out.chunk_size = buf.len() as u64;
    out.checksum = whole_hasher.finalize();
    out.delta_checksum = delta_checksum;

    file.seek(SeekFrom::Start(0))?;

    Ok(out)
}

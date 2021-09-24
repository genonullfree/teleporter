use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::convert::{TryFrom, TryInto};

struct SizeUnit {
    partial: f64,
    partial_unit: char,
    total: f64,
    total_unit: char,
}

pub fn print_updates(received: f64, header: &TeleportInit) {
    let percent: f64 = (received as f64 / header.filesize as f64) * 100f64;
    let units = convert_units(received as f64, header.filesize as f64);
    print!(
        "\r => {:>8.03}{} of {:>8.03}{} ({:02.02}%)",
        units.partial, units.partial_unit, units.total, units.total_unit, percent
    );
    io::stdout().flush().unwrap();
}

fn convert_units(mut partial: f64, mut total: f64) -> SizeUnit {
    let unit = ['B', 'K', 'M', 'G', 'T'];
    let mut out = SizeUnit {
        partial: 0.0,
        partial_unit: 'B',
        total: 0.0,
        total_unit: 'B',
    };

    let mut count = 0;
    loop {
        if (total / 1024.0) > 1.0 {
            count += 1;
            total /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.total = total;
    out.total_unit = unit[count];

    count = 0;
    loop {
        if (partial / 1024.0) > 1.0 {
            count += 1;
            partial /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.partial = partial;
    out.partial_unit = unit[count];
    out
}

impl TeleportInit {
    pub fn new() -> TeleportInit {
        TeleportInit {
            protocol: PROTOCOL.to_string(),
            version: VERSION.to_string(),
            filename: "".to_string(),
            filenum: 0,
            totalfiles: 0,
            filesize: 0,
            chmod: 0,
            overwrite: false,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();
        let size: u32 = self.len() as u32 + 5; // sizeof(struct) + 1csum + 4len
        out.append(&mut size.to_le_bytes().to_vec());
        out.append(&mut self.protocol.clone().into_bytes());
        out.push(0);
        out.append(&mut self.version.clone().into_bytes());
        out.push(0);
        out.append(&mut self.filename.clone().into_bytes().to_vec());
        out.push(0);
        out.append(&mut self.filenum.to_le_bytes().to_vec());
        out.append(&mut self.totalfiles.to_le_bytes().to_vec());
        out.append(&mut self.filesize.to_le_bytes().to_vec());
        out.append(&mut self.chmod.to_le_bytes().to_vec());
        let bbyte = TeleportInit::bool_to_u8(self.overwrite);
        out.push(bbyte);
        let csum: u8 = out.iter().map(|x| *x as u64).sum::<u64>() as u8;
        out.push(csum);
        out
    }

    pub fn len(&self) -> usize {
        let mut out: usize = 0;
        out += self.protocol.len() + 1;
        out += self.version.len() + 1;
        out += 8; // filenum
        out += 8; // totalfiles
        out += 8; // filesize
        out += self.filename.len() + 1;
        out += 4; // chmod
        out += 1; // overwrite
        out
    }

    fn bool_to_u8(b: bool) -> u8 {
        if b {
            1
        } else {
            0
        }
    }

    fn vec_to_string(input: &[u8]) -> String {
        let mut s: String = "".to_string();
        for i in input.iter() {
            let c: char = match (*i).try_into() {
                Ok(c) => c,
                Err(_) => break,
            };
            if c.is_ascii_graphic() || c == ' ' {
                s.push(c);
            } else {
                break;
            }
        }

        s
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        let mut buf: &[u8] = &input;
        let size = buf.read_u32::<LittleEndian>().unwrap() as usize;
        if input.len() < size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Not enough data received",
            ));
        }
        let mut ofs = 4;
        self.protocol = TeleportInit::vec_to_string(&input[ofs..]);
        ofs += self.protocol.len() + 1;
        self.version = TeleportInit::vec_to_string(&input[ofs..]);
        ofs += self.version.len() + 1;
        self.filename = TeleportInit::vec_to_string(&input[ofs..]);
        ofs += self.filename.len() + 1;
        let mut buf: &[u8] = &input[ofs..];
        self.filenum = buf.read_u64::<LittleEndian>().unwrap();
        self.totalfiles = buf.read_u64::<LittleEndian>().unwrap();
        self.filesize = buf.read_u64::<LittleEndian>().unwrap();
        self.chmod = buf.read_u32::<LittleEndian>().unwrap();
        self.overwrite = buf.read_u8().unwrap() > 0;
        let csumr = buf.read_u8().unwrap();
        let csum: u8 = *&input[..size - 1].iter().map(|x| *x as u64).sum::<u64>() as u8;
        if csum != csumr {
            return Err(Error::new(ErrorKind::InvalidData, "Checksum is invalid"));
        }
        Ok(())
    }
}

impl TryFrom<u8> for TeleportStatus {
    type Error = &'static str;

    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            x if x == TeleportStatus::Proceed as u8 => Ok(TeleportStatus::Proceed),
            x if x == TeleportStatus::Overwrite as u8 => Ok(TeleportStatus::Overwrite),
            x if x == TeleportStatus::NoOverwrite as u8 => Ok(TeleportStatus::NoOverwrite),
            x if x == TeleportStatus::NoSpace as u8 => Ok(TeleportStatus::NoSpace),
            x if x == TeleportStatus::NoPermission as u8 => Ok(TeleportStatus::NoPermission),
            x if x == TeleportStatus::WrongVersion as u8 => Ok(TeleportStatus::WrongVersion),
            _ => Err("TeleportStatus is invalid"),
        }
    }
}

impl TeleportResponse {
    pub fn new(status: TeleportStatus) -> TeleportResponse {
        TeleportResponse { ack: status }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();
        out.push(self.ack as u8);
        out
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        let mut buf: &[u8] = &input;
        self.ack = buf.read_u8().unwrap().try_into().unwrap();
        Ok(())
    }
}

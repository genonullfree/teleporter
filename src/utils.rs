use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::convert::{TryFrom, TryInto};

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
        let size: u32 = self.size() as u32 + 5; // sizeof(struct) + 1csum + 4len
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

    pub fn size(&self) -> usize {
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
        let csum: u8 = input[..size - 1].iter().map(|x| *x as u64).sum::<u64>() as u8;
        if csum != csumr {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "TeleportInit checksum is invalid",
            ));
        }
        Ok(())
    }
}

impl Default for TeleportInit {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for TeleportInit {
    fn eq(&self, other: &Self) -> bool {
        self.protocol == other.protocol
            && self.version == other.version
            && self.filename == other.filename
            && self.filenum == other.filenum
            && self.totalfiles == other.totalfiles
            && self.filesize == other.filesize
            && self.chmod == other.chmod
            && self.overwrite == other.overwrite
    }
}

impl TryFrom<u8> for TeleportInitStatus {
    type Error = &'static str;

    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            x if x == TeleportInitStatus::Proceed as u8 => Ok(TeleportInitStatus::Proceed),
            x if x == TeleportInitStatus::Overwrite as u8 => Ok(TeleportInitStatus::Overwrite),
            x if x == TeleportInitStatus::NoOverwrite as u8 => Ok(TeleportInitStatus::NoOverwrite),
            x if x == TeleportInitStatus::NoSpace as u8 => Ok(TeleportInitStatus::NoSpace),
            x if x == TeleportInitStatus::NoPermission as u8 => {
                Ok(TeleportInitStatus::NoPermission)
            }
            x if x == TeleportInitStatus::WrongVersion as u8 => {
                Ok(TeleportInitStatus::WrongVersion)
            }
            _ => Err("TeleportInitStatus is invalid"),
        }
    }
}

//impl TryFrom<u8> for TeleportDataStatus

impl TeleportInitAck {
    pub fn new(status: TeleportInitStatus) -> TeleportInitAck {
        TeleportInitAck {
            ack: status,
            version: VERSION.to_string(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = vec![self.ack as u8];
        out.append(&mut self.version.clone().into_bytes());
        out.push(0);
        let csum: u8 = out.iter().map(|x| *x as u64).sum::<u64>() as u8;
        out.push(csum);
        out
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        let mut buf: &[u8] = &input;
        let size = input.len();
        self.ack = buf.read_u8().unwrap().try_into().unwrap();
        self.version = TeleportInit::vec_to_string(&input[1..]);
        let csumr = input[size - 1];
        let csum: u8 = input[..size - 2].iter().map(|x| *x as u64).sum::<u64>() as u8;
        if csum != csumr {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "TeleportInitAck checksum is invalid",
            ));
        }

        Ok(())
    }
}

/*impl TeleportDataAck {
    pub fn new(status: TeleportDataStatus) -> TeleportDataAck {
        TeleportDataAck { ack: status }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = vec![self.ack as u8];
        let csum: u8 = out.iter().map(|x| *x as u64).sum::<u64>() as u8;
        out.push(csum);

        out
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        let mut buf: &[u8] = &input;
        let size = input.len();
        self.ack = buf.read_u8().unwrap().try_into().unwrap();
        let csumr = input[size - 1];
        let csum: u8 = input[..size - 1].iter().map(|x| *x as u64).sum::<u64>() as u8;
        if csum != csumr {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "TeleportDataAck checksum is invalid",
            ));
        }

        Ok(())
    }
}*/

impl Default for TeleportData {
    fn default() -> Self {
        Self::new()
    }
}

impl TeleportData {
    pub fn new() -> TeleportData {
        TeleportData {
            length: 0,
            offset: 0,
            data: vec![],
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();
        let len: u32 = self.data.len() as u32;
        out.append(&mut len.to_le_bytes().to_vec());
        out.append(&mut self.offset.to_le_bytes().to_vec());
        out.append(&mut self.data.clone());
        let csum: u8 = out.iter().map(|x| *x as u64).sum::<u64>() as u8;
        out.push(csum);
        out
    }

    pub fn size(&self) -> usize {
        self.data.len() + 4 + 8
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let size = input.len();
        let mut buf: &[u8] = input;
        self.length = buf.read_u32::<LittleEndian>().unwrap();
        self.offset = buf.read_u64::<LittleEndian>().unwrap();
        self.data = input[12..size - 1].to_vec();
        let csumr = input[size - 1];
        let csum: u8 = input[..size - 1].iter().map(|x| *x as u64).sum::<u64>() as u8;
        if csum != csumr {
            println!("data: {:?}", self.data);
            println!("\nlength: {} offset: {}", self.length, self.offset);
            println!("expected: {}, received: {}", csum, csumr);
            return Err(Error::new(
                ErrorKind::InvalidData,
                "TeleportData checksum is invalid",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTINIT: &[u8] = &[
        62, 0, 0, 0, 84, 69, 76, 69, 80, 79, 82, 84, 0, 48, 46, 50, 46, 50, 0, 116, 101, 115, 116,
        102, 105, 108, 101, 46, 98, 105, 110, 0, 1, 0, 0, 0, 0, 0, 0, 0, 231, 3, 0, 0, 0, 0, 0, 0,
        41, 35, 0, 0, 0, 0, 0, 0, 243, 2, 0, 0, 1, 145,
    ];
    const TESTINITACK: &[u8] = &[5, 48, 46, 50, 46, 51, 0, 246];
    const TESTDATA: &[u8] = &[4, 0, 0, 0, 184, 34, 0, 0, 0, 0, 0, 0, 10, 10, 32, 3, 21];

    #[test]
    fn test_update_unit() {
        let pe = 2.0;
        let te = 1_234_567_890_123_456.0;
        let s = update_units(pe, te);
        assert_eq!(s.partial.unit, 'B');
        assert_eq!(s.total.unit, 'T');
    }

    #[test]
    fn test_teleportinit_serialize() {
        let t: TeleportInit = TeleportInit {
            protocol: PROTOCOL.to_string(),
            version: "0.2.2".to_string(),
            filename: "testfile.bin".to_string(),
            filenum: 1,
            totalfiles: 999,
            filesize: 9001,
            chmod: 00755,
            overwrite: true,
        };
        let s = t.serialize();
        assert_eq!(s, TESTINIT);
    }

    #[test]
    fn test_teleportinit_deserialize() {
        let t: TeleportInit = TeleportInit {
            protocol: PROTOCOL.to_string(),
            version: "0.2.2".to_string(),
            filename: "testfile.bin".to_string(),
            filenum: 1,
            totalfiles: 999,
            filesize: 9001,
            chmod: 00755,
            overwrite: true,
        };
        let mut te = TeleportInit::new();
        te.deserialize(TESTINIT.to_vec()).unwrap();
        assert_eq!(te, t);
    }

    #[test]
    fn test_teleportinitack_serialize() {
        let mut t = TeleportInitAck::new(TeleportInitStatus::WrongVersion);
        t.version = "0.2.3".to_string();
        let te = t.serialize();

        assert_eq!(te, TESTINITACK);
    }

    #[test]
    fn test_teleportinitack_deserialize() {
        let mut te = TeleportInitAck::new(TeleportInitStatus::Proceed);
        let test = TeleportInitAck {
            ack: TeleportInitStatus::WrongVersion,
            version: "0.2.3".to_string(),
        };

        te.deserialize(TESTINITACK.to_vec()).unwrap();
        te.version = "0.2.3".to_string();
        assert_eq!(test, te);
    }

    #[test]
    fn test_teleportdata_serialize() {
        let t = TeleportData {
            length: 4,
            offset: 8888,
            data: vec![0x0a, 0x0a, 0x20, 0x03],
        }
        .serialize();
        assert_eq!(t, TESTDATA);
    }

    #[test]
    fn test_teleportdata_deserialize() {
        let mut t = TeleportData::new();
        t.deserialize(TESTDATA).unwrap();
        let test = TeleportData {
            length: 4,
            offset: 8888,
            data: vec![0x0a, 0x0a, 0x20, 0x03],
        };
        assert_eq!(t, test);
    }
}

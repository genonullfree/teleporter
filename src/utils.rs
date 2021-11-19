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

fn send_packet(
    sock: &mut TcpStream,
    action: TeleportAction,
    enc: Option<TeleportEnc>,
    mut data: &mut Vec<u8>,
) -> Result<(), Error> {
    let mut header = TeleportHeader::new(action);

    // If encryption is enabled
    if let Some(ctx) = enc {
        // Use random IV
        let mut rng = StdRng::from_entropy();
        let mut iv: [u8; 12] = [0; 12];
        rng.fill(&mut iv);

        // Encrypt the data array
        *data = ctx.encrypt(&iv, data)?;

        // Set the IV in the header
        header.iv = Some(iv);
    }

    // Serialize the message
    let message = header.serialize();

    // Send the packet
    sock.write_all(&message)?;

    Ok(())
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

pub fn calc_delta_hash(mut file: &File) -> Result<TeleportDelta, Error> {
    let meta = file.metadata()?;
    let file_size = meta.len();

    file.seek(SeekFrom::Start(0))?;
    let mut buf = Vec::<u8>::new();
    buf.resize(gen_chunk_size(meta.len()), 0);
    let mut hasher = blake3::Hasher::new();
    let mut whole_hasher = blake3::Hasher::new();
    let mut delta_csum = Vec::<Hash>::new();

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
        delta_csum.push(hasher.finalize());
        hasher.reset();

        whole_hasher.update(&buf);
    }

    let out = TeleportDelta {
        size: file_size as u64,
        delta_size: buf.len() as u64,
        csum: whole_hasher.finalize(),
        delta_csum,
    };

    file.seek(SeekFrom::Start(0))?;

    Ok(out)
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

fn generate_checksum(input: &[u8]) -> u8 {
    input.iter().map(|x| *x as u64).sum::<u64>() as u8
}

fn validate_checksum(input: &[u8]) -> Result<(), Error> {
    if input.len() < 2 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Vector is too short to validate checksum",
        ));
    }

    let csum: u8 = input[..input.len() - 1]
        .iter()
        .map(|x| *x as u64)
        .sum::<u64>() as u8;
    if csum != *input.last().unwrap() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Teleport checksum is invalid",
        ));
    }

    Ok(())
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
        out.push(generate_checksum(&out));
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

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        validate_checksum(&input)?;
        let mut buf: &[u8] = &input;
        let size = buf.read_u32::<LittleEndian>().unwrap() as usize;
        if input.len() < size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Not enough data received",
            ));
        }
        let mut ofs = 4;
        self.protocol = vec_to_string(&input[ofs..]);
        ofs += self.protocol.len() + 1;
        self.version = vec_to_string(&input[ofs..]);
        ofs += self.version.len() + 1;
        self.filename = vec_to_string(&input[ofs..]);
        ofs += self.filename.len() + 1;
        let mut buf: &[u8] = &input[ofs..];
        self.filenum = buf.read_u64::<LittleEndian>().unwrap();
        self.totalfiles = buf.read_u64::<LittleEndian>().unwrap();
        self.filesize = buf.read_u64::<LittleEndian>().unwrap();
        self.chmod = buf.read_u32::<LittleEndian>().unwrap();
        self.overwrite = buf.read_u8().unwrap() > 0;
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

impl TeleportInitAck {
    pub fn new(status: TeleportInitStatus) -> TeleportInitAck {
        TeleportInitAck {
            ack: status,
            version: VERSION.to_string(),
            delta: None,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = vec![self.ack as u8];
        out.append(&mut self.version.clone().into_bytes());
        out.push(0);
        match &self.delta {
            Some(d) => {
                out.append(&mut d.size.to_le_bytes().to_vec());
                out.append(&mut d.delta_size.to_le_bytes().to_vec());
                out.append(&mut d.csum.as_bytes().to_vec());
                out.append(&mut TeleportInitAck::csum_serial(&d.delta_csum));
            }
            None => {}
        };
        out.push(generate_checksum(&out));
        out
    }

    fn csum_serial(input: &[Hash]) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        for i in input {
            out.append(&mut i.as_bytes().to_vec());
        }

        out
    }

    fn csum_deserial(input: &[u8]) -> Result<Vec<Hash>, Error> {
        if input.len() % 32 != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Cannot deserialize Vec<Hash>",
            ));
        }

        let mut out = Vec::<Hash>::new();

        for i in input.chunks(32) {
            let a: [u8; 32] = i.try_into().unwrap();
            let h: Hash = a.try_into().unwrap();
            out.push(h);
        }

        Ok(out)
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        validate_checksum(&input)?;
        let mut buf: &[u8] = &input;
        let size = input.len();
        self.ack = buf.read_u8().unwrap().try_into().unwrap();
        self.version = vec_to_string(&input[1..]);
        if size > self.version.len() + 3 {
            buf = &input[self.version.len() + 2..];
            let c: [u8; 32] = input[self.version.len() + 2 + 16..self.version.len() + 2 + 16 + 32]
                .try_into()
                .unwrap();
            self.delta = Some(TeleportDelta {
                size: buf.read_u64::<LittleEndian>().unwrap(),
                delta_size: buf.read_u64::<LittleEndian>().unwrap(),
                csum: c.try_into().unwrap(),
                delta_csum: TeleportInitAck::csum_deserial(
                    &input[self.version.len() + 2 + 16 + 32..size - 1],
                )?,
            });
        } else {
            self.delta = None;
        }

        Ok(())
    }
}

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
        out.push(generate_checksum(&out));
        out
    }

    pub fn size(&self) -> usize {
        self.data.len() + 4 + 8
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        validate_checksum(&input.to_vec())?;
        let size = input.len();
        let mut buf: &[u8] = input;
        self.length = buf.read_u32::<LittleEndian>().unwrap();
        self.offset = buf.read_u64::<LittleEndian>().unwrap();
        self.data = input[12..size - 1].to_vec();

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
            delta: None,
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

    #[test]
    fn test_generate_checksum() {
        let t = TESTINITACK[..TESTINITACK.len() - 1].to_vec();
        let c = generate_checksum(&t);
        assert_eq!(c, TESTINITACK[TESTINITACK.len() - 1]);
    }

    #[test]
    fn test_validate_checksum() {
        assert_eq!((), validate_checksum(&TESTINITACK.to_vec()).unwrap());
    }
}

use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use rand::prelude::*;

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

    let checksum: u8 = input[..input.len() - 1]
        .iter()
        .map(|x| *x as u64)
        .sum::<u64>() as u8;
    if checksum != *input.last().unwrap() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Teleport checksum is invalid",
        ));
    }

    Ok(())
}

fn vec_to_string(input: &[u8], size: u16) -> Vec<char> {
    let mut s = Vec::<char>::new();
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
        if s.len() == size.into() {
            break;
        }
    }

    s
}

fn read_version(mut input: &[u8]) -> [u16; 3] {
    let mut out: [u16; 3] = [0; 3];
    for i in &mut out {
        *i = input.read_u16::<LittleEndian>().unwrap();
    }
    out
}

fn write_version(input: [u16; 3]) -> Vec<u8> {
    let mut out = Vec::<u8>::new();
    for i in &input {
        out.append(&mut i.to_le_bytes().to_vec());
    }
    out
}

#[derive(Debug, PartialEq)]
pub struct TeleportHeader {
    protocol: u64,
    //data_len: u32,
    action: TeleportAction,
    iv: Option<[u8; 12]>,
    data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TeleportAction {
    Init = 0x01,
    InitAck = 0x02,
    TxFile = 0x10,
    RxFile = 0x20,
    Data = 0x40,
    Encrypted = 0x80,
}

impl TeleportHeader {
    pub fn new() -> TeleportHeader {
        TeleportHeader {
            protocol: PROTOCOL_NEXT,
            action: TeleportAction::Init,
            iv: None,
            data: Vec::<u8>::new(),
        }
    }

    pub fn serialize(&mut self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add Protocol identifier
        out.append(&mut self.protocol.to_le_bytes().to_vec());

        // Add data length
        let data_len = self.data.len() as u32;
        out.append(&mut data_len.to_le_bytes().to_vec());

        // Add action code
        let mut action = self.action as u8;
        if self.iv.is_some() {
            action |= TeleportAction::Encrypted as u8;
        }
        out.push(action);

        // If Encrypted, add IV
        if let Some(iv) = self.iv {
            out.append(&mut iv[..].to_vec());
        };

        // Add data
        out.append(&mut self.data.clone());

        out
    }

    pub fn deserialize(&mut self, input: Vec<u8>) -> Result<(), Error> {
        let mut buf: &[u8] = &input;

        // Extract Protocol
        self.protocol = buf.read_u64::<LittleEndian>().unwrap();
        if self.protocol != PROTOCOL_NEXT {
            return Err(Error::new(ErrorKind::InvalidData, "Error reading protocol"));
        }

        // Extract data length
        let data_len = buf.read_u32::<LittleEndian>().unwrap() as usize;

        // Extract action code
        let action = buf.read_u8().unwrap();

        // If Encrypted, extract IV
        if (action & TeleportAction::Encrypted as u8) == TeleportAction::Encrypted as u8 {
            if input.len() < 25 {
                return Err(Error::new(ErrorKind::InvalidData, "Not enough data for IV"));
            }
            let iv: [u8; 12] = input[13..25].try_into().expect("Error reading IV");
            self.iv = Some(iv);
        }

        // Extract data
        self.data = input[25..].to_vec();
        if self.data.len() != data_len {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Data is not the expected length",
            ));
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TeleportEnc {
    secret: [u8; 32],
    privkey: [u8; 64],
    remote: [u8; 32],
    public: TeleportEcdhExchange,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TeleportEcdhExchange {
    key: [u8; 32],
}

impl TeleportEnc {
    pub fn new() -> TeleportEnc {
        let (privkey, pubkey) = crypto::genkey();
        TeleportEnc {
            secret: [0; 32],
            remote: [0; 32],
            privkey,
            public: TeleportEcdhExchange { key: pubkey },
        }
    }

    pub fn serialize(self) -> Vec<u8> {
        self.public.key.to_vec()
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        if input.len() < 32 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Not enough data for public key",
            ));
        }

        self.remote = input[..32].try_into().expect("Error reading public key");

        Ok(())
    }

    pub fn calc_secret(&mut self) {
        self.secret = crypto::calc_secret(&self.remote, &self.privkey)
    }

    pub fn encrypt(self, nonce: &[u8; 12], input: &[u8]) -> Result<Vec<u8>, Error> {
        crypto::encrypt(&self.secret, nonce.to_vec(), input.to_vec())
    }

    pub fn decrypt(self, nonce: &[u8; 12], input: &[u8]) -> Result<Vec<u8>, Error> {
        crypto::decrypt(&self.secret, nonce.to_vec(), input.to_vec())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportInit {
    version: [u16; 3],
    features: u32,
    chmod: u32,
    filesize: u64,
    filename_len: u16,
    filename: Vec<char>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TeleportFeatures {
    NewFile = 0x01,
    Delta = 0x02,
    Overwrite = 0x04,
    Backup = 0x08,
    Rename = 0x10,
}

impl TeleportInit {
    pub fn new(features: TeleportFeatures) -> TeleportInit {
        let version = Version::parse(VERSION).unwrap();

        TeleportInit {
            version: [
                version.major as u16,
                version.minor as u16,
                version.patch as u16,
            ],
            features: features as u32,
            chmod: 0o644,
            filesize: 0,
            filename_len: 0,
            filename: Vec::<char>::new(),
        }
    }

    pub fn serialize(self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add version
        for i in self.version {
            out.append(&mut i.to_le_bytes().to_vec());
        }

        // Add features
        out.append(&mut (self.features as u32).to_le_bytes().to_vec());

        // Add chmod
        out.append(&mut self.chmod.to_le_bytes().to_vec());

        // Add filesize
        out.append(&mut self.filesize.to_le_bytes().to_vec());

        // Add filename_len
        let flen = self.filename.len() as u16;
        out.append(&mut flen.to_le_bytes().to_vec());

        // Add filename
        out.append(&mut self.filename.iter().map(|x| *x as u8).collect());

        out
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let mut buf: &[u8] = input;

        // Extract version info
        for i in &mut self.version {
            *i = buf.read_u16::<LittleEndian>().unwrap();
        }

        // Extract file command feature requests
        self.features = buf.read_u32::<LittleEndian>().unwrap();

        // Extract file chmod permissions
        self.chmod = buf.read_u32::<LittleEndian>().unwrap();

        // Extract file size
        self.filesize = buf.read_u64::<LittleEndian>().unwrap();

        // Extract filename_len
        self.filename_len = buf.read_u16::<LittleEndian>().unwrap();

        // Extract filename
        let fname = &buf[..self.filename_len as usize].to_vec();
        self.filename = fname.iter().map(|x| *x as char).collect();
        if self.filename.len() != self.filename_len as usize {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Filename incorrect length",
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportInitAck {
    status: TeleportStatus,
    version: [u16; 3],
    features: Option<u32>,
    delta: Option<TeleportDelta>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TeleportStatus {
    Proceed,
    NoOverwrite,
    NoSpace,
    NoPermission,
    WrongVersion,
    UnknownAction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportDelta {
    filesize: u64,
    checksum: Hash,
    chunk_size: u64,
    delta_checksum_len: u16,
    delta_checksum: Vec<Hash>,
}

impl TeleportDelta {
    pub fn new() -> TeleportDelta {
        TeleportDelta {
            filesize: 0,
            checksum: [0; 32].try_into().unwrap(),
            chunk_size: 0,
            delta_checksum_len: 0,
            delta_checksum: Vec::<Hash>::new(),
        }
    }

    fn delta_serial(input: &[Hash]) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        for i in input {
            out.append(&mut i.as_bytes().to_vec());
        }

        out
    }

    pub fn serialize(self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add file size
        out.append(&mut self.filesize.to_le_bytes().to_vec());

        // Add file hash
        out.append(&mut self.checksum.as_bytes().to_vec());

        // Add chunk size
        out.append(&mut self.chunk_size.to_le_bytes().to_vec());

        // Add delta vector length
        let dlen = self.delta_checksum.len() as u16;
        out.append(&mut dlen.to_le_bytes().to_vec());

        // Add delta vector
        out.append(&mut TeleportDelta::delta_serial(&self.delta_checksum));

        out
    }

    fn delta_deserial(input: &[u8]) -> Result<Vec<Hash>, Error> {
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

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let mut buf: &[u8] = input;

        self.filesize = buf.read_u64::<LittleEndian>().unwrap();

        // Extract file hash
        let csum: [u8; 32] = input[8..40].try_into().unwrap();
        self.checksum = csum.try_into().unwrap();
        let mut buf: &[u8] = &input[40..];

        // Extract chunk size
        self.chunk_size = buf.read_u64::<LittleEndian>().unwrap();

        // Extract delta vector length
        self.delta_checksum_len = buf.read_u16::<LittleEndian>().unwrap();

        // Extract delta vector
        self.delta_checksum = TeleportDelta::delta_deserial(&input[50..]).unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTHEADER: &[u8] = &[
        84, 69, 76, 69, 80, 79, 82, 84, 17, 0, 0, 0, 129, 5, 48, 46, 50, 46, 51, 0, 246, 9, 10, 11,
        12, 4, 0, 0, 0, 184, 34, 0, 0, 0, 0, 0, 0, 10, 10, 32, 3, 21,
    ];
    const TESTHEADERIV: &[u8; 12] = &[5, 48, 46, 50, 46, 51, 0, 246, 9, 10, 11, 12];
    const TESTDATA: &[u8] = &[4, 0, 0, 0, 184, 34, 0, 0, 0, 0, 0, 0, 10, 10, 32, 3, 21];
    const TESTINIT: &[u8] = &[
        0, 0, 5, 0, 5, 0, 5, 0, 0, 0, 237, 1, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0, 4, 0, 102, 105, 108,
        101,
    ];
    const TESTDELTA: &[u8] = &[
        177, 104, 222, 58, 0, 0, 0, 0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 21, 205, 91, 7, 0, 0, 0, 0, 0, 0,
    ];

    #[test]
    fn test_teleportheader_serialize() {
        let mut t = TeleportHeader::new();
        t.data.append(&mut TESTDATA.to_vec());
        t.iv = Some(*TESTHEADERIV);
        let s = t.serialize();
        assert_eq!(s, TESTHEADER);
    }

    #[test]
    fn test_teleportheader_deserialize() {
        let mut test = TeleportHeader::new();
        test.data.append(&mut TESTDATA.to_vec());
        test.iv = Some(*TESTHEADERIV);
        let mut t = TeleportHeader::new();
        t.deserialize(TESTHEADER.to_vec());
        assert_eq!(t, test);
    }

    #[test]
    fn test_teleportenc_key_exchange() {
        let mut a = TeleportEnc::new();
        let mut b = TeleportEnc::new();

        a.deserialize(&b.serialize());
        b.deserialize(&a.serialize());

        a.calc_secret();
        b.calc_secret();

        assert_eq!(a.secret, b.secret);
    }

    #[test]
    fn test_teleportenc_encrypt_decrypt() {
        let mut rng = StdRng::from_entropy();
        let mut nonce: [u8; 12] = [0; 12];

        let mut a = TeleportEnc::new();
        let mut b = TeleportEnc::new();

        a.deserialize(&b.serialize());
        b.deserialize(&a.serialize());

        a.calc_secret();
        b.calc_secret();
        assert_eq!(a.secret, b.secret);

        let data = TESTHEADER.to_vec();
        rng.fill(&mut nonce);
        let ciphertext = a.encrypt(&nonce, &data).unwrap();
        let plaintext = b.decrypt(&nonce, &ciphertext).unwrap();

        assert_eq!(plaintext, data);
    }

    #[test]
    fn test_teleportinit_serialize() {
        let mut test = TeleportInit::new(TeleportFeatures::NewFile);
        test.filename = vec!['f', 'i', 'l', 'e'];
        test.filesize = 12345;
        test.chmod = 0o755;
        test.features |= TeleportFeatures::Overwrite as u32;

        let out = test.serialize();
        assert_eq!(out, TESTINIT);
    }

    #[test]
    fn test_teleportinit_deserialize() {
        let mut test = TeleportInit::new(TeleportFeatures::NewFile);
        test.filename = vec!['f', 'i', 'l', 'e'];
        test.filename_len = test.filename.len() as u16;
        test.filesize = 12345;
        test.chmod = 0o755;
        test.features |= TeleportFeatures::Overwrite as u32;

        let mut t = TeleportInit::new(TeleportFeatures::NewFile);
        t.deserialize(TESTINIT);

        println!("{:?}", t);
        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportdelta_serialize() {
        let mut test = TeleportDelta::new();
        test.filesize = 987654321;
        test.checksum = [5; 32].try_into().unwrap();
        test.chunk_size = 123456789;
        test.delta_checksum = Vec::<Hash>::new();

        let out = test.serialize();

        assert_eq!(out, TESTDELTA);
    }

    #[test]
    fn test_teleportdelta_deserialize() {
        let mut test = TeleportDelta::new();
        test.filesize = 987654321;
        test.checksum = [5; 32].try_into().unwrap();
        test.chunk_size = 123456789;
        test.delta_checksum = Vec::<Hash>::new();

        let mut t = TeleportDelta::new();
        t.deserialize(TESTDELTA);

        assert_eq!(test, t);
    }
}

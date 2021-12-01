use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use x25519_dalek::{EphemeralSecret, PublicKey};

#[derive(Debug, PartialEq)]
pub struct TeleportHeader {
    protocol: u64,
    data_len: u32,
    pub action: u8,
    pub iv: Option<[u8; 12]>,
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TeleportAction {
    Init = 0x01,
    InitAck = 0x02,
    Ecdh = 0x04,
    EcdhAck = 0x08,
    Data = 0x40,
    Encrypted = 0x80,
}

impl TeleportHeader {
    pub fn new(action: TeleportAction) -> TeleportHeader {
        TeleportHeader {
            protocol: PROTOCOL,
            data_len: 0,
            action: action as u8,
            iv: None,
            data: Vec::<u8>::new(),
        }
    }

    pub fn serialize(&mut self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add Protocol identifier
        out.append(&mut self.protocol.to_le_bytes().to_vec());

        // Add data length
        self.data_len = self.data.len() as u32;
        out.append(&mut self.data_len.to_le_bytes().to_vec());

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
        if self.protocol != PROTOCOL {
            return Err(Error::new(ErrorKind::InvalidData, "Error reading protocol"));
        }

        // Extract data length
        self.data_len = buf.read_u32::<LittleEndian>().unwrap();
        let mut data_ofs = 13;

        // Extract action code
        let action = buf.read_u8().unwrap();
        self.action = action;

        // If Encrypted, extract IV
        if (action & TeleportAction::Encrypted as u8) == TeleportAction::Encrypted as u8 {
            if input.len() < 25 {
                return Err(Error::new(ErrorKind::InvalidData, "Not enough data for IV"));
            }
            let iv: [u8; 12] = input[13..25].try_into().expect("Error reading IV");
            self.iv = Some(iv);
            data_ofs += 12;
        }

        // Extract data
        self.data = input[data_ofs..].to_vec();
        if self.data.len() != self.data_len as usize {
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
    remote: [u8; 32],
    pub public: [u8; 32],
}

impl TeleportEnc {
    pub fn new() -> TeleportEnc {
        TeleportEnc {
            secret: [0; 32],
            remote: [0; 32],
            public: [0; 32],
        }
    }

    pub fn serialize(self) -> Vec<u8> {
        self.public.to_vec()
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

    pub fn calc_secret(&mut self, privkey: EphemeralSecret) {
        let pubkey = PublicKey::from(self.remote);
        self.secret = privkey.diffie_hellman(&pubkey).to_bytes()
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
    pub version: [u16; 3],
    pub features: u32,
    pub chmod: u32,
    pub filesize: u64,
    pub filename_len: u16,
    pub filename: Vec<char>,
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

    pub fn serialize(&self) -> Vec<u8> {
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
    pub status: u8,
    pub version: [u16; 3],
    pub features: Option<u32>,
    pub delta: Option<TeleportDelta>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TeleportStatus {
    Proceed,
    NoOverwrite,
    NoSpace,
    NoPermission,
    WrongVersion,
    RequiresEncryption,
    EncryptionError,
    UnknownAction,
}

impl TryFrom<u8> for TeleportStatus {
    type Error = std::io::Error;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == TeleportStatus::Proceed as u8 => Ok(TeleportStatus::Proceed),
            x if x == TeleportStatus::NoOverwrite as u8 => Ok(TeleportStatus::NoOverwrite),
            x if x == TeleportStatus::NoSpace as u8 => Ok(TeleportStatus::NoSpace),
            x if x == TeleportStatus::NoPermission as u8 => Ok(TeleportStatus::NoPermission),
            x if x == TeleportStatus::WrongVersion as u8 => Ok(TeleportStatus::WrongVersion),
            x if x == TeleportStatus::RequiresEncryption as u8 => {
                Ok(TeleportStatus::RequiresEncryption)
            }
            x if x == TeleportStatus::EncryptionError as u8 => Ok(TeleportStatus::EncryptionError),
            x if x == TeleportStatus::UnknownAction as u8 => Ok(TeleportStatus::UnknownAction),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                "Unknown TeleportStatus code - update Teleporter?",
            )),
        }
    }
}

impl TeleportInitAck {
    pub fn new(status: TeleportStatus) -> TeleportInitAck {
        let version = Version::parse(VERSION).unwrap();

        TeleportInitAck {
            status: status as u8,
            version: [
                version.major as u16,
                version.minor as u16,
                version.patch as u16,
            ],
            features: None,
            delta: None,
        }
    }

    pub fn serialize(self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add status
        let status = self.status as u8;
        out.append(&mut vec![status]);

        // Add version
        for i in self.version {
            out.append(&mut i.to_le_bytes().to_vec());
        }

        // If no features, return early
        if status != TeleportStatus::Proceed as u8 || self.features.is_none() {
            return out;
        }

        // Add optional features
        let feat = self.features.unwrap() as u32;
        out.append(&mut feat.to_le_bytes().to_vec());

        // If no delta, return early
        if feat & (TeleportFeatures::Delta as u32) != TeleportFeatures::Delta as u32
            || self.delta.is_none()
        {
            return out;
        }

        // Add optional TeleportDelta data
        out.append(&mut self.delta.unwrap().serialize());

        out
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let mut buf: &[u8] = input;

        // Extract status
        self.status = buf.read_u8().unwrap();

        // Extract version
        for i in &mut self.version {
            *i = buf.read_u16::<LittleEndian>().unwrap();
        }

        // If no features, return early
        if self.status != TeleportStatus::Proceed as u8 {
            return Ok(());
        }

        // Extract optional features
        let features = buf.read_u32::<LittleEndian>().unwrap();
        self.features = Some(features);

        // If no delta, return early
        if features & (TeleportFeatures::Delta as u32) != TeleportFeatures::Delta as u32 {
            return Ok(());
        }

        // Extract optional TeleportDelta data
        let mut delta = TeleportDelta::new();
        delta.deserialize(&input[11..])?;
        self.delta = Some(delta);

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportDelta {
    pub filesize: u64,
    pub hash: u64,
    pub chunk_size: u64,
    chunk_hash_len: u16,
    pub chunk_hash: Vec<u64>,
}

impl TeleportDelta {
    pub fn new() -> TeleportDelta {
        TeleportDelta {
            filesize: 0,
            hash: 0,
            chunk_size: 0,
            chunk_hash_len: 0,
            chunk_hash: Vec::<u64>::new(),
        }
    }

    fn delta_serial(input: &[u64]) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        for i in input {
            out.append(&mut i.to_le_bytes().to_vec());
        }

        out
    }

    pub fn serialize(self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add file size
        out.append(&mut self.filesize.to_le_bytes().to_vec());

        // Add file hash
        out.append(&mut self.hash.to_le_bytes().to_vec());

        // Add chunk size
        out.append(&mut self.chunk_size.to_le_bytes().to_vec());

        // Add delta vector length
        let dlen = self.chunk_hash.len() as u16;
        out.append(&mut dlen.to_le_bytes().to_vec());

        // Add delta vector
        out.append(&mut TeleportDelta::delta_serial(&self.chunk_hash));

        out
    }

    fn delta_deserial(input: &[u8], len: u16) -> Result<Vec<u64>, Error> {
        if input.len() % 8 != 0 || len as usize != input.len() / 8 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Cannot deserialize Vec<u64>",
            ));
        }

        let mut out = Vec::<u64>::new();
        let mut buf = input;
        let mut count: u16 = len;
        while count > 0 {
            let a: u64 = buf.read_u64::<LittleEndian>().unwrap();
            out.push(a);
            count -= 1;
        }

        Ok(out)
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let mut buf: &[u8] = input;

        if input.len() < 26 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Not enough data for Delta deserialize",
            ));
        }

        self.filesize = buf.read_u64::<LittleEndian>().unwrap();

        // Extract file hash
        self.hash = buf.read_u64::<LittleEndian>().unwrap();

        // Extract chunk size
        self.chunk_size = buf.read_u64::<LittleEndian>().unwrap();

        // Extract delta vector length
        self.chunk_hash_len = buf.read_u16::<LittleEndian>().unwrap();

        // Extract delta vector
        self.chunk_hash = TeleportDelta::delta_deserial(buf, self.chunk_hash_len).unwrap();

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct TeleportData {
    pub offset: u64,
    pub data_len: u32,
    pub data: Vec<u8>,
}

impl TeleportData {
    pub fn new() -> TeleportData {
        TeleportData {
            offset: 0,
            data_len: 0,
            data: Vec::<u8>::new(),
        }
    }

    pub fn serialize(&mut self) -> Vec<u8> {
        let mut out = Vec::<u8>::new();

        // Add offset
        out.append(&mut self.offset.to_le_bytes().to_vec());

        // Add data length
        let length = self.data.len() as u32;
        out.append(&mut length.to_le_bytes().to_vec());

        // Add data
        out.append(&mut self.data);

        out
    }

    pub fn deserialize(&mut self, input: &[u8]) -> Result<(), Error> {
        let mut buf: &[u8] = input;

        // Extract offset
        self.offset = buf.read_u64::<LittleEndian>().unwrap();

        // Extract data length
        self.data_len = buf.read_u32::<LittleEndian>().unwrap();

        // Extract data
        self.data = input[12..].to_vec();
        if self.data.len() != self.data_len as usize {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Filename incorrect length",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

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
        177, 104, 222, 58, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0, 21, 205, 91, 7, 0, 0, 0, 0, 0, 0,
    ];
    const TESTDATAPKT: &[u8] = &[49, 212, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 1, 2, 3, 4, 5];
    const TESTINITACK: &[u8] = &[0, 0, 0, 6, 0, 0, 0, 5, 0, 0, 0];

    #[test]
    fn test_teleportheader_serialize() {
        let mut t = TeleportHeader::new(TeleportAction::Init);
        t.data.append(&mut TESTDATA.to_vec());
        t.action |= TeleportAction::Encrypted as u8;
        t.iv = Some(*TESTHEADERIV);
        let s = t.serialize();
        assert_eq!(s, TESTHEADER);
    }

    #[test]
    fn test_teleportheader_deserialize() {
        let mut test = TeleportHeader::new(TeleportAction::Init);
        test.data.append(&mut TESTDATA.to_vec());
        test.action |= TeleportAction::Encrypted as u8;
        test.iv = Some(*TESTHEADERIV);
        test.data_len = 17;
        let mut t = TeleportHeader::new(TeleportAction::Init);
        t.deserialize(TESTHEADER.to_vec()).unwrap();
        assert_eq!(t, test);
    }

    #[test]
    fn test_teleportenc_key_exchange() {
        let mut a = TeleportEnc::new();
        let mut b = TeleportEnc::new();

        let priva = crypto::genkey(&mut a);
        let privb = crypto::genkey(&mut b);

        a.deserialize(&b.serialize()).unwrap();
        b.deserialize(&a.serialize()).unwrap();

        a.calc_secret(priva);
        b.calc_secret(privb);

        assert_eq!(a.secret, b.secret);
    }

    #[test]
    fn test_teleportenc_encrypt_decrypt() {
        let mut rng = StdRng::from_entropy();
        let mut nonce: [u8; 12] = [0; 12];

        let mut a = TeleportEnc::new();
        let mut b = TeleportEnc::new();

        let priva = crypto::genkey(&mut a);
        let privb = crypto::genkey(&mut b);

        a.deserialize(&b.serialize()).unwrap();
        b.deserialize(&a.serialize()).unwrap();

        a.calc_secret(priva);
        b.calc_secret(privb);

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
        test.version = [0, 5, 5];
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
        test.version = [0, 5, 5];
        test.filename = vec!['f', 'i', 'l', 'e'];
        test.filename_len = test.filename.len() as u16;
        test.filesize = 12345;
        test.chmod = 0o755;
        test.features |= TeleportFeatures::Overwrite as u32;

        let mut t = TeleportInit::new(TeleportFeatures::NewFile);
        t.deserialize(TESTINIT).unwrap();
        t.version = [0, 5, 5];

        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportdelta_serialize() {
        let mut test = TeleportDelta::new();
        test.filesize = 987654321;
        test.hash = 12345;
        test.chunk_size = 123456789;
        test.chunk_hash = Vec::<u64>::new();

        let out = test.serialize();

        assert_eq!(out, TESTDELTA);
    }

    #[test]
    fn test_teleportdelta_deserialize() {
        let mut test = TeleportDelta::new();
        test.filesize = 987654321;
        test.hash = 12345;
        test.chunk_size = 123456789;
        test.chunk_hash = Vec::<u64>::new();

        let mut t = TeleportDelta::new();
        t.deserialize(TESTDELTA).unwrap();

        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportdata_serialize() {
        let mut test = TeleportData::new();
        test.offset = 54321;
        test.data_len = 5;
        test.data = vec![1, 2, 3, 4, 5];

        let out = test.serialize();

        assert_eq!(out, TESTDATAPKT);
    }

    #[test]
    fn test_teleportdata_deserialize() {
        let mut test = TeleportData::new();
        test.offset = 54321;
        test.data_len = 5;
        test.data = vec![1, 2, 3, 4, 5];

        let mut t = TeleportData::new();
        t.deserialize(TESTDATAPKT).unwrap();

        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportinitack_serialize() {
        let mut test = TeleportInitAck::new(TeleportStatus::Proceed);
        let feat = TeleportFeatures::NewFile as u32 | TeleportFeatures::Overwrite as u32;
        test.features = Some(feat);
        test.version = [0, 6, 0];
        let out = test.serialize();

        assert_eq!(out, TESTINITACK);
    }

    #[test]
    fn test_teleportinitack_deserialize() {
        let mut test = TeleportInitAck::new(TeleportStatus::Proceed);
        let feat = TeleportFeatures::NewFile as u32 | TeleportFeatures::Overwrite as u32;
        test.features = Some(feat);
        test.version = [0, 6, 0];

        let mut t = TeleportInitAck::new(TeleportStatus::Proceed);
        t.deserialize(TESTINITACK).unwrap();

        assert_eq!(test, t);
    }
}

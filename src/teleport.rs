use crate::*;
use deku::prelude::*;
use x25519_dalek::{EphemeralSecret, PublicKey};

#[derive(Debug, PartialEq, DekuWrite, DekuRead, Eq)]
pub struct TeleportHeader {
    protocol: u64,
    #[deku(update = "self.data.len()")]
    pub data_len: u32,
    pub action: u8,
    #[deku(cond = "*action & TeleportAction::Encrypted as u8 == TeleportAction::Encrypted as u8")]
    pub iv: Option<[u8; 12]>,
    #[deku(count = "data_len")]
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, Default, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "little")]
pub struct TeleportInit {
    pub version: [u16; 3],
    pub features: u32,
    pub chmod: u32,
    pub filesize: u64,
    #[deku(update = "self.filename.len()")]
    pub filename_len: u16,
    #[deku(count = "filename_len")]
    pub filename: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
            filename: Vec::<u8>::new(),
        }
    }
}

#[derive(Clone, Debug, DekuRead, DekuWrite, PartialEq, Eq)]
pub struct TeleportInitAck {
    pub status: u8,
    pub version: [u16; 3],
    #[deku(cond = "*status == TeleportStatus::Proceed as u8")]
    pub features: Option<u32>,
    #[deku(
        cond = "(*features).map_or(false, |x| x  & TeleportFeatures::Delta as u32 == TeleportFeatures::Delta as u32)"
    )]
    pub delta: Option<TeleportDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeleportStatus {
    Proceed,
    NoOverwrite,
    NoSpace,
    NoPermission,
    WrongVersion,
    RequiresEncryption,
    EncryptionError,
    BadFileName,
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
}

#[derive(Clone, Debug, Default, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "little")]
pub struct TeleportDelta {
    pub filesize: u64,
    pub hash: u64,
    pub chunk_size: u32,
    #[deku(update = "self.chunk_hash.len()")]
    chunk_hash_len: u16,
    #[deku(count = "chunk_hash_len")]
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
}

#[derive(Debug, Default, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "little")]
pub struct TeleportData {
    pub offset: u64,
    #[deku(update = "self.data.len()")]
    pub data_len: u32,
    #[deku(count = "data_len")]
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
        177, 104, 222, 58, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0, 21, 205, 91, 7, 0, 0,
    ];
    const TESTDATAPKT: &[u8] = &[49, 212, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 1, 2, 3, 4, 5];
    const TESTINITACK: &[u8] = &[0, 0, 0, 6, 0, 0, 0, 5, 0, 0, 0];

    #[test]
    fn test_teleportheader_serialize() {
        let mut t = TeleportHeader::new(TeleportAction::Init);
        t.data.append(&mut TESTDATA.to_vec());
        t.action |= TeleportAction::Encrypted as u8;
        t.iv = Some(*TESTHEADERIV);
        let s = t.serialize().unwrap();
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
        test.filename = vec![b'f', b'i', b'l', b'e'];
        test.filename_len = test.filename.len() as u16;
        test.filesize = 12345;
        test.chmod = 0o755;
        test.features |= TeleportFeatures::Overwrite as u32;

        let out = test.to_bytes().unwrap();
        assert_eq!(out, TESTINIT);
    }

    #[test]
    fn test_teleportinit_deserialize() {
        let mut test = TeleportInit::new(TeleportFeatures::NewFile);
        test.version = [0, 5, 5];
        test.filename = vec![b'f', b'i', b'l', b'e'];
        test.filename_len = test.filename.len() as u16;
        test.filesize = 12345;
        test.chmod = 0o755;
        test.features |= TeleportFeatures::Overwrite as u32;

        let (_, mut t) = TeleportInit::from_bytes((&TESTINIT, 0)).unwrap();
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

        let out = test.to_bytes().unwrap();

        assert_eq!(out, TESTDELTA);
    }

    #[test]
    fn test_teleportdelta_deserialize() {
        let mut test = TeleportDelta::new();
        test.filesize = 987654321;
        test.hash = 12345;
        test.chunk_size = 123456789;
        test.chunk_hash = Vec::<u64>::new();

        let (_, t) = TeleportDelta::from_bytes((&TESTDELTA, 0)).unwrap();

        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportdata_serialize() {
        let mut test = TeleportData::new();
        test.offset = 54321;
        test.data_len = 5;
        test.data = vec![1, 2, 3, 4, 5];

        let out = test.to_bytes().unwrap();

        assert_eq!(out, TESTDATAPKT);
    }

    #[test]
    fn test_teleportdata_deserialize() {
        let mut test = TeleportData::new();
        test.offset = 54321;
        test.data_len = 5;
        test.data = vec![1, 2, 3, 4, 5];

        let (_, t) = TeleportData::from_bytes((&TESTDATAPKT, 0)).unwrap();

        assert_eq!(test, t);
    }

    #[test]
    fn test_teleportinitack_serialize() {
        let mut test = TeleportInitAck::new(TeleportStatus::Proceed);
        let feat = TeleportFeatures::NewFile as u32 | TeleportFeatures::Overwrite as u32;
        test.features = Some(feat);
        test.version = [0, 6, 0];
        let out = test.to_bytes().unwrap();

        assert_eq!(out, TESTINITACK);
    }

    #[test]
    fn test_teleportinitack_deserialize() {
        let mut test = TeleportInitAck::new(TeleportStatus::Proceed);
        let feat = TeleportFeatures::NewFile as u32 | TeleportFeatures::Overwrite as u32;
        test.features = Some(feat);
        test.version = [0, 6, 0];

        let (_, t) = TeleportInitAck::from_bytes((&TESTINITACK, 0)).unwrap();

        assert_eq!(test, t);
    }
}

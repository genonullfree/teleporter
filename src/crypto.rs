use crate::{Error, ErrorKind};
use aes_gcm::aead::{Aead, NewAead};
use aes_gcm::{Aes256Gcm, Key};
use crypto::ed25519::{exchange, keypair};
use generic_array::GenericArray;
use rand::prelude::*;

pub fn genkey() -> ([u8; 64], [u8; 32]) {
    let mut rng = StdRng::from_entropy();

    let mut seed: [u8; 32] = [0; 32];
    rng.fill(&mut seed);

    keypair(&seed)
}

pub fn calc_secret(public: &[u8; 32], private: &[u8; 64]) -> [u8; 32] {
    exchange(public, private)
}

pub fn decrypt(key: &[u8; 32], nonce: Vec<u8>, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    let key = Key::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);

    let plaintext = cipher
        .decrypt(gen_nonce, data.as_ref())
        .expect("Decrypt failed");

    Ok(plaintext.to_vec())
}

pub fn encrypt(key: &[u8; 32], nonce: Vec<u8>, input: Vec<u8>) -> Result<Vec<u8>, Error> {
    let key = Key::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);
    match cipher.encrypt(gen_nonce, input.as_ref()) {
        Ok(s) => Ok(s),
        Err(_) => Err(Error::new(ErrorKind::InvalidData, "Encryption failed")),
    }
}

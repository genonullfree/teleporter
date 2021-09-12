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

pub fn decrypt(key: &[u8; 32], text: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    let (nonce, data) = match extract_iv_data(text) {
        Ok((n, d)) => (n, d),
        Err(s) => return Err(s),
    };

    let key = Key::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);

    let plaintext = cipher
        .decrypt(gen_nonce, data.as_ref())
        .expect("Decrypt failed");

    Ok(plaintext.to_vec())
}

fn extract_iv_data(input: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    if input.len() < 13 {
        return Err("Input too short");
    }

    let nonce = &input[..12];
    let data = &input[12..];

    Ok((nonce.to_vec(), data.to_vec()))
}

pub fn encrypt(key: &[u8; 32], nonce: Vec<u8>, input: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    let key = Key::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);
    let ciphertext = cipher
        .encrypt(gen_nonce, input.as_ref())
        .expect("Encrypt failed");

    match insert_iv_data(nonce, ciphertext) {
        Ok(d) => return Ok(d),
        Err(s) => return Err(s),
    };
}

fn insert_iv_data(mut nonce: Vec<u8>, mut data: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    let mut buf = Vec::<u8>::new();
    if data.len() == 0 {
        return Err("No data to encrypt");
    }

    buf.append(&mut nonce);
    buf.append(&mut data);

    Ok(buf)
}

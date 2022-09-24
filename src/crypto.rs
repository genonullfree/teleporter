use crate::teleport::TeleportEnc;
use crate::{Error, ErrorKind};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use generic_array::GenericArray;
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey};

pub fn genkey(ctx: &mut TeleportEnc) -> EphemeralSecret {
    let secret = EphemeralSecret::new(OsRng);
    ctx.public = PublicKey::from(&secret).to_bytes();

    secret
}

pub fn decrypt(key: &[u8; 32], nonce: Vec<u8>, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    let key = GenericArray::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);

    let plaintext = cipher
        .decrypt(gen_nonce, data.as_ref())
        .expect("Decrypt failed");

    Ok(plaintext.to_vec())
}

pub fn encrypt(key: &[u8; 32], nonce: Vec<u8>, input: Vec<u8>) -> Result<Vec<u8>, Error> {
    let key = GenericArray::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let gen_nonce = GenericArray::from_slice(&nonce);
    match cipher.encrypt(gen_nonce, input.as_ref()) {
        Ok(s) => Ok(s),
        Err(_) => Err(Error::new(ErrorKind::InvalidData, "Encryption failed")),
    }
}

use crypto::ed25519::{exchange, keypair};
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

//
// Copyright 2018 rust-wallet developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//!
//! # Key derivation
//!
//! TREZOR compatible key derivation
//!

use bitcoin::network::constants::Network;
use bitcoin::util::bip32::{ExtendedPubKey, ExtendedPrivKey, ChildNumber};
use secp256k1::Secp256k1;
use rand::{rngs::OsRng, RngCore};

use super::error::WalletError;
use super::mnemonic::Mnemonic;

/// a fabric of keys
pub struct KeyFactory;

impl KeyFactory {
    /// create a new random master private key
    pub fn new_master_private_key(
        entropy: MasterKeyEntropy,
        network: Network,
        passphrase: &str,
        salt: &str,
        debug: bool,
    ) -> Result<(ExtendedPrivKey, Mnemonic, Vec<u8>), WalletError> {
        let mut encrypted = vec![0u8; entropy as usize];
        if let Ok(mut rng) = OsRng::new() {
            if !debug {
                rng.fill_bytes(encrypted.as_mut_slice());
            }
            let mnemonic = Mnemonic::new(&encrypted, passphrase)?;
            let seed = Seed::new(&mnemonic, salt);
            let key = KeyFactory::master_private_key(network, &seed)?;
            return Ok((key, mnemonic, encrypted));
        }
        Err(WalletError::CannotObtainRandomSource)
    }

    /// decrypt stored master key
    pub fn decrypt(
        encrypted: &[u8],
        network: Network,
        passphrase: &str,
        salt: &str,
    ) -> Result<(ExtendedPrivKey, Mnemonic), WalletError> {
        let mnemonic = Mnemonic::new(encrypted, passphrase)?;
        let seed = Seed::new(&mnemonic, salt);
        let key = KeyFactory::master_private_key(network, &seed)?;
        Ok((key, mnemonic))
    }

    pub fn recover_from_mnemonic(
        mnemonic: &Mnemonic,
        network: Network,
        salt: &str,
    ) -> Result<ExtendedPrivKey, WalletError> {
        let seed = Seed::new(&mnemonic, salt);
        KeyFactory::master_private_key(network, &seed)
    }

    /// create a master private key from seed
    pub fn master_private_key(
        network: Network,
        seed: &Seed,
    ) -> Result<ExtendedPrivKey, WalletError> {
        ExtendedPrivKey::new_master(network, &seed.0)
            .map_err(WalletError::KeyDerivation)
    }

    /// get extended public key for a known private key
    pub fn extended_public_from_private(extended_private_key: &ExtendedPrivKey) -> ExtendedPubKey {
        ExtendedPubKey::from_private(&Secp256k1::new(), extended_private_key)
    }

    pub fn private_child(
        extended_private_key: &ExtendedPrivKey,
        child: ChildNumber,
    ) -> Result<ExtendedPrivKey, WalletError> {
        extended_private_key.ckd_priv(&Secp256k1::new(), child)
            .map_err(WalletError::KeyDerivation)
    }

    pub fn public_child(
        &self,
        extended_public_key: &ExtendedPubKey,
        child: ChildNumber,
    ) -> Result<ExtendedPubKey, WalletError> {
        extended_public_key.ckd_pub(&Secp256k1::new(), child)
            .map_err(WalletError::KeyDerivation)
    }
}

#[derive(Copy, Clone)]
pub enum MasterKeyEntropy {
    Low = 16,
    Recommended = 32,
    Paranoid = 64,
}

pub struct Seed(Vec<u8>);

#[cfg(test)]
impl Seed {
    // return a copy of the seed data
    pub fn data(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl Seed {
    /// create a seed from mnemonic (optionally with salt)
    pub fn new(mnemonic: &Mnemonic, salt: &str) -> Seed {
        use crypto::pbkdf2;
        use crypto::hmac::Hmac;
        use crypto::sha2::Sha512;

        let mut mac = Hmac::new(Sha512::new(), mnemonic.to_string().as_bytes());
        let mut output = [0u8; 64];
        let msalt = "mnemonic".to_owned() + salt;
        pbkdf2::pbkdf2(&mut mac, msalt.as_bytes(), 2048, &mut output);
        Seed(output.to_vec())
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::path::PathBuf;
    use std::io::Read;
    use bitcoin::network::constants::Network;
    use bitcoin::util::bip32::ChildNumber;
    use crate::keyfactory::Seed;
    use rustc_serialize::json::Json;

    #[test]
    fn bip32_tests() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("tests/BIP32.json");
        let mut file = File::open(d).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();
        let json = Json::from_str(&data).unwrap();
        let tests = json.as_array().unwrap();
        for test in tests {
            let seed = Seed(hex::decode(test["seed"].as_string().unwrap()).unwrap());
            let master_private =
                super::KeyFactory::master_private_key(Network::Bitcoin, &seed).unwrap();
            assert_eq!(
                test["private"].as_string().unwrap(),
                master_private.to_string()
            );
            assert_eq!(
                test["public"].as_string().unwrap(),
                super::KeyFactory::extended_public_from_private(&master_private).to_string()
            );
            for d in test["derived"].as_array().unwrap() {
                let mut key = master_private.clone();
                for l in d["locator"].as_array().unwrap() {
                    let sequence = l["sequence"].as_u64().unwrap();
                    let private = l["private"].as_boolean().unwrap();
                    let child = if private {
                        ChildNumber::Hardened {
                            index: sequence as u32,
                        }
                    } else {
                        ChildNumber::Normal {
                            index: sequence as u32,
                        }
                    };
                    key = super::KeyFactory::private_child(&key.clone(), child).unwrap();
                }
                assert_eq!(d["private"].as_string().unwrap(), key.to_string());
                assert_eq!(
                    d["public"].as_string().unwrap(),
                    super::KeyFactory::extended_public_from_private(&key).to_string()
                );
            }
        }
    }
}

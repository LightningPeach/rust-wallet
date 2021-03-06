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
//! # Accounts
//!
//! TREZOR compatible accounts (BIP44)
//!
use bitcoin::{
    util::{
        bip32::{ExtendedPubKey, ExtendedPrivKey, ChildNumber, Error as Bip32Error},
        address::Address,
    },
    blockdata::{
        script::Script,
        transaction::OutPoint,
    },
    network::constants::Network,
    PrivateKey,
    PublicKey
};
use secp256k1::Secp256k1;
use serde::{Serialize, Deserialize};

use super::DB;

use std::{
    sync::{Arc, RwLock},
    collections::HashMap,
};

/// Address type an account is using
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub enum AccountAddressType {
    /// pay to public key hash (aka. legacy)
    P2PKH,
    /// pay to script hash of a witness script (aka. segwit in legacy address format)
    P2SHWH,
    /// pay to witness public key hash
    P2WKH,
}

impl<'a> From<&'a str> for AccountAddressType {
    fn from(addr_type: &'a str) -> AccountAddressType {
        // let addr_type_str: &str = &addr_type;
        match addr_type {
            "p2pkh" => AccountAddressType::P2PKH,
            "p2shwh" => AccountAddressType::P2SHWH,
            "p2wkh" => AccountAddressType::P2WKH,
            _ => panic!("unknown address type: {}", addr_type),
        }
    }
}

impl From<AccountAddressType> for usize {
    fn from(val: AccountAddressType) -> usize {
        match val {
            AccountAddressType::P2PKH => 0,
            AccountAddressType::P2SHWH => 1,
            AccountAddressType::P2WKH => 2,
        }
    }
}

impl Into<AccountAddressType> for usize {
    fn into(self) -> AccountAddressType {
        match self {
            0 => AccountAddressType::P2PKH,
            1 => AccountAddressType::P2SHWH,
            2 => AccountAddressType::P2WKH,
            _ => panic!("unknown address code: {}", self),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AddressChain {
    External,
    Internal,
}

impl Into<u32> for AddressChain {
    fn into(self) -> u32 {
        match self {
            AddressChain::External => 0,
            AddressChain::Internal => 1,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeyPath {
    addr_chain: AddressChain,
    addr_index: u32,
}

impl KeyPath {
    pub fn new(addr_chain: AddressChain, addr_index: u32) -> Self {
        Self {
            addr_chain,
            addr_index,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Utxo {
    pub value: u64,
    pub key_path: KeyPath,
    pub out_point: OutPoint,
    pub account_index: u32,
    pub pk_script: Script,
    pub addr_type: AccountAddressType,
}

impl Utxo {
    pub fn new(
        value: u64,
        key_path: KeyPath,
        out_point: OutPoint,
        account_index: u32,
        pk_script: Script,
        addr_type: AccountAddressType,
    ) -> Self {
        Utxo {
            value,
            key_path,
            out_point,
            account_index,
            pk_script,
            addr_type,
        }
    }
}

/// a TREZOR compatible account
pub struct Account {
    account_key: ExtendedPrivKey,
    pub address_type: AccountAddressType,
    network: Network,

    external_index: u32,
    internal_index: u32,
    pub external_pk_list: Vec<PublicKey>,
    pub internal_pk_list: Vec<PublicKey>,

    pub btc_address_list: Vec<String>,

    pub utxo_list: HashMap<OutPoint, Utxo>,
    db: Arc<RwLock<DB>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SecretKeyHelper {
    pub addr_type: AccountAddressType,
    addr_chain: AddressChain,
    index: u32,
}

impl SecretKeyHelper {
    fn new(addr_type: AccountAddressType, addr_chain: AddressChain, index: u32) -> Self {
        SecretKeyHelper {
            addr_type,
            addr_chain,
            index,
        }
    }
}

impl Account {
    pub fn new(
        account_key: ExtendedPrivKey,
        address_type: AccountAddressType,
        network: Network,
        db: Arc<RwLock<DB>>,
    ) -> Account {
        Account {
            account_key,
            address_type,
            network,

            external_index: 0,
            internal_index: 0,
            external_pk_list: Vec::new(),
            internal_pk_list: Vec::new(),

            btc_address_list: Vec::new(),

            utxo_list: HashMap::new(),
            db,
        }
    }

    pub fn get_sk(&self, key_path: &KeyPath) -> PrivateKey {
        let path = &[
            ChildNumber::Normal {
                index: key_path.addr_chain.clone().into(),
            }, // TODO(evg): use addr chain enum instead?
            ChildNumber::Normal {
                index: key_path.addr_index,
            },
        ];
        let extended_priv_key = self
            .account_key
            .derive_priv(&Secp256k1::new(), path)
            .unwrap();
        extended_priv_key.private_key
    }

    pub fn grab_utxo(&mut self, utxo: Utxo) {
        self.utxo_list.insert(utxo.out_point, utxo.clone());
        self.db.write().unwrap().put_utxo(&utxo.out_point, &utxo);
    }

    pub fn get_utxo_list(&self) -> &HashMap<OutPoint, Utxo> {
        &self.utxo_list
    }

    pub fn next_external_pk(&mut self) -> Result<PublicKey, Bip32Error> {
        let path = &[
            ChildNumber::Normal { index: 0 }, // TODO(evg): use addr chain enum instead?
            ChildNumber::Normal {
                index: self.external_index,
            },
        ];
        let extended_priv_key = self.account_key.derive_priv(&Secp256k1::new(), path)?;

        let extended_pub_key = ExtendedPubKey::from_private(&Secp256k1::new(), &extended_priv_key);
        self.external_pk_list.push(extended_pub_key.public_key);

        // DB BEGIN
        let key = SecretKeyHelper::new(
            self.address_type.clone(),
            AddressChain::External,
            self.external_index,
        );
        self.db
            .write()
            .unwrap()
            .put_external_public_key(&key, &extended_pub_key.public_key);
        // DB END

        self.external_index += 1;
        Ok(extended_pub_key.public_key)
    }

    pub fn next_internal_pk(&mut self) -> Result<PublicKey, Bip32Error> {
        let path = &[
            ChildNumber::Normal { index: 1 },
            ChildNumber::Normal {
                index: self.internal_index,
            },
        ];
        self.internal_index += 1;
        let extended_priv_key = self.account_key.derive_priv(&Secp256k1::new(), path)?;

        let extended_pub_key = ExtendedPubKey::from_private(&Secp256k1::new(), &extended_priv_key);
        self.internal_pk_list.push(extended_pub_key.public_key);

        // DB BEGIN
        let key = SecretKeyHelper::new(
            self.address_type.clone(),
            AddressChain::Internal,
            self.internal_index,
        );
        self.db
            .write()
            .unwrap()
            .put_internal_public_key(&key, &extended_pub_key.public_key);
        // DB END

        Ok(extended_pub_key.public_key)
    }

    pub fn addr_from_pk(&self, pk: &PublicKey) -> String {
        fn p2pkh_addr_from_public_key(pk: &PublicKey, network: Network) -> String {
            let addr = Address::p2pkh(pk, network);
            addr.to_string()
        }

        fn p2shwh_addr_from_public_key(pk: &PublicKey, network: Network) -> String {
            let addr = Address::p2shwpkh(pk, network);
            addr.to_string()
        }

        fn p2wkh_addr_from_public_key_bip0173(pk: &PublicKey, network: Network) -> String {
            let addr = Address::p2wpkh(pk, network);
            addr.to_string()
        }

        match self.address_type {
            AccountAddressType::P2PKH => p2pkh_addr_from_public_key(pk, self.network),
            AccountAddressType::P2SHWH => p2shwh_addr_from_public_key(pk, self.network),
            AccountAddressType::P2WKH => p2wkh_addr_from_public_key_bip0173(pk, self.network),
        }
    }

    pub fn script_from_pk(&self, pk: &PublicKey) -> Script {
        fn p2pkh_script_from_public_key(pk: &PublicKey, network: Network) -> Script {
            let p2pkh_addr = Address::p2pkh(pk, network);
            p2pkh_addr.script_pubkey()
        }

        fn p2shwh_script_from_public_key(pk: &PublicKey, network: Network) -> Script {
            let addr = Address::p2shwpkh(pk, network);
            addr.script_pubkey()
        }

        fn p2wkh_script_from_public_key(pk: &PublicKey, network: Network) -> Script {
            let p2wkh_addr = Address::p2wpkh(pk, network);
            p2wkh_addr.script_pubkey()
        }

        match self.address_type {
            AccountAddressType::P2PKH => p2pkh_script_from_public_key(pk, self.network),
            AccountAddressType::P2SHWH => p2shwh_script_from_public_key(pk, self.network),
            AccountAddressType::P2WKH => p2wkh_script_from_public_key(pk, self.network),
        }
    }

    pub fn new_address(&mut self) -> Result<String, Bip32Error> {
        let pk = self.next_external_pk()?;
        let addr = self.addr_from_pk(&pk);
        self.btc_address_list.push(addr.clone());
        self.db
            .write()
            .unwrap()
            .put_address(self.address_type.clone(), addr.clone());
        Ok(addr)
    }

    pub fn new_change_address(&mut self) -> Result<String, Bip32Error> {
        let pk = self.next_internal_pk()?;
        let addr = self.addr_from_pk(&pk);
        self.btc_address_list.push(addr.clone());
        self.db
            .write()
            .unwrap()
            .put_address(self.address_type.clone(), addr.clone());
        Ok(addr)
    }
}

#[cfg(test)]
mod test {
    use bitcoin::{
        network::constants::Network,
        Block, Transaction,
    };
    use bitcoin_hashes::sha256d::Hash as Sha256dHash;
    use std::{fmt, error::Error};

    use crate::walletlibrary::{WalletConfigBuilder, WalletLibraryMode, KeyGenConfig};
    use crate::default::WalletWithTrustedFullNode;
    use crate::account::AccountAddressType;
    use crate::interface::BlockChainIO;

    struct FakeBlockChainIO;

    #[derive(Debug)]
    struct FakeBlockChainIoError;

    impl fmt::Display for FakeBlockChainIoError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "")
        }
    }

    impl Error for FakeBlockChainIoError {}

    impl BlockChainIO for FakeBlockChainIO {
        type Error = FakeBlockChainIoError;

        fn get_block_count(&self) -> Result<u32, Self::Error> {
            Err(FakeBlockChainIoError)
        }

        fn get_block_hash(&self, height: u32) -> Result<Sha256dHash, Self::Error> {
            let _ = height;
            Err(FakeBlockChainIoError)
        }

        fn get_block(&self, header_hash: &Sha256dHash) -> Result<Block, Self::Error> {
            let _ = header_hash;
            Err(FakeBlockChainIoError)
        }

        fn send_raw_transaction(&self, tx: &Transaction) -> Result<Sha256dHash, Self::Error> {
            let _ = tx;
            Err(FakeBlockChainIoError)
        }
    }

    #[test]
    fn test_p2pkh_public_key_generation() {
        fn get_external_pk_vec() -> Vec<String> {
            vec![
                "02ea034fa1bd663e56b014902d59a50f2cdca9991edb7584ae49b0f3b0904905fd".to_string(),
                "03d0faa428f4f8202318acfeb8269074df2bb24e8aefbe5745d4b8a4c2e800d5ed".to_string(),
                "02bedbe580f9b48fd3a63d383bd7378619343e7965f3ce250ee6de1b59a79336ac".to_string(),
                "0218fb5c9ff790be111cb8c980f7ec89be8786e91ce4145751d70dc775ee2de332".to_string(),
                "026160eef736bece582e20c72da48adbb6424377e97c7203913675cfed1820d2dd".to_string(),
            ]
        }
        fn get_internal_pk_vec() -> Vec<String> {
            vec![
                "03bc63404d619ed19cdc0d343ebcaaa93550656f8e3aaa49cef29a38f0e6f3a9de".to_string(),
                "033b47c69e29c4b99591ee5eeee2c37e28901a1c69369f637e0c71609dbe8ade53".to_string(),
                "028ec40eb8314a6a977ca816ed0d6807e4c3ddf2f14f42df435d7247b100091df0".to_string(),
                "022a329996e9d365f3f5cfc35966dd7d52afe26526cfe4673e79f099b060a32f68".to_string(),
                "037cc18470eed6675c57fc63e9da6bf94174e464676fdc6eae77ec778be959a758".to_string(),
            ]
        }

        let wc = WalletConfigBuilder::new()
            .db_path("/tmp/test_p2pkh_public_key_generation".to_string())
            .network(Network::Testnet)
            .finalize();
        let (mut af, _) = WalletWithTrustedFullNode::new(
            wc,
            FakeBlockChainIO,
            WalletLibraryMode::Create(KeyGenConfig::debug()),
        )
        .unwrap();
        let account = af.wallet_lib.get_account_mut(AccountAddressType::P2PKH);

        for expected_pk in get_external_pk_vec() {
            let pk = account.next_external_pk().unwrap();
            assert_eq!(hex::encode(&pk.key.serialize()[..]), expected_pk);
        }

        for expected_pk in get_internal_pk_vec() {
            let pk = account.next_internal_pk().unwrap();
            assert_eq!(hex::encode(&pk.key.serialize()[..]), expected_pk);
        }
    }

    #[test]
    fn test_p2wkh_public_key_generation() {
        let external_pk_vec = vec![
            "02a863d8d8852c4e2dccf510f306c59f8d626ce21d12320d186420226a5243d0b5",
            "0393b0aea13a6c1290a27de788a77e5a3b5d2abdd920689016415d53e97f5616b8",
            "028fd85e6951a9d4c0c96f017cd193e377f83d87d5f8625859e14c2da2a80913d8",
            "0254200ef7081b7a9225d72ea3ac3e31c0b65f807e412779ede30f4eb2aecf7eaf",
            "023d2c707f539d6d1a96b3b5249225e0ff6205a3ba7c5c280e4fd9441e0391a8a4",
        ];

        let internal_pk_vec = vec![
            "022aaac31d8fbb1161c96c5b866d0af6f20652df95342a89ddb9d554d35599480c",
            "039a7fb40f59111eb4b9d9e33572195d2dd1c9d32799a4c92f1b0d04c48a4baf3f",
            "0288239d7f610e643924cec725bac86bc0d3839d27096e6140e6766b59f6bbc9e2",
            "0271bc0074892a9b44de9bcf4aaae13c4a9354f9eca8d925c4fc322f7bbd19da69",
            "02a954c4e5275a094182284d96c9044dcb4d9d208cb23d4e181f05459c26e32778",
        ];

        let wc = WalletConfigBuilder::new()
            .db_path("/tmp/test_p2wkh_public_key_generation".to_string())
            .network(Network::Testnet)
            .finalize();
        let (mut af, _) = WalletWithTrustedFullNode::new(
            wc,
            FakeBlockChainIO,
            WalletLibraryMode::Create(KeyGenConfig::debug()),
        )
        .unwrap();
        let account = af.wallet_lib.get_account_mut(AccountAddressType::P2WKH);

        for expected_pk in external_pk_vec {
            let pk = account.next_external_pk().unwrap();
            assert_eq!(hex::encode(&pk.key.serialize()[..]), expected_pk);
        }

        for expected_pk in internal_pk_vec {
            let pk = account.next_internal_pk().unwrap();
            assert_eq!(hex::encode(&pk.key.serialize()[..]), expected_pk);
        }
    }
}

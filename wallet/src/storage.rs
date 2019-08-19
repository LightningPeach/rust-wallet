use super::account::{Utxo, SecretKeyHelper, AccountAddressType};
use super::walletlibrary::{LockId, LockGroup};

use serde::{Serialize, Deserialize};
use bitcoin::{OutPoint, util::key::PublicKey};

use std::collections::HashMap;

pub struct DB {
    path: String,
    state: State,
}

impl DB {
    pub fn new(db_path: String) -> Self {
        DB {
            path: db_path,
            state: State::default(),
        }
    }

    fn store(&self) {
        let _ = self.path;
        unimplemented!()
    }

    pub fn get_bip39_randomness(&self) -> Option<Vec<u8>> {
        self.state.bip39_randomness.clone()
    }

    pub fn put_bip39_randomness(&mut self, randomness: &[u8]) {
        self.state.bip39_randomness = Some(randomness.to_vec());
        self.store();
    }

    pub fn get_last_seen_block_height(&self) -> usize {
        self.state.last_seen_block_height as _
    }

    pub fn put_last_seen_block_height(&mut self, last_seen_block_height: u32) {
        self.state.last_seen_block_height = last_seen_block_height;
        self.store();
    }

    pub fn get_utxo_map(&self) -> HashMap<OutPoint, Utxo> {
        self.state.utxo_map.clone()
    }

    pub fn put_utxo(&mut self, op: &OutPoint, utxo: &Utxo) {
        self.state.utxo_map.insert(op.clone(), utxo.clone());
        self.store();
    }

    pub fn delete_utxo(&mut self, op: &OutPoint) {
        self.state.utxo_map.remove(op);
        self.store();
    }

    pub fn get_external_public_key_list(&self) -> Vec<(SecretKeyHelper, PublicKey)> {
        self.state.external_public_key_list.clone()
    }

    pub fn get_internal_public_key_list(&self) -> Vec<(SecretKeyHelper, PublicKey)> {
        self.state.internal_public_key_list.clone()
    }

    pub fn get_full_address_list(&self) -> Vec<String> {
        [
            self.state.p2pkh_address_list.clone(),
            self.state.p2shwh_address_list.clone(),
            self.state.p2wkh_address_list.clone(),
        ].concat()
    }

    pub fn get_account_address_list(&self, addr_type: AccountAddressType) -> Vec<String> {
        match addr_type {
            AccountAddressType::P2PKH => self.state.p2pkh_address_list.clone(),
            AccountAddressType::P2SHWH => self.state.p2shwh_address_list.clone(),
            AccountAddressType::P2WKH => self.state.p2wkh_address_list.clone(),
        }
    }

    pub fn put_external_public_key(&mut self, key_helper: &SecretKeyHelper, pk: &PublicKey) {
        self.state.external_public_key_list.push((key_helper.clone(), pk.clone()));
        self.store();
    }

    pub fn put_internal_public_key(&mut self, key_helper: &SecretKeyHelper, pk: &PublicKey) {
        self.state.internal_public_key_list.push((key_helper.clone(), pk.clone()));
        self.store();
    }

    pub fn put_address(&mut self, addr_type: AccountAddressType, address: String) {
        match addr_type {
            AccountAddressType::P2PKH => self.state.p2pkh_address_list.push(address),
            AccountAddressType::P2SHWH => self.state.p2shwh_address_list.push(address),
            AccountAddressType::P2WKH => self.state.p2wkh_address_list.push(address),
        }
        self.store();
    }

    pub fn put_lock_group(&mut self, lock_id: &LockId, lock_group: &LockGroup) {
        self.state.lock_group.insert(lock_id.clone(), lock_group.clone());
        self.store();
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    bip39_randomness: Option<Vec<u8>>,
    last_seen_block_height: u32,
    utxo_map: HashMap<OutPoint, Utxo>,
    external_public_key_list: Vec<(SecretKeyHelper, PublicKey)>,
    internal_public_key_list: Vec<(SecretKeyHelper, PublicKey)>,
    p2pkh_address_list: Vec<String>,
    p2shwh_address_list: Vec<String>,
    p2wkh_address_list: Vec<String>,
    lock_group: HashMap<LockId, LockGroup>
}

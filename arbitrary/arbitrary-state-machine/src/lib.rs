use std::collections::HashMap;

use bigint::H256;
use vm::Bytes32;

#[allow(dead_code)]
mod vm;

// enum Tx {
//     Deploy {
//         code: Vec<u8>,
//     },
//     CallAndTransfer {

//     },
// }

pub struct Api {
    address: H256,
    database: HashMap<H256, Vec<u8>>,
    root: H256,
}

impl Api {
    pub fn new(address: H256) -> Self {
        Self {
            address,
            root: trie::EMPTY_TRIE_HASH,
            database: Default::default(),
        }
    }
    fn hashed_key(address: &H256, key: &Bytes32) -> Vec<u8> {
        let mut h_key = Vec::with_capacity(address.len() + key.len());
        h_key.extend_from_slice(address);
        h_key.extend_from_slice(key);
        h_key
    }
}

impl vm::Ext for Api {
    fn get(&self, key: &Bytes32) -> Bytes32 {
        let db = &self.database;
        let h_key = Self::hashed_key(&self.address, &key);
        let value = trie::get(self.root, &db, &h_key)
            .expect("Db get Error")
            .expect("Db get None");
        let mut ret = [0; 32];
        ret.copy_from_slice(value);
        ret
    }

    fn set(&mut self, key: &Bytes32, value: &Bytes32) {
        let h_key = Self::hashed_key(&self.address, &key);
        let db = &self.database;
        let (root, _change) = trie::insert(self.root, &db, &h_key, value).expect("Db Set Error");
        self.root = root;
    }
}

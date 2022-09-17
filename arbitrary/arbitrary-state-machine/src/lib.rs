use std::collections::HashMap;

use bigint::H256;
use sha3::{Digest, Sha3_256};
use trie::Change;
use vm::{Bytes32, Ext};
#[allow(dead_code)]
mod vm;

pub enum Tx {
    Deploy { code: Vec<u8>, calldata: Vec<u8> },
    CallAndTransfer { address: H256, calldata: Vec<u8> },
}

pub struct Block {
    txns: Vec<Tx>,
    state_root: H256,
}

// Wasm Contracts: Hash(b"ContractCode" + blob)
// Account states: A
#[derive(Default)]
pub struct GlobalState {
    state: HashMap<H256, Vec<u8>>,
}

impl Block {
    pub fn new(txns: Vec<Tx>, state_root: H256) -> Self {
        Self { txns, state_root }
    }
}
pub fn execute(block: Block, mut pre_state: GlobalState) -> GlobalState {
    let mut post_state: GlobalState = pre_state;
    for tx in block.txns.iter() {
        match tx {
            Tx::Deploy { code, calldata } => {
                // generate new address Create2
                // take Hash(code) and Hash(payload)

                // create a SHA3-256 object
                let mut hasher = Sha3_256::default();
                // write input message
                hasher.input(code);
                hasher.input(calldata);

                let result = hasher.result();
                let address = result.as_slice();
                let mut contract_address_arr = [0; 32];
                contract_address_arr.copy_from_slice(address);

                let contract_address = bigint::H256(contract_address_arr);
                let hashed_contract_address =
                    keyhash_with_prefix(b"ContractCode", &contract_address);

                // Creates new Contract and saves to trie and doesnt check for duplicate
                let contract_state = Box::new(ContractState::new(contract_address));
                let (_root, change) = trie::insert(
                    block.state_root,
                    &&post_state.state,
                    &hashed_contract_address,
                    code,
                )
                .expect("Failed to insert wasm code into state");
                apply_changes(&mut post_state.state, change);

                // Execute contract
                vm::execute(contract_state, &code).expect("Deploy's call to execute failed");
            }
            Tx::CallAndTransfer { address, calldata } => {
                let keyed_addr = keyhash_with_prefix(b"ContractCode", &address);
                // load wasm contract from global state
                let wasm = trie::get(block.state_root, &&post_state.state, &keyed_addr)
                    .expect("Trying to load WASM contract that doesn't exist");
                // instante new Ext
                let ext = Box::new(ContractState::new(*address));
                vm::execute(ext, calldata);
            }
        }
    }
    post_state
}

fn keyhash_with_prefix(prefix: &[u8], key: &[u8]) -> Vec<u8> {
    let mut h_key = Vec::with_capacity(prefix.len() + key.len());
    h_key.extend_from_slice(prefix);
    h_key.extend_from_slice(key);
    h_key
}
fn apply_changes(state: &mut HashMap<H256, Vec<u8>>, changes: Change) {
    state.extend(changes.adds);
    for del in changes.removes.iter() {
        // Do we need to care if were deleting something that doesnt exist? I mean it should exist.
        state
            .remove(del)
            .expect("Shouldn't delete something that doesn't exist!");
    }
}
pub struct ContractState {
    address: H256,
    database: HashMap<H256, Vec<u8>>,
    root: H256,
}

impl ContractState {
    pub fn new(address: H256) -> Self {
        Self {
            address,
            root: trie::EMPTY_TRIE_HASH,
            database: Default::default(),
        }
    }
}

impl vm::Ext for ContractState {
    fn get(&self, key: &Bytes32) -> Bytes32 {
        let db = &self.database;
        let h_key = keyhash_with_prefix(&self.address, key);
        let value = trie::get(self.root, &db, &h_key)
            .expect("Db get Error")
            .expect("Db get None");
        let mut ret = [0; 32];
        ret.copy_from_slice(value);
        ret
    }

    fn set(&mut self, key: &Bytes32, value: &Bytes32) {
        let h_key = keyhash_with_prefix(&self.address, key);
        let db = &self.database;
        let (root, change) = trie::insert(self.root, &db, &h_key, value).expect("Db Set Error");
        apply_changes(&mut self.database, change);
        self.root = root;
    }
}

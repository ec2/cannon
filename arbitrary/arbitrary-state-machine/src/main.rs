#![allow(dead_code)]
use std::{collections::HashMap, hash::Hash, io::SeekFrom};

use bigint::{H256, U256};
use sha3::{Digest, Sha3_256};
use trie::{Change, EMPTY_TRIE_HASH};
use vm::Bytes32;
// use rlp::{self, Encodable, Decodable};
use rlp::{self, Decodable};

mod iommu;
mod vm;
#[derive(Clone)]
pub struct Tx {
    source: H256,
    dest: H256,
    amount: U256,
}

pub struct Block {
    parent_hash: H256,
    state_root: H256,
    txns: Vec<Tx>,
}

#[derive(Clone)]
pub struct StateTrie {
    state: HashMap<H256, Vec<u8>>,
}

// impl Encodable for Tx {
//     fn rlp_append(&self, s: &mut rlp::RlpStream) {
//         let mut bytes:[u8;32] = [0;32];
//         self.amount.to_big_endian(&mut bytes);
//         s.append(&self.source.as_ref());
//         s.append(&self.dest.as_ref());
//         s.append(bytes);
//     }
// }
// impl Encodable for Block {
//     fn rlp_append(&self, s: &mut rlp::RlpStream) {
//         s.append(&self.parent_hash.as_ref());
//         s.append(&self.state_root.as_ref());
//         s.append_list(&self.txns);
//     }
// }
impl Decodable for Tx {
    fn decode(rlp: &rlp::UntrustedRlp) -> Result<Self, rlp::DecoderError> {
        let source = rlp.as_val()?;
        let dest = rlp.as_val()?;
        let amount = rlp.as_val()?;
        Ok(Self {
            source,
            dest,
            amount,
        })
    }
}
impl Decodable for Block {
    fn decode(rlp: &rlp::UntrustedRlp) -> Result<Self, rlp::DecoderError> {
        let parent_hash: H256 = rlp.as_val()?;
        let state_root: H256 = rlp.as_val()?;
        let txns: Vec<Tx> = rlp.as_list()?;
        Ok(Self {
            parent_hash,
            state_root,
            txns,
        })
    }
}

// Applies Changes to State Trie to add and remove nodes
pub fn apply_changes(map: &mut StateTrie, changes: Change) {
    map.state.extend(changes.adds);
    for del in changes.removes {
        map.state
            .remove(&del)
            .expect("failed to apply remove changes from trie");
    }
}

// Creates a fresh account with balance of 0 and returns new Trie root
pub fn create_account(root: H256, trie_db: &mut StateTrie, account: H256, bal: U256) -> H256 {
    let (root, changes) = trie::insert(root, &&trie_db.state, &account, &rlp::encode(&bal))
        .expect("failed to insert to trie");
    apply_changes(trie_db, changes);
    root
}

// Takes the previous state trie, root, and list of txs.
// The state_root of the block should be the empty hash right now because we
// calculate the new state_root in this routine.
pub fn execute_txs(trie_db: &StateTrie, prev_state_root: H256, txs: Vec<Tx>) -> (H256, StateTrie) {
    let mut curr_root = prev_state_root.clone();
    let mut post_trie = trie_db.clone();

    for tx in txs {
        let sender = tx.source;
        let receiver = tx.dest;
        let amount = tx.amount;

        // Read RLP encoded balance of Sender from state trie
        let mut sender_bal: U256 = rlp::decode(
            trie::get(prev_state_root, &&post_trie.state, &sender)
                .expect("Failed to retrieve sender bal from trie")
                .expect("Sender balance doesnt exist"),
        );

        if amount > sender_bal {
            panic!("Sender balance too low");
        }
        // Subtract amount from sender
        sender_bal = sender_bal - amount;

        // Read RLP encoded balance of Receiver from State Trie and add amount to send
        let receiver_bal: U256 = match trie::get(prev_state_root, &&post_trie.state, &sender) {
            Ok(Some(bal)) => rlp::decode::<U256>(bal) + amount,
            // Unintialized Account, populate state trie
            Ok(None) => {
                let root = create_account(curr_root, &mut post_trie, receiver, amount);
                curr_root = root;
                amount
            }
            Err(e) => {
                panic!("Failed to get receiver: {} bal: {:?}", receiver, e)
            }
        };

        // Update Accounts with new balances
        let (root, changes_a) = trie::insert(
            curr_root,
            &&post_trie.state,
            &receiver,
            &rlp::encode(&receiver_bal),
        )
        .expect("Failed to update receiver balance");
        apply_changes(&mut post_trie, changes_a);

        let (root, changes_b) =
            trie::insert(root, &&post_trie.state, &sender, &rlp::encode(&sender_bal))
                .expect("Failed to update sender bal");
        apply_changes(&mut post_trie, changes_b);

        curr_root = root;
    }

    (curr_root, post_trie)
}

// The state_root of the block should be the empty hash right now because we
// calculate the new state_root in this routine.
fn execute_block(state: &StateTrie, block: Block) -> (Block, StateTrie) {
    let prev_block = iommu::preimage(block.parent_hash)
        .expect("Could not find previous block in preimage oracle");
    let prev_block: Block = rlp::decode::<Block>(&prev_block);
    let prev_state_root = prev_block.state_root;

    let (new_root, new_trie) = execute_txs(state, prev_state_root, block.txns.clone());

    (
        Block {
            parent_hash: block.parent_hash,
            state_root: new_root,
            txns: block.txns,
        },
        new_trie,
    )
}
pub fn main() {
    // println!("Fuck");
    // let wasm =
    //     include_bytes!("../../contracts/target/wasm32-unknown-unknown/release/flipper.wasm");
    // let tx1 = Tx::Deploy { code: wasm.to_vec(), calldata: [1u8;32].to_vec()};
    // let b1 = Block::new(vec![tx1], EMPTY_TRIE_HASH);
    // let global_state = GlobalState::default();
    // println!("Fuck3");
    // let post_state = execute(b1, global_state);

    // println!("{:?}", post_state);
    println!("hello");
}

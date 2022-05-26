use chrono::Utc;
use log::{error, warn};

// holds the application state
pub struct App {
    // todo: i want to try with generic later on
    pub blocks: Vec<Block>,
}

// state is a list of blocks
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub id: u64,
    pub hash: String,
    // sha256
    pub previous_hash: String,
    pub timestamp: i64,
    pub data: String,
    pub nonce: u64,
}


/*
*   Basis for our simplistic mining scheme.
*   Essentially, when mining a block, the person mining has to hash the data for the block
*   (with SHA256, in our case) and find a hash, which, in binary, starts with 00 (two zeros).
*   This also denotes our “difficulty” on the network.
*/
const DIFFICULTY_PREFIX: &str = "00";

impl App {
    fn new() -> Self {
        Self { blocks: vec![] }
    }

    fn genesis(&mut self) {
        let genesis_block = Block {
            id: 0,
            timestamp: Utc::now().timestamp(),
            previous_hash: String::from("genesis"),
            data: String::from("genesis!"),
            nonce: 2836,
            hash: "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(),
        };

        self.blocks.push(genesis_block);
    }

    fn is_block_valid(&self, block: &Block, previous_block: &Block) -> bool {
        if block.previous_hash != previous_block.hash {
            warn!("block with id: {} has wrong previous hash", block.id);
            return false;
        } else if !hash_to_binary_representation(
            &hex::decode(&block.hash).expect("can decode from hex"),
        ).starts_with(DIFFICULTY_PREFIX) {
            warn!("block with id: {} has invalid difficulty", block.id);
            return false;
        } else if block.id != previous_block.id + 1 {
            warn!(
                "block with id: {} is not the next block after the latest: {}",
                block.id, previous_block.id
            );
            return false;
        } else if hex::encode(
            calculate_hash(
                block.id,
                block.timestamp,
                &block.previous_hash,
                &block.data,
                block.nonce,
            )
        ) != block.hash
        {
            warn!("block with id: {} has invalid hash", block.id);
            return false;
        }
        true
    }

    fn try_add_block(&mut self, block: Block) {
        let latest_block = self.blocks.last().expect("there is at least one block");

        if self.is_block_valid(&block, latest_block) {
            self.blocks.push(block);
        } else {
            error!("could not add block - invalid");
        }
    }

    // validating whole chain
    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len() {
            if i == 0 {
                // ignoring the genesis block
                continue;
            }

            let first = chain.get(i - 1).expect("has to exist");
            let second = chain.get(i).expect("has to exist");

            if !self.is_block_valid(second, first) {
                return false;
            }
        }
        true
    }

    // We always choose the longest valid chain
    fn choose_chain(&mut self, local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        // todo: i want to try with generic later on
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);

        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len() {
                local
            } else {
                remote
            }
        } else if is_remote_valid && !is_local_valid {
            remote
        } else if !is_remote_valid && is_local_valid {
            local
        } else {
            panic!("local and remote chains are both invalid");
        }
    }
}

fn hash_to_binary_representation(hash: &[u8]) -> String {
    let mut res: String = String::default();

    for c in hash {
        res.push_str(&format!("{:b}", c));
    }
    res
}

fn main() {
    // let aux = App::<Block>::new(); // turbofish syntax
    let aux = App::new();
}

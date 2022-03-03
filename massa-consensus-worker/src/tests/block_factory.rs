// Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::{
    mock_protocol_controller::MockProtocolController,
    tools::{validate_notpropagate_block, validate_propagate_block},
};
use massa_hash::hash::Hash;
use massa_models::{Block, BlockHeader, BlockHeaderContent, BlockId, Endorsement, Operation, Slot};
use massa_signature::{derive_public_key, generate_random_private_key, PrivateKey};

pub struct BlockFactory {
    pub best_parents: Vec<BlockId>,
    pub creator_priv_key: PrivateKey,
    pub slot: Slot,
    pub endorsements: Vec<Endorsement>,
    pub operations: Vec<Operation>,
    pub protocol_controller: MockProtocolController,
}

impl BlockFactory {
    pub fn start_block_factory(
        genesis: Vec<BlockId>,
        protocol_controller: MockProtocolController,
    ) -> BlockFactory {
        BlockFactory {
            best_parents: genesis,
            creator_priv_key: generate_random_private_key(),
            slot: Slot::new(1, 0),
            endorsements: Vec::new(),
            operations: Vec::new(),
            protocol_controller,
        }
    }

    pub async fn create_and_receive_block(&mut self, valid: bool) -> (BlockId, Block) {
        let public_key = derive_public_key(&self.creator_priv_key);
        let (hash, header) = BlockHeader::new_signed(
            &self.creator_priv_key,
            BlockHeaderContent {
                creator: public_key,
                slot: self.slot,
                parents: self.best_parents.clone(),
                operation_merkle_root: Hash::compute_from(
                    &self
                        .operations
                        .iter()
                        .flat_map(|op| op.get_operation_id().unwrap().to_bytes())
                        .collect::<Vec<_>>()[..],
                ),
                endorsements: self.endorsements.clone(),
            },
        )
        .unwrap();

        let block = Block {
            header,
            operations: self.operations.clone(),
        };

        self.protocol_controller.receive_block(block.clone()).await;
        if valid {
            // Assert that the block is propagated.
            validate_propagate_block(&mut self.protocol_controller, hash, 2000).await;
        } else {
            // Assert that the the block is not propagated.
            validate_notpropagate_block(&mut self.protocol_controller, hash, 500).await;
        }
        (hash, block)
    }

    pub fn sign_header(&self, header: BlockHeaderContent) -> Block {
        let _public_key = derive_public_key(&self.creator_priv_key);
        let (_hash, header) = BlockHeader::new_signed(&self.creator_priv_key, header).unwrap();

        Block {
            header,
            operations: self.operations.clone(),
        }
    }

    pub async fn receieve_block(&mut self, valid: bool, block: Block) {
        let hash = block.header.compute_block_id().unwrap();
        self.protocol_controller.receive_block(block.clone()).await;
        if valid {
            // Assert that the block is propagated.
            validate_propagate_block(&mut self.protocol_controller, hash, 2000).await;
        } else {
            // Assert that the the block is not propagated.
            validate_notpropagate_block(&mut self.protocol_controller, hash, 500).await;
        }
    }

    pub fn take_protocol_controller(self) -> MockProtocolController {
        self.protocol_controller
    }
}

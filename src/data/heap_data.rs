use std::borrow::Cow;

use super::{Block, Data, DataWriter, Metadata};

pub struct HeapData {
    metadata: Metadata,
    blocks: Vec<Block>
}


impl Data<Vec<Block>> for HeapData {
    fn new(metadata: Metadata, blocks: Vec<Block>) -> HeapData {
        if blocks.len() != metadata.data_blocks() as usize {
            panic!("Invalid blocks! Metadata");
        }
        HeapData {
            metadata: metadata,
            blocks: blocks
        }
    }

    fn get_metadata(&self) -> Metadata {
        self.metadata
    }

    fn read_block(&self, block_number: u32) -> Cow<Block> {
        unimplemented!()
    }
}

pub struct HeapDataWriter {
}
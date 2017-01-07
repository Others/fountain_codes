use std::cmp;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor};
use std::ops::{BitXor, BitXorAssign, Index};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use super::{Client, CreationError, Data, Decoder, Encoder, Metadata, Packet, PartialEncoder, Source};
use super::distributions::{Distribution, RobustSolitonDistribution};


// These constants are parameters to the robust soltion distribution
const DEFAULT_FAILURE_PROBABILITY: f64 = 0.1;
const DEFAULT_HINT_CONSTANT: f64 = 0.3;

pub struct LtSource {
    blocks: Vec<Block>,
    distribution: Distribution
}

impl Source<LtPacket> for LtSource {
    fn new(metadata: Metadata, data: Data) -> Result<Self, CreationError> {
        let data_bytes = metadata.data_bytes();

        if data_bytes == 0 {
            return Err(CreationError::DataZeroBytes);
        }

        if data_bytes != data.len() as u64 {
            return Err(CreationError::InvalidMetadata);
        }

        let extra_block = cmp::min((data_bytes % BLOCK_BYTES as u64), 1);

        let block_count = (data_bytes / (BLOCK_BYTES as u64)) + extra_block;
        if block_count > (u32::max_value() as u64) {
            return Err(CreationError::DataTooBig)
        }

        let mut blocks: Vec<Block> = Vec::with_capacity(block_count as usize);
        for chunk in data.chunks(BLOCK_BYTES) {
            let mut block = [0; BLOCK_BYTES];
            for i in 0..chunk.len() {
                block[i] = chunk[i];
            }
            blocks.push(Block::from_data(block));
        }

        let density_function = RobustSolitonDistribution::new_using_heuristic(DEFAULT_FAILURE_PROBABILITY, DEFAULT_HINT_CONSTANT);
        let distribution = Distribution::new(&density_function, block_count as u32).map_err(|e| CreationError::RandomInitializationError(e))?;

        Ok(LtSource{
            blocks: blocks,
            distribution: distribution
        })
    }
}

fn choose_blocks_to_combine(distribution: &Distribution, blocks: &mut Vec<u32>) {
    // TODO: Ensure this "as usize" is safe
    let blocks_to_combine = cmp::min(blocks.len(), distribution.query() as usize);

    for i in 0..blocks_to_combine {
        let j = distribution.query_interior_rng_usize(i, blocks.len());
        blocks.swap(i, j);
    }

    blocks.truncate(blocks_to_combine as usize);
}

impl Encoder<LtPacket> for LtSource {
    fn create_packet(&self) -> LtPacket {
        let block_count = self.blocks.len();

        let mut blocks: Vec<u32> = Vec::with_capacity(block_count);
        for i in 0..block_count{
            blocks.push(i as u32);
        }

        choose_blocks_to_combine(&self.distribution, &mut blocks);

        let mut new_block = Block::new();
        for block_id in &blocks {
            new_block ^= self.blocks.index(*block_id as usize);
        }

        LtPacket::new(blocks, new_block)
    }
}

#[derive(Debug)]
pub struct LtClient {
    metadata: Metadata,
    block_count: u32,

    distribution: Distribution,

    decoded_blocks: HashMap<u32, Block>,

    // TODO: Can we organize this data to find Packets containing certain blocks quicker?
    // TODO: Refactor to do only one pass if the block cannot be simplified, modifying in place
    stale_packets: HashSet<LtPacket>
}

impl Client<LtPacket> for LtClient {
    fn new(metadata: Metadata) -> Result<Self, CreationError> {
        let data_bytes = metadata.data_bytes();

        if data_bytes == 0 {
            return Err(CreationError::DataZeroBytes)
        }

        // If BLOCK_BYTES goes evenly into data_bytes we don't need an extra block, but otherwise we do
        let extra_block = cmp::min((data_bytes % BLOCK_BYTES as u64), 1);

        let block_count = (data_bytes / (BLOCK_BYTES as u64)) + extra_block;
        if block_count > (u32::max_value() as u64) {
            return Err(CreationError::DataTooBig)
        }

        let density_function = RobustSolitonDistribution::new_using_heuristic(DEFAULT_FAILURE_PROBABILITY, DEFAULT_HINT_CONSTANT);
        let distribution = Distribution::new(&density_function, block_count as u32).map_err(|e| CreationError::RandomInitializationError(e))?;

        Ok(LtClient {
            metadata: metadata,
            block_count: block_count as u32,

            distribution: distribution,

            decoded_blocks: HashMap::new(),
            stale_packets: HashSet::new()
        })
    }
}

// TODO: Unify duplicate code in LtClient and LtSource
impl PartialEncoder<LtPacket> for LtClient {
    fn try_create_packet(&self) -> Option<LtPacket> {
        let mut blocks: Vec<u32> = Vec::with_capacity(self.decoded_blocks.len());

        for &key in self.decoded_blocks.keys() {
            blocks.push(key);
        }

        if blocks.len() == 0 {
            return None;
        }

        choose_blocks_to_combine(&self.distribution, &mut blocks);

        let mut new_block = Block::new();
        for block_id in &blocks {
            new_block = new_block ^ self.decoded_blocks.index(block_id);
        }

        return Some(LtPacket::new(blocks, new_block));
    }
}

impl Decoder<LtPacket> for LtClient {

    fn receive_packet(&mut self, packet: LtPacket) {
        // TODO: Investigate using sets instead of vectors here

        // Fresh packets might turn out to be reducible
        let mut fresh_packets: Vec<LtPacket> = vec![packet];
        // Stale packets we know are irreducible unless we decode a new block

        while let Some(packet) = fresh_packets.pop() {
            let mut xor: Vec<u32> = Vec::with_capacity(packet.combined_blocks.len());

            let mut multiple_remaining = false;
            let mut remainder: Option<u32> = None;

            for block_id in &packet.combined_blocks {
                if self.decoded_blocks.contains_key(&block_id) {
                    xor.push(*block_id);
                } else {
                    remainder = match remainder {
                        Option::None => {
                            Some(*block_id)
                        }
                        Option::Some(remainder) => {
                            multiple_remaining = true;
                            Some(remainder)
                        }
                    };

                    if multiple_remaining {
                        break;
                    }
                }
            }

            if multiple_remaining || remainder.is_none(){
                self.stale_packets.insert(packet);
            }else {
                let block_id = remainder.unwrap();
                if !self.decoded_blocks.contains_key(&block_id) {
                    let mut data = packet.data;
                    for block_id in xor {
                        data = data ^ self.decoded_blocks.get(&block_id).expect("Blocks selected to be xor'd must exist");
                    }

                    self.decoded_blocks.insert(block_id, data);

                    // TODO: Get rid of this unnecessary copy (check if it's optimized out)
                    // TODO: Test giving this a good capacity
                    let mut refreshed_packets: Vec<LtPacket> = Vec::new();

                    // Note: Using unsafe just isn't worth it here, it isn't a big win
                    for stale_packet in &self.stale_packets {
                        if stale_packet.combined_blocks.contains(&block_id) {
                            refreshed_packets.push(stale_packet.clone());
                        }
                    }
                    for packet in refreshed_packets {
                        self.stale_packets.remove(&packet);
                        fresh_packets.push(packet);
                    }
                }
            }
        }
    }

    fn get_result(&self) -> Option<Data> {
        if self.decoded_blocks.len() < self.block_count as usize {
            return None;
        }

        let mut block_bytes: Vec<u8> = Vec::with_capacity(self.metadata.data_bytes() as usize);
        for i in 0..self.block_count {
            let block_option = self.decoded_blocks.get(&i);
            if block_option.is_none() {
                // TODO: Figure out whether we should panic here, since it indicates bad entries in the decoded_blocks map
                return None;
            }
            block_bytes.extend_from_slice(block_option.unwrap().data());
        }
        // We have to truncate here, because extra padding may have been added
        block_bytes.truncate(self.metadata.data_bytes() as usize);
        Some(block_bytes)
    }

    fn decoding_progress(&self) -> f64 {
        (self.decoded_blocks.len() as f64) / (self.block_count as f64)
    }
}

// We use a wrapper struct so we can impl on Block
const BLOCK_BYTES: usize = 1024;

struct Block {
    data: [u8; BLOCK_BYTES]
}

impl Block {
    fn new() -> Block {
        Block {
            data: [0; BLOCK_BYTES]
        }
    }

    fn from_data(data: [u8; BLOCK_BYTES]) -> Block {
        Block {
            data: data
        }
    }

    fn data(&self) -> &[u8] {
        &self.data[..]
    }
}

impl<'a> BitXorAssign<&'a Block> for Block {
    fn bitxor_assign(&mut self, rhs: &'a Block) {
        for i in 0..BLOCK_BYTES {
            self.data[i] ^= rhs.data[i]
        }
    }
}

impl<'a> BitXor<&'a Block> for Block {
    type Output = Self;

    fn bitxor(self, rhs: &'a Block) -> Self {
        let mut result = self;
        result ^= rhs;
        return result;
    }
}

// Large fixed size arrays break auto-deriving
impl Clone for Block {
    fn clone(&self) -> Self {
        Block {
            data: self.data
        }
    }
}

impl Debug for Block {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        fmt.write_str(&format!("{:?}", &self.data[..]))
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        &self.data[..] == &other.data[..]
    }
}

impl Eq for Block {}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.data[..])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtPacket {
    // TODO: Test making this a set, for faster lookup. (When picking elements just use a loop that selects.)
    combined_blocks: Vec<u32>,
    data: Block
}

impl LtPacket {
    fn new(combined_blocks: Vec<u32>, data: Block) -> LtPacket {
        LtPacket {
            combined_blocks: combined_blocks,
            data: data
        }
    }
}

impl Packet for LtPacket {
    fn from_bytes(bytes: Vec<u8>) -> io::Result<LtPacket> {
        let mut rdr = Cursor::new(bytes);

        let block_count = rdr.read_u32::<BigEndian>()?;
        let mut combined_blocks = Vec::new();
        for _ in 0..block_count {
            let block = rdr.read_u32::<BigEndian>()?;
            combined_blocks.push(block);
        }

        let mut block_data = [0; BLOCK_BYTES];
        for i in 0..BLOCK_BYTES {
            block_data[i] = rdr.read_u8()?;
        }

        let block = Block::from_data(block_data);

        Ok(LtPacket::new(combined_blocks, block))
    }

    fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let mut dest = Vec::new();

        dest.write_u32::<BigEndian>(self.combined_blocks.len() as u32)?;
        for block in &self.combined_blocks {
            dest.write_u32::<BigEndian>(*block)?;
        }

        for byte in self.data.data() {
            dest.write_u8(*byte)?;
        }

        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Packet;
    use super::{BLOCK_BYTES, Block, LtPacket};

    #[test]
    fn block_equals() {
        assert_eq!(Block::new() ^ &Block::new(), Block::new());

        let one_block = Block::from_data([1; BLOCK_BYTES]);

        assert_eq!(one_block.clone() ^ &Block::new(), one_block);
    }

    #[test]
    fn packet_round_trips() {
        let combined_blocks = vec![1, 2, 3, 4, 5];
        let block_data = [0; BLOCK_BYTES];
        let packet = LtPacket::new(combined_blocks.clone(), Block::from_data(block_data).clone());

        let bytes = packet.clone().to_bytes().unwrap();

        assert_eq!(LtPacket::from_bytes(bytes).unwrap(), packet);
    }
}
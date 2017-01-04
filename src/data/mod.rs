use std::borrow::Cow;
use std::cmp;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::io;
use std::ops::{BitXor, BitXorAssign};

pub mod heap_data;
use self::heap_data::{HeapData, HeapDataWriter};


// TODO: Add fingerprint to Metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    data_bytes: u64,
    data_blocks: u32
}

pub enum MetadataError {
    DataZeroBytes,
    DataTooBig
}

impl Metadata {
    pub fn new(data_bytes: u64, block_bytes: u32) -> Result<Metadata, MetadataError> {
        if data_bytes == 0 {
            return Err(MetadataError::DataZeroBytes)
        }

        // If block_bytes goes evenly into data_bytes we don't need an extra block, but otherwise we do
        let extra_block = cmp::min((data_bytes % block_bytes as u64), 1);
        let data_blocks = (data_bytes / block_bytes as u64) + extra_block;

        if data_blocks > (u32::max_value() as u64) {
            return Err(MetadataError::DataTooBig)
        }

        Ok(Metadata {
            data_bytes: data_bytes,
            data_blocks: data_blocks as u32
        })
    }

    pub fn data_bytes(&self) -> u64 {
        self.data_bytes
    }

    pub fn data_blocks(&self) -> u32 {
        self.data_blocks
    }
}

pub trait Data<P>{
    fn new(metadata: Metadata, p: P) -> Self;

    fn get_metadata(&self) -> Metadata;

    fn read_block(&self, block_number: u32) -> Cow<Block>;
}

pub trait DataWriter<T, P> where T: Data<P> {
    fn get_metadata(&self) -> Metadata;

    fn written_blocks(&self) -> Vec<u32>;

    fn read_block<'a>(&'a self, index: u32) -> Result<Cow<'a, Block>, ReadBlockError>;

    fn write_block(&mut self, index:u32, value: Block) -> io::Result<()>;

    fn get_finalizer(&self) -> Option<Box<FnOnce(Self) -> Result<T, DataFinalizationError>>>;
}

pub enum ReadBlockError {
    Io(io::Error),
    BlockNotWritten
}

pub enum DataFinalizationError {
    IoError(io::Error)
}

// We use a wrapper struct so we can impl on Block
struct Block {
    data: [u8; BLOCK_BYTES]
}
// TODO: Benchmark the performance win of fixed size blocks
pub const BLOCK_BYTES: usize = 1024;

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

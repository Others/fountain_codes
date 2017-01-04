extern crate byteorder;
extern crate rand;

use std::io;

mod metadata;
pub use metadata::Metadata;

pub mod lt;
pub use lt::{LtClient, LtSource};

mod distributions;

// TODO: Make Data more generic
type Data = Vec<u8>;

pub trait Packet: Sized {
    fn from_bytes(bytes: Vec<u8>) -> io::Result<Self>;

    fn to_bytes(&self) -> io::Result<Vec<u8>>;
}

pub trait Encoder<P: Packet> {
    fn create_packet(&self) -> P;
}

pub trait PartialEncoder<P: Packet> {
    fn try_create_packet(&self) -> Option<P>;
}

impl<P: Packet> PartialEncoder<P> for Encoder<P> {
    fn try_create_packet(&self) -> Option<P> {
        Some(self.create_packet())
    }
}

pub trait Decoder<P: Packet> {
    fn receive_packet(&mut self, packet: P);

    fn decoding_progress(&self) -> f64;

    fn get_result(&self) -> Option<Data>;
}

pub trait Source<P: Packet> : Encoder<P> + Sized {
    fn new(metadata: Metadata, data: Data) -> Result<Self, CreationError>;
}

// TODO: Figure out if Clients should be generic over some sort of "parameter" type
pub trait Client<P: Packet> : Decoder<P> + PartialEncoder<P> + Sized {
    fn new(metadata: Metadata) -> Result<Self, CreationError>;
}

#[derive(Debug)]
pub enum CreationError {
    DataZeroBytes,
    DataTooBig,
    InvalidMetadata,
    RandomInitializationError(io::Error)
}
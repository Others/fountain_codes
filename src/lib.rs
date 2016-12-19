extern crate rand;

use std::io;

mod metadata;
pub use metadata::Metadata;

pub mod lt;
pub use lt::{LtClient, LtSource};

mod distributions;

// TODO: Make Data more generic
type Data = Vec<u8>;

pub trait Encoder<P> {
    fn create_packet(&self) -> P;
}

pub trait PartialEncoder<P> {
    fn try_create_packet(&self) -> Option<P>;
}

impl<P> PartialEncoder<P> for Encoder<P> {
    fn try_create_packet(&self) -> Option<P> {
        Some(self.create_packet())
    }
}

pub trait Decoder<P> {
    fn receive_packet(&mut self, packet: P);

    fn decoding_progress(&self) -> f64;

    fn get_result(&self) -> Option<Data>;
}

pub trait Source<P> : Encoder<P> + Sized {
    fn new(metadata: Metadata, data: Data) -> Result<Self, CreationError>;
}

// TODO: Figure out if Clients should be generic over some sort of "parameter" type
pub trait Client<P> : Decoder<P> + PartialEncoder<P> + Sized {
    fn new(metadata: Metadata) -> Result<Self, CreationError>;
}

#[derive(Debug)]
pub enum CreationError {
    DataZeroBytes,
    DataTooBig,
    InvalidMetadata,
    RandomInitializationError(io::Error)
}
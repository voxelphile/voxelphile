use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Write,
    mem,
};

use flate2::{
    write::{DeflateDecoder, DeflateEncoder},
    Compression,
};
use serde_derive::{Deserialize, Serialize};

pub type PacketId = usize;

pub enum PacketError {
    Serialize,
    Compress,
    Checksum,
    Decompress,
    Deserialize,
}

pub enum SocketError {
    Bind,
    Connect,
    Nonblocking,
}

#[derive(Serialize, Deserialize)]
pub struct Header {
    id: PacketId,
}

#[derive(Serialize, Deserialize)]
pub struct Packet<M> {
    pub header: Header,
    pub message: M,
}

fn checksum_hash(data: &[u8]) -> [u8; 8] {
    let mut hasher = DefaultHasher::default();
    data.hash(&mut hasher);
    hasher.finish().to_be_bytes()
}

impl<M: serde::ser::Serialize + serde::de::DeserializeOwned> Packet<M> {
    fn encode(&self) -> Result<Vec<u8>, PacketError> {
        let mut data = vec![];

        let bytecode = bincode::serialize(self).map_err(|_| PacketError::Serialize)?;

        let payload = {
            let mut compressor = DeflateEncoder::new(vec![], Compression::best());
            compressor
                .write_all(&bytecode)
                .map_err(|_| PacketError::Compress)?;
            compressor.finish().map_err(|_| PacketError::Compress)?
        };

        let checksum = checksum_hash(&payload);

        data.extend(checksum);
        data.extend(payload);

        Ok(data)
    }

    fn decode(data: &[u8]) -> Result<Self, PacketError> {
        const U64_BYTES: usize = mem::size_of::<u64>();

        let checksum = &data[..U64_BYTES];
        let payload = &data[U64_BYTES..];

        if checksum != checksum_hash(payload) {
            Err(PacketError::Checksum)?
        }

        let payload = {
            let mut decompressor = DeflateDecoder::new(vec![]);
            decompressor
                .write_all(&payload)
                .map_err(|_| PacketError::Decompress)?;
            decompressor.finish().map_err(|_| PacketError::Decompress)?
        };

        let this = bincode::deserialize(&payload).map_err(|_| PacketError::Deserialize)?;

        Ok(this)
    }
}

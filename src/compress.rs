//! Writes packs, which needs a real LZMA compressor. Only build tools use this.

use sm_logic::pack::{encode, EncodeOptions, PackError};
use sm_logic::World;
use std::io::Write;
use xz2::stream::{LzmaOptions, Stream};
use xz2::write::XzEncoder;

/// liblzma's flag for spending extra effort on compression.
const PRESET_EXTREME: u32 = 1 << 31;

/// The best settings found by trying them all on the data: the bytes aren't aligned
/// to any boundary, so position bits don't help.
pub fn compress_lzma(data: &[u8]) -> Vec<u8> {
    let mut options = LzmaOptions::new_preset(9 | PRESET_EXTREME).expect("the preset is valid");
    options.literal_context_bits(2).literal_position_bits(0).position_bits(0);
    let stream = Stream::new_lzma_encoder(&options).expect("the settings are valid");
    let mut encoder = XzEncoder::new_stream(Vec::new(), stream);
    encoder.write_all(data).expect("writing to memory can't fail");
    encoder.finish().expect("writing to memory can't fail")
}

/// Writes a world as a pack that [`sm_logic::pack::decode`] can read.
pub fn encode_pack(world: &World, options: &EncodeOptions) -> Result<Vec<u8>, PackError> {
    encode(world, options, compress_lzma)
}

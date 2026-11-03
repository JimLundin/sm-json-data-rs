//! A small decoder for the `.lzma` ("LZMA alone") format.
//!
//! Compression is left to build tools, because decoding is all a game needs. This
//! follows the LZMA specification and only ever indexes checked positions, so bad
//! input gives an error rather than a panic.

const TOP: u32 = 1 << 24;
const PROB_BITS: u32 = 11;
const PROB_INIT: u16 = 1 << (PROB_BITS - 1);
const MOVE_BITS: u32 = 5;
const STATES: usize = 12;
const POS_STATES: usize = 16;
const HEADER: usize = 13;
const UNKNOWN_SIZE: u64 = u64::MAX;
const END_MARKER: u32 = u32::MAX;

struct RangeDecoder<'a> {
    input: &'a [u8],
    range: u32,
    code: u32,
    /// Set when the input runs out, and checked once per symbol.
    exhausted: bool,
}

impl RangeDecoder<'_> {
    fn next_byte(&mut self) -> u32 {
        match self.input.split_first() {
            Some((byte, rest)) => {
                self.input = rest;
                u32::from(*byte)
            }
            None => {
                self.exhausted = true;
                0
            }
        }
    }

    fn normalize(&mut self) {
        if self.range < TOP {
            self.range <<= 8;
            self.code = (self.code << 8) | self.next_byte();
        }
    }

    fn bit(&mut self, prob: &mut u16) -> u32 {
        let bound = (self.range >> PROB_BITS) * u32::from(*prob);
        let bit = if self.code < bound {
            self.range = bound;
            *prob += ((1 << PROB_BITS) - *prob) >> MOVE_BITS;
            0
        } else {
            self.code = self.code.wrapping_sub(bound);
            self.range -= bound;
            *prob -= *prob >> MOVE_BITS;
            1
        };
        self.normalize();
        bit
    }

    fn direct_bits(&mut self, count: u32) -> u32 {
        let mut value = 0u32;
        for _ in 0..count {
            self.range >>= 1;
            self.code = self.code.wrapping_sub(self.range);
            let mask = 0u32.wrapping_sub(self.code >> 31);
            self.code = self.code.wrapping_add(self.range & mask);
            self.normalize();
            value = (value << 1).wrapping_add(mask.wrapping_add(1));
        }
        value
    }

    fn tree(&mut self, probs: &mut [u16], bits: u32) -> u32 {
        let mut node = 1usize;
        for _ in 0..bits {
            node = (node << 1) + self.bit(&mut probs[node]) as usize;
        }
        node as u32 - (1 << bits)
    }

    fn reverse_tree(&mut self, probs: &mut [u16], bits: u32) -> u32 {
        let mut node = 1usize;
        let mut value = 0;
        for index in 0..bits {
            let bit = self.bit(&mut probs[node]);
            node = (node << 1) + bit as usize;
            value |= bit << index;
        }
        value
    }
}

struct LengthDecoder {
    choice: [u16; 2],
    low: [[u16; 8]; POS_STATES],
    mid: [[u16; 8]; POS_STATES],
    high: [u16; 256],
}

impl LengthDecoder {
    fn new() -> Self {
        Self {
            choice: [PROB_INIT; 2],
            low: [[PROB_INIT; 8]; POS_STATES],
            mid: [[PROB_INIT; 8]; POS_STATES],
            high: [PROB_INIT; 256],
        }
    }

    fn decode(&mut self, rc: &mut RangeDecoder<'_>, pos_state: usize) -> usize {
        let value = if rc.bit(&mut self.choice[0]) == 0 {
            rc.tree(&mut self.low[pos_state], 3)
        } else if rc.bit(&mut self.choice[1]) == 0 {
            8 + rc.tree(&mut self.mid[pos_state], 3)
        } else {
            16 + rc.tree(&mut self.high, 8)
        };
        value as usize + 2
    }
}

/// Decompresses a `.lzma` stream, refusing to produce more than `limit` bytes.
pub(crate) fn decompress(input: &[u8], limit: usize) -> Result<Vec<u8>, &'static str> {
    let header = input.get(..HEADER).ok_or("compressed section is truncated")?;
    let props = usize::from(header[0]);
    if props >= 225 {
        return Err("bad compression settings");
    }
    let lc = props % 9;
    let lp = (props / 9) % 5;
    let pb = props / 45;
    let dict_size = u32::from_le_bytes([header[1], header[2], header[3], header[4]]);
    let size = u64::from_le_bytes([header[5], header[6], header[7], header[8], header[9], header[10], header[11], header[12]]);
    if size != UNKNOWN_SIZE && size > limit as u64 {
        return Err("section is too large");
    }
    let body = &input[HEADER..];
    let (&first, rest) = body.split_first().ok_or("compressed section is truncated")?;
    if first != 0 {
        return Err("bad compressed data");
    }
    let mut rc = RangeDecoder { input: rest, range: u32::MAX, code: 0, exhausted: false };
    for _ in 0..4 {
        rc.code = (rc.code << 8) | rc.next_byte();
    }

    let mut literals = vec![PROB_INIT; 0x300usize << (lc + lp)];
    let mut is_match = [[PROB_INIT; POS_STATES]; STATES];
    let mut is_rep = [PROB_INIT; STATES];
    let mut is_rep_g0 = [PROB_INIT; STATES];
    let mut is_rep_g1 = [PROB_INIT; STATES];
    let mut is_rep_g2 = [PROB_INIT; STATES];
    let mut is_rep0_long = [[PROB_INIT; POS_STATES]; STATES];
    let mut pos_slot = [[PROB_INIT; 64]; 4];
    let mut pos_special = [PROB_INIT; 115];
    let mut align = [PROB_INIT; 16];
    let mut match_length = LengthDecoder::new();
    let mut rep_length = LengthDecoder::new();

    let pos_mask = (1usize << pb) - 1;
    let literal_mask = (1usize << lp) - 1;
    let mut state = 0usize;
    let mut reps = [0u32; 4];
    let mut out: Vec<u8> = Vec::new();

    loop {
        if rc.exhausted {
            return Err("compressed section is truncated");
        }
        if size != UNKNOWN_SIZE && out.len() as u64 == size {
            return Ok(out);
        }
        if out.len() >= limit {
            return Err("section is too large");
        }
        let pos_state = out.len() & pos_mask;

        if rc.bit(&mut is_match[state][pos_state]) == 0 {
            let previous = out.last().copied().unwrap_or(0);
            let context = ((out.len() & literal_mask) << lc) + (usize::from(previous) >> (8 - lc));
            let probs = &mut literals[0x300 * context..0x300 * (context + 1)];
            let mut symbol = 1usize;
            if state >= 7 {
                let distance = reps[0] as usize + 1;
                let mut match_byte = if distance <= out.len() { usize::from(out[out.len() - distance]) } else { 0 };
                while symbol < 0x100 {
                    let match_bit = (match_byte >> 7) & 1;
                    match_byte <<= 1;
                    let bit = rc.bit(&mut probs[((1 + match_bit) << 8) + symbol]) as usize;
                    symbol = (symbol << 1) | bit;
                    if match_bit != bit {
                        break;
                    }
                }
            }
            while symbol < 0x100 {
                symbol = (symbol << 1) | rc.bit(&mut probs[symbol]) as usize;
            }
            out.push(symbol as u8);
            state = match state {
                0..=3 => 0,
                4..=9 => state - 3,
                _ => state - 6,
            };
            continue;
        }

        let length;
        if rc.bit(&mut is_rep[state]) != 0 {
            if out.is_empty() {
                return Err("bad compressed data");
            }
            if rc.bit(&mut is_rep_g0[state]) == 0 {
                if rc.bit(&mut is_rep0_long[state][pos_state]) == 0 {
                    state = if state < 7 { 9 } else { 11 };
                    let distance = reps[0] as usize + 1;
                    if distance > out.len() {
                        return Err("bad compressed data");
                    }
                    out.push(out[out.len() - distance]);
                    continue;
                }
            } else {
                let distance = if rc.bit(&mut is_rep_g1[state]) == 0 {
                    reps[1]
                } else {
                    let distance = if rc.bit(&mut is_rep_g2[state]) == 0 {
                        reps[2]
                    } else {
                        let distance = reps[3];
                        reps[3] = reps[2];
                        distance
                    };
                    reps[2] = reps[1];
                    distance
                };
                reps[1] = reps[0];
                reps[0] = distance;
            }
            length = rep_length.decode(&mut rc, pos_state);
            state = if state < 7 { 8 } else { 11 };
        } else {
            reps = [0, reps[0], reps[1], reps[2]];
            length = match_length.decode(&mut rc, pos_state);
            state = if state < 7 { 7 } else { 10 };

            let length_state = (length - 2).min(3);
            let slot = rc.tree(&mut pos_slot[length_state], 6);
            reps[0] = if slot < 4 {
                slot
            } else {
                let direct = (slot >> 1) - 1;
                let base = (2 | (slot & 1)) << direct;
                if slot < 14 {
                    let start = (base as usize + 1) - slot as usize;
                    let end = (start + (1usize << direct)).min(pos_special.len());
                    base.wrapping_add(rc.reverse_tree(&mut pos_special[start - 1..end], direct))
                } else {
                    let high = rc.direct_bits(direct - 4) << 4;
                    base.wrapping_add(high).wrapping_add(rc.reverse_tree(&mut align, 4))
                }
            };
            if reps[0] == END_MARKER {
                if size != UNKNOWN_SIZE && out.len() as u64 != size {
                    return Err("compressed section ended early");
                }
                return Ok(out);
            }
        }

        let distance = reps[0] as usize + 1;
        if distance > out.len() || distance > (dict_size.max(4096) as usize) {
            return Err("bad compressed data");
        }
        let remaining = if size == UNKNOWN_SIZE { usize::MAX } else { (size as usize) - out.len() };
        if length > remaining || out.len().saturating_add(length) > limit {
            return Err("section is too large");
        }
        for _ in 0..length {
            out.push(out[out.len() - distance]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `xz --format=lzma` output for `HELLO_LZMA`, as written with an unknown size
    /// and an end marker.
    const SAMPLE: &[u8] = &[93, 0, 0, 0, 4, 255, 255, 255, 255, 255, 255, 255, 255, 0, 52, 25, 73, 238, 141, 233, 80, 149, 252, 11, 238, 251, 32, 97, 183, 161, 227, 157, 13, 231, 255, 255, 47, 32, 0, 0];
    const HELLO_LZMA: &[u8] = b"hello hello hello lzma, hello lzma, hello!";

    #[test]
    fn decodes_a_stream_with_an_end_marker() {
        assert_eq!(decompress(SAMPLE, 1000).unwrap(), HELLO_LZMA);
    }

    #[test]
    fn honors_a_known_size() {
        let mut known = SAMPLE.to_vec();
        known[5..13].copy_from_slice(&(HELLO_LZMA.len() as u64).to_le_bytes());
        assert_eq!(decompress(&known, 1000).unwrap(), HELLO_LZMA);
    }

    #[test]
    fn enforces_the_limit() {
        assert!(decompress(SAMPLE, 3).is_err());
    }

    #[test]
    fn rejects_bad_input_without_panicking() {
        assert!(decompress(&[], 10).is_err());
        assert!(decompress(&SAMPLE[..HEADER], 10).is_err());
        assert!(decompress(&SAMPLE[..SAMPLE.len() - 6], 1000).is_err());
        let mut bad = SAMPLE.to_vec();
        bad[0] = 225;
        assert!(decompress(&bad, 1000).is_err());
        for index in 0..SAMPLE.len() {
            for flip in [1u8, 0x10, 0xff] {
                let mut copy = SAMPLE.to_vec();
                copy[index] ^= flip;
                let _ = decompress(&copy, 1000);
            }
        }
    }
}

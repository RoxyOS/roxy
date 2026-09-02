//! Encoding of supported-code lists into the Linux capability bitmap layout.
//!
//! Linux exposes device capabilities to user space through `EVIOCGBIT`, which fills a caller
//! buffer with a bitmap where bit *code* is set iff the device supports the event code
//! `code`. Bitmaps are returned one byte per 8 codes, least-significant bit first. These
//! helpers build that layout from a list of supported codes; the caller decides how much of the
//! returned bitmap to copy into the user buffer (Linux zero-pads past the end).

/// Writes a capability bitmap for the given supported codes into `buffer`.
///
/// Returns the number of bytes the full bitmap occupies (the highest supported code + 1,
/// rounded up to a byte), so the caller can report how many bytes the device logically exposes.
/// The bitmap is truncated to `buffer.len()`; callers that pass a buffer shorter than the full
/// code range (e.g. Xorg reads a single byte for probing) receive only the leading bits.
pub fn encode_bits_bitmap(supported: &[u16], buffer: &mut [u8]) -> usize {
    let bytes = core::cmp::min(needed_bytes(supported), buffer.len());
    for &code in supported {
        let byte = usize::from(code) / 8;
        let bit = usize::from(code) % 8;
        if byte < bytes {
            buffer[byte] |= 1 << bit;
        }
    }
    needed_bytes(supported)
}

/// Number of bytes required to represent the highest supported code + 1.
fn needed_bytes(supported: &[u16]) -> usize {
    match supported.iter().max() {
        Some(&max) => usize::from(max) / 8 + 1,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::encode_bits_bitmap;

    #[test]
    fn sets_the_correct_bits() {
        let mut buffer = [0u8; 8];
        // KEY_ESC=1, KEY_ENTER=28, KEY_A=30
        let n = encode_bits_bitmap(&[1, 28, 30], &mut buffer);
        assert_eq!(n, 4); // ceil((30+1)/8)
        // byte 0: bit 1 (ESC=1)
        assert_eq!(buffer[0], 0b0000_0010);
        // byte 3: bit 4 (ENTER=28) and bit 6 (A=30)
        assert_eq!(buffer[3], 0b0101_0000);
    }
}

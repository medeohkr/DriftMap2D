// format.rs reads bounded tile payloads and little-endian field values

use half::f16;
use std::{borrow::Cow, io::Read};

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

pub(super) fn is_zstd(bytes: &[u8]) -> bool {
    bytes.starts_with(&ZSTD_MAGIC)
}

pub(super) fn decompress(bytes: &[u8], limit: usize) -> Result<Cow<'_, [u8]>, String> {
    if !is_zstd(bytes) {
        return if bytes.len() <= limit {
            Ok(Cow::Borrowed(bytes))
        } else {
            Err("tile exceeds decoded size limit".into())
        };
    }
    let decoder = ruzstd::decoding::StreamingDecoder::new(bytes)
        .map_err(|e| format!("invalid zstd header: {e}"))?;
    let mut decoded = Vec::new();
    decoder
        .take(limit as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|e| format!("failed to decompress tile: {e}"))?;
    if decoded.len() > limit {
        return Err("tile exceeds decoded size limit".into());
    }
    Ok(Cow::Owned(decoded))
}

pub(super) fn checked_product(values: &[usize]) -> Result<usize, String> {
    values.iter().try_fold(1usize, |total, &value| {
        total
            .checked_mul(value)
            .ok_or_else(|| "tile dimensions overflow".into())
    })
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
    pub(super) fn starts_with(&self, magic: &[u8]) -> bool {
        self.bytes[self.offset..].starts_with(magic)
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("tile offset overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated tile payload")?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn u32(&mut self) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)
    }

    pub(super) fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn read_field(&mut self, target: &mut Vec<f32>, count: usize) -> Result<(), String> {
        let bytes = self.take(checked_product(&[count, 2])?)?;
        target.extend(
            bytes
                .chunks_exact(2)
                .map(|v| f16::from_bits(u16::from_le_bytes([v[0], v[1]])).to_f32()),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_and_raw_tiles_respect_the_decoded_limit() {
        let compressed = include_bytes!("norkyst/fixtures/004_005.bin");
        assert!(decompress(compressed, 16).is_err());
        assert!(decompress(compressed, 8_000_000).is_ok());
        assert!(decompress(&[0; 17], 16).is_err());
        assert!(decompress(&[0; 16], 16).is_ok());
        assert!(decompress(&ZSTD_MAGIC, 100).is_err());
    }

    #[test]
    fn field_decoding_preserves_zero_sign_nan_and_infinity() {
        let bytes: Vec<_> = [0_u16, 0x8000, 0x7e00, 0x7c00]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut reader = Reader::new(&bytes);
        let mut field = Vec::new();
        reader.read_field(&mut field, 4).unwrap();
        assert_eq!(field[0].to_bits(), 0.0_f32.to_bits());
        assert_eq!(field[1].to_bits(), (-0.0_f32).to_bits());
        assert!(field[2].is_nan());
        assert_eq!(field[3], f32::INFINITY);
        assert!(reader.u32().is_err());
        assert!(reader.take(usize::MAX).is_err());
        assert!(checked_product(&[usize::MAX, 2]).is_err());
    }
}

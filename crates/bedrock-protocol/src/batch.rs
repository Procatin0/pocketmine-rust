use std::io::{Read, Write};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use crate::binary::{CodecError, Reader, Writer};

pub const COMPRESSION_ZLIB: u8 = 0;
pub const COMPRESSION_SNAPPY: u8 = 1;
pub const COMPRESSION_NONE: u8 = 0xFF;

pub struct PacketBatch;

impl PacketBatch {
    /// Decompresses a Bedrock network payload (prefixed by compression algorithm byte).
    pub fn decompress(payload: &[u8]) -> Result<Vec<u8>, CodecError> {
        if payload.is_empty() {
            return Ok(Vec::new());
        }

        let compression_type = payload[0];
        let data = &payload[1..];

        match compression_type {
            COMPRESSION_ZLIB => {
                let mut decoder = ZlibDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).map_err(|e| {
                    CodecError::Custom(format!("Zlib decompression failed: {}", e))
                })?;
                Ok(decompressed)
            }
            COMPRESSION_NONE => Ok(data.to_vec()),
            other => Err(CodecError::Custom(format!(
                "Unsupported compression algorithm: 0x{:02X}",
                other
            ))),
        }
    }

    /// Compresses a raw Bedrock packet batch payload and prefixes it with the compression algorithm byte.
    pub fn compress(data: &[u8], threshold: usize) -> Result<Vec<u8>, CodecError> {
        if data.len() < threshold {
            let mut out = Vec::with_capacity(data.len() + 1);
            out.push(COMPRESSION_NONE);
            out.extend_from_slice(data);
            return Ok(out);
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).map_err(|e| {
            CodecError::Custom(format!("Zlib compression failed: {}", e))
        })?;
        let compressed = encoder.finish().map_err(|e| {
            CodecError::Custom(format!("Zlib finish failed: {}", e))
        })?;

        let mut out = Vec::with_capacity(compressed.len() + 1);
        out.push(COMPRESSION_ZLIB);
        out.extend_from_slice(&compressed);
        Ok(out)
    }

    /// Decodes individual packets from an uncompressed batch buffer:
    /// Returns Vec<(packet_id, payload)>
    pub fn decode_packets(decompressed: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, CodecError> {
        let mut reader = Reader::new(decompressed);
        let mut packets = Vec::new();

        while reader.remaining() > 0 {
            let length = reader.read_var_u32()? as usize;
            if length == 0 || length > reader.remaining() {
                break;
            }
            let pos_start = reader.position();
            let header = reader.read_var_u32()?;
            let packet_id = header & 0x3FF;
            let header_len = reader.position() - pos_start;
            let payload_len = length.saturating_sub(header_len);
            let payload = reader.read_bytes(payload_len)?.to_vec();
            packets.push((packet_id, payload));
        }

        Ok(packets)
    }

    /// Encodes a single packet into an individual packet entry [var_u32(length), var_u32(header), payload]
    pub fn encode_packet(packet_id: u32, payload: &[u8]) -> Vec<u8> {
        let mut pkt_writer = Writer::new();
        let header = packet_id & 0x3FF;
        pkt_writer.write_var_u32(header);
        pkt_writer.write_bytes(payload);
        let pkt_bytes = pkt_writer.into_bytes();

        let mut entry_writer = Writer::new();
        entry_writer.write_var_u32(pkt_bytes.len() as u32);
        entry_writer.write_bytes(&pkt_bytes);
        entry_writer.into_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_compression_roundtrip() {
        let original = b"Hello Bedrock Protocol Packet Batch Data";
        let compressed = PacketBatch::compress(original, 0).unwrap();
        assert_eq!(compressed[0], COMPRESSION_ZLIB);

        let decompressed = PacketBatch::decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_batch_packets_roundtrip() {
        let p1 = PacketBatch::encode_packet(0x01, b"login_payload");
        let p2 = PacketBatch::encode_packet(0x08, b"resource_pack_payload");

        let mut batch = Vec::new();
        batch.extend_from_slice(&p1);
        batch.extend_from_slice(&p2);

        let decoded = PacketBatch::decode_packets(&batch).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, 0x01);
        assert_eq!(decoded[0].1, b"login_payload");
        assert_eq!(decoded[1].0, 0x08);
        assert_eq!(decoded[1].1, b"resource_pack_payload");
    }
}

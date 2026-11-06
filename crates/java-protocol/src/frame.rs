//! Java packet framing.
//!
//! A frame is `varint(length)` followed by `varint(packet id)` and the packet
//! payload. Compression is disabled: the server never sends `login_compress`
//! (0x03), so the client keeps sending uncompressed frames.

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::binary::{CodecError, Reader, Writer};

/// Upper bound for a single frame body. Vanilla allows 2 MiB packets; chunk
/// data for a 16x16x256 column fits comfortably below it.
pub const MAX_FRAME_LEN: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub id: i32,
    pub payload: Vec<u8>,
}

/// Splits an already de-framed body into packet id and payload.
pub fn decode_frame(body: &[u8]) -> Result<Frame, CodecError> {
    let mut reader = Reader::new(body);
    let id = reader.read_var_i32()?;
    Ok(Frame {
        id,
        payload: reader.rest().to_vec(),
    })
}

/// Builds a complete frame (length prefix included) for a packet id + payload.
pub fn encode_frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = Writer::new();
    body.write_var_i32(id);
    body.write_bytes(payload);
    let body = body.into_inner();

    let mut frame = Writer::new();
    frame.write_var_i32(body.len() as i32);
    frame.write_bytes(&body);
    frame.into_inner()
}

/// Reads one frame body from the stream.
///
/// Returns `Ok(None)` on a clean end of stream (the client closed the socket).
pub async fn read_frame<R>(stream: &mut R) -> std::io::Result<Option<Vec<u8>>>
where
    R: AsyncRead + Unpin,
{
    let mut length: i32 = 0;
    let mut shift = 0;
    loop {
        let mut byte = [0u8; 1];
        let read = stream.read(&mut byte).await?;
        if read == 0 {
            return Ok(None);
        }
        length |= ((byte[0] & 0x7F) as i32) << shift;
        if byte[0] & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 28 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid frame length VarInt",
            ));
        }
    }

    if length < 0 || length as usize > MAX_FRAME_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length out of range: {length}"),
        ));
    }

    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body).await?;
    Ok(Some(body))
}

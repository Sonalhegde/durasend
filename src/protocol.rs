use anyhow::{bail, Context, Result};
use std::io::{Read, Write};

/// Writes a length-prefixed frame: [u32 LE length][payload bytes]
pub fn write_frame<W: Write>(writer: &mut W, data: &[u8]) -> Result<()> {
    let len = data.len() as u32;
    writer.write_all(&len.to_le_bytes())
        .context("Failed to write frame length header")?;
    writer.write_all(data)
        .context("Failed to write frame payload")?;
    writer.flush()
        .context("Failed to flush frame")?;
    Ok(())
}

/// Reads a length-prefixed frame: [u32 LE length][payload bytes]
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            bail!("Connection closed by peer");
        }
        Err(e) => return Err(e).context("Failed to read frame length header"),
    }
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)
        .context("Failed to read full frame payload")?;
    Ok(payload)
}

/// Encodes a chunk payload: [index (u32 LE)][ciphertext bytes]
pub fn encode_chunk_frame(index: u32, ciphertext: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(4 + ciphertext.len());
    frame.extend_from_slice(&index.to_le_bytes());
    frame.extend_from_slice(ciphertext);
    frame
}

/// Decodes a chunk payload into index and ciphertext slice
pub fn decode_chunk_frame(payload: &[u8]) -> Result<(u32, &[u8])> {
    if payload.len() < 4 {
        bail!("Chunk frame too short: {} bytes (minimum 4)", payload.len());
    }
    let index = u32::from_le_bytes(payload[0..4].try_into().unwrap());
    let ciphertext = &payload[4..];
    Ok((index, ciphertext))
}

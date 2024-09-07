//! Forged Compression Engine (fCE)

use lz4_flex::block::{compress, decompress};
use std::io::ErrorKind::UnexpectedEof;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::BUFFER_SIZE;

pub async fn compress_stream<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let compressed = compress(&buffer[..bytes_read]);

        writer.write_u64_le(compressed.len() as u64).await?;
        writer.write_all(&compressed).await?;
    }

    Ok(())
}

pub async fn decompress_stream<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    // If unlucky, compressed data can be slightly larger than the original data
    // So we allocate a bigger buffer according to the worst case scenario (prepended size + header + data)
    const MAX_UNCOMPRESSED_SIZE: usize = 4 + 258 + BUFFER_SIZE;

    let mut buffer = [0u8; MAX_UNCOMPRESSED_SIZE];

    loop {
        let result = reader.read_u64_le().await;

        let size = usize::try_from(match result {
            Ok(x) => x,
            // Unexpected EOF means all data has been read
            Err(e) if e.kind() == UnexpectedEof => break,
            Err(e) => return Err(e),
        })
        .expect("Size is too big");

        reader.read_exact(&mut buffer[..size]).await?;

        let decompressed = decompress(&buffer[..size], MAX_UNCOMPRESSED_SIZE).map_err(|e| {
            log::error!("Decompression failed: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Decompression failed")
        })?;
        writer.write_all(&decompressed).await?;
    }

    Ok(())
}

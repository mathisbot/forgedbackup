//! Forged Compression Engine (fCE)

use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::BUFFER_SIZE;

pub async fn compress_data<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let compressed = compress_prepend_size(&buffer[..bytes_read]);

        let size = compressed.len() as u32;
        let size = &size.to_le_bytes();

        writer.write_all(size).await?;

        writer.write_all(&compressed).await?;
    }

    Ok(())
}

pub async fn decompress_data<R, W>(reader: &mut R, writer: &mut W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut size_buffer = [0u8; 4]; // 4 bytes to read the size prefix
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        if reader.read_exact(&mut size_buffer).await.is_err() {
            break;
        }
        let size = u32::from_le_bytes(size_buffer) as usize;

        // Read the compressed data
        // Size should be at most BUFFER_SIZE if the data was compressed using the above function
        reader.read_exact(&mut buffer[..size]).await?;

        // Decompress the data
        let decompressed = decompress_size_prepended(&buffer[..size]).map_err(|e| {
            log::error!("Decompression failed: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Decompression failed")
        })?;

        // Write the decompressed data
        writer.write_all(&decompressed).await?;
    }

    Ok(())
}

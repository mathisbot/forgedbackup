//! Forged Compression Engine (fCE)

use std::mem::size_of;

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

        let size = compressed.len();
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
    let mut size_buffer = [0u8; size_of::<usize>()]; // 4 bytes to read the size prefix
    // If unlucky, compressed data can be larger than the original data
    // So we allocate a vec instead of a fixed buffer
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);

    loop {
        if reader.read_exact(&mut size_buffer).await.is_err() {
            break;
        }
        let size = usize::from_le_bytes(size_buffer);

        buffer.resize(size, 0);
        reader.read_exact(&mut buffer).await?;
        
        let decompressed = decompress_size_prepended(&buffer).map_err(|e| {
            log::error!("Decompression failed: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Decompression failed")
        })?;
        writer.write_all(&decompressed).await?;

        // Quickly clear the buffer
        unsafe {
            buffer.set_len(0);
        }
    }

    Ok(())
}

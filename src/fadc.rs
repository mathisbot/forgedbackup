//! Forged Asynchronous Directory Crawler (fADC)

use std::io::ErrorKind::UnexpectedEof;
use std::path::PathBuf;
use tokio::fs::ReadDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use crate::BUFFER_SIZE;

async fn crawl_dir(mut directory: ReadDir, tx: &mut DuplexStream) -> Result<(), std::io::Error> {
    Box::pin(async move {
        while let Some(entry) = directory.next_entry().await? {
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                let path = entry.path();
                let path_bytes = path.as_os_str().as_encoded_bytes();

                tx.write_u64_le(path_bytes.len() as u64).await?;

                tx.write_all(&path_bytes).await?;

                let file_size = metadata.len();
                tx.write_u64_le(file_size).await?;

                let file = tokio::fs::File::open(&path).await?;
                let mut src = tokio::io::BufReader::new(file);

                log::debug!("Sending file: {:?}", path);

                let mut buf = vec![0; BUFFER_SIZE];
                loop {
                    let bytes_read = src.read(&mut buf).await?;
                    if bytes_read == 0 {
                        break;
                    }
                    tx.write_all(&buf[..bytes_read]).await?;
                }
            } else if metadata.is_dir() {
                let new_directory = tokio::fs::read_dir(entry.path()).await?;
                crawl_dir(new_directory, tx).await?;
            } else if metadata.is_symlink() {
                // TODO: Handle symlinks
                log::warn!("Symlink support is a work in progress: {:?}", entry.path());
                continue;
            } else {
                log::warn!("Skipping non-file/directory: {:?}", entry.path());
                continue;
            }
        }

        Ok(())
    })
    .await
}

pub async fn read_dir(dir_path: PathBuf, tx: &mut DuplexStream) -> Result<(), std::io::Error> {
    let directory = tokio::fs::read_dir(dir_path).await?;
    crawl_dir(directory, tx).await?;

    Ok(())
}

pub async fn write_dir(
    reader: &mut DuplexStream,
    output_path: PathBuf,
) -> Result<(), std::io::Error> {
    let mut buf = [0; BUFFER_SIZE];
    let mut file_path = [0u8; 2048]; // Filepath is at most 4096 chars

    loop {
        let result = reader.read_u64_le().await;
        let file_path_len = match result {
            Ok(x) => x,
            // Unexpected EOF means all data has been read
            Err(e) if e.kind() == UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        } as usize;

        reader.read_exact(&mut file_path[..file_path_len]).await?;
        let file_path = output_path.join(std::str::from_utf8(&file_path[..file_path_len]).unwrap());

        let file_size = reader.read_u64_le().await? as usize;

        tokio::fs::create_dir_all(file_path.parent().unwrap()).await?;
        let file = tokio::fs::File::create(&file_path).await?;
        let mut writer = tokio::io::BufWriter::new(file);

        let turn = file_size / BUFFER_SIZE;
        let remainder = file_size % BUFFER_SIZE;

        for _ in 0..turn {
            reader.read_exact(&mut buf).await?;
            writer.write_all(&buf).await?;
        }

        if remainder > 0 {
            reader.read_exact(&mut buf[..remainder]).await?;
            writer.write_all(&buf[..remainder]).await?;
        }

        writer.flush().await?;
    }

    Ok(())
}

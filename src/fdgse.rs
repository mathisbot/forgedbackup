//! Forged Data General Security Engine (fDGSE)

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use std::io::{
    Error,
    ErrorKind::{InvalidData, UnexpectedEof},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::BUFFER_SIZE;

pub type CipherKey = Key<Aes256Gcm>;

const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

pub fn generate_key() -> CipherKey {
    Aes256Gcm::generate_key(&mut OsRng)
}

pub fn read_key(key_path: &str) -> CipherKey {
    let key: &[u8; 32] = &std::fs::read(key_path)
        .expect("Could not read key file")
        .try_into()
        .expect("Invalid key file");
    *Key::<Aes256Gcm>::from_slice(key)
}

pub async fn cipher_stream<R, W>(
    reader: &mut R,
    writer: &mut W,
    key: &CipherKey,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; BUFFER_SIZE];
    let cipher = Aes256Gcm::new(key);

    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        writer.write_all(&nonce).await?;

        let cipher_text = cipher.encrypt(&nonce, &buffer[..bytes_read]).map_err(|e| {
            log::error!("Encryption failed: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Encryption failed")
        })?;

        writer.write_u64_le(cipher_text.len() as u64).await?;

        writer.write_all(&cipher_text).await?;
    }

    Ok(())
}

pub async fn decipher_stream<R, W>(
    reader: &mut R,
    writer: &mut W,
    key: CipherKey,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; BUFFER_SIZE + TAG_SIZE];
    let mut nonce = [0u8; NONCE_SIZE];
    let cipher = Aes256Gcm::new(&key);

    loop {
        let nonce = {
            let result = reader.read_exact(&mut nonce).await;
            match result {
                Ok(_) => (),
                // Unexpected EOF means all data has been read
                Err(e) if e.kind() == UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            nonce
        };

        let size = reader.read_u64_le().await? as usize;
        if size == 0 {
            break;
        }

        let bytes_read = reader.read_exact(&mut buffer[..size]).await?;
        if bytes_read == 0 {
            break;
        }

        let nonce = Nonce::from_slice(&nonce);

        let plain_text = cipher.decrypt(&nonce, &buffer[..size]).map_err(|e| {
            log::error!("Decryption failed: {}", e);
            Error::new(InvalidData, "Decryption failed")
        })?;

        writer.write_all(&plain_text).await?;
    }

    Ok(())
}

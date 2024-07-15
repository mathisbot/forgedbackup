//! Forged Data General Security Engine (fDGSE)

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use std::io::ErrorKind::UnexpectedEof;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::BUFFER_SIZE;

pub type CipherKey = Key<Aes256Gcm>;

const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

pub fn generate_key() -> Key<Aes256Gcm> {
    Aes256Gcm::generate_key(&mut OsRng)
}

pub fn read_key(key_path: &str) -> Key<Aes256Gcm> {
    let key: &[u8; 32] = &std::fs::read(key_path)
        .expect("Could not read key file")
        .try_into()
        .expect("Invalid key file");
    *Key::<Aes256Gcm>::from_slice(key)
}

pub async fn send_plaintext<R, W>(
    reader: &mut R,
    writer: &mut W,
    key: Key<Aes256Gcm>,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; BUFFER_SIZE];
    let cipher = Aes256Gcm::new(&key);

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

        // Send size
        writer.write_u64_le(cipher_text.len() as u64).await?;

        writer.write_all(&cipher_text).await?;
    }

    Ok(())
}

pub async fn receive_ciphertext<R, W>(
    reader: &mut R,
    writer: &mut W,
    key: Key<Aes256Gcm>,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; BUFFER_SIZE + TAG_SIZE];
    let cipher = Aes256Gcm::new(&key);
    loop {
        let nonce = {
            let mut nonce = [0u8; NONCE_SIZE];
            let result = reader.read_exact(&mut nonce).await;
            match result {
                Ok(_) => (),
                // Unexpected EOF means all data has been read
                Err(ref e) if e.kind() == UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            nonce
        };

        let size = reader.read_u64_le().await?;
        if size == 0 {
            break;
        }

        let bytes_read = reader.read_exact(&mut buffer[..size as usize]).await?;
        if bytes_read == 0 {
            break;
        }

        let nonce = Nonce::from_slice(&nonce);

        let plain_text = cipher.decrypt(&nonce, &buffer[..size as usize]).map_err(|e| {
            log::error!("Decryption failed: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Decryption failed")
        })?;

        writer.write_all(&plain_text).await?;
    }

    Ok(())
}

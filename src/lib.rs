#![deny(clippy::all)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod config;
pub mod fadc;
pub mod fce;
pub mod fdgse;
pub mod fsas;

use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};
use tokio::{fs::File, io::duplex, net::TcpStream};

// Buffer size doesn't seem to affect performances too much
// However, it is clear that it affects compression ratio
pub const BUFFER_SIZE: usize = 1 << 15; // 32 KiB
pub const DUPLEX_BUFFER_SIZE: usize = 1 << 15; // 32 KiB

pub enum Mode {
    // Operator mode
    Server,
    Client,

    // Admin mode
    Admin,
}

impl TryFrom<String> for Mode {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "s" | "server" => Ok(Self::Server),
            "c" | "client" => Ok(Self::Client),
            "a" | "admin" => Ok(Self::Admin),
            _ => Err("Invalid mode".to_string()),
        }
    }
}

pub enum SubMode {
    // Operator mode
    Init,
    Start,

    // Admin mode
    List,
    Decompress,
}

impl TryFrom<String> for SubMode {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "i" | "init" => Ok(Self::Init),
            "s" | "start" => Ok(Self::Start),
            "l" | "list" => Ok(Self::List),
            "dc" | "decompress" => Ok(Self::Decompress),
            _ => Err("Invalid submode".to_string()),
        }
    }
}

pub struct Client {
    pub hostname: String,
    pub info: config::ClientInfo,
}

pub async fn handle_client(
    client: Client,
    mut stream: TcpStream,
    backup_dir: PathBuf,
) -> std::io::Result<()> {
    fsas::send_and_verify_challenge(&mut stream, &client.info.keypair.verifying_key).await?;
    log::debug!("Client {} verified", client.hostname);

    fsas::receive_and_answer_challenge(&mut stream, &client.info.keypair.signing_key).await?;
    log::debug!("Authenticated to client {}", client.hostname);

    let dirname = format!("{}/{}", backup_dir.to_str().unwrap(), client.hostname);
    let filename = format!(
        "{}/{}.lz4",
        dirname,
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    tokio::fs::create_dir_all(dirname).await?;
    let mut file = File::create(filename).await?;
    log::trace!("Backfup file created for {}", client.hostname);

    let start = Instant::now();
    log::info!("Backup started for {}", client.hostname);

    let (mut tx, mut rx) = duplex(DUPLEX_BUFFER_SIZE);

    let cipher_handle = tokio::spawn(async move {
        Box::pin(fdgse::decipher_stream(
            &mut stream,
            &mut tx,
            client.info.cipher_key,
        ))
        .await
        .expect("Error deciphering data");
    });
    let compress_handle = tokio::spawn(async move {
        Box::pin(fce::compress_stream(&mut rx, &mut file))
            .await
            .expect("Error compressing data");
    });

    cipher_handle.await?;
    compress_handle.await?;

    let duration = start.elapsed();
    log::info!("Backup finished for {} in {:?}", client.hostname, duration);

    Ok(())
}

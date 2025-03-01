//! # ForgedBackup
//!
//! ForgedBackup is a tool written in Rust for creating and automating fast, secure backups.
//!
//! The architecture pays particular attention to optimization and safety.
//!
//! Let's call "client" the server that wants to be backed up and "server" the server that actually hosts backups.
//!
//! It's up to the client to decide when to initiate a backup.
//! When it does:
//! 1. It authenticates to the server, and the server authenticates itself to the client.
//!    This is done so that your private data cannot be sent anywhere else than on the chosen servers, and that you don't accept data incoming from unknown servers.
//! 2. An encrypted pipe is opened, making sure that no one can eavesdrop on your private data.
//! 3. The client sends the files to be backed up to the server
//! 4. The server compresseses them on the fly

use std::{io, path::PathBuf};

use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use forgedbackup::config::ClientInfo;
use forgedbackup::{config, fadc, fce, fdgse, fsas, Client};
use forgedbackup::{Mode, SubMode};

async fn start_server(config: &config::ServerConfig) -> io::Result<()> {
    let listener = TcpListener::bind(config.listening_socker_addr).await?;
    log::info!("Server listening on {}", config.listening_socker_addr);

    loop {
        let (mut stream, peer_addr) = listener.accept().await?;
        log::debug!("Incoming connexion from {}", peer_addr);

        let mut hostname = [0u8; 256];
        let amount_read = stream.read(&mut hostname).await?;
        let hostname = String::from_utf8(hostname[..amount_read].to_vec()).unwrap();
        let hostname = hostname.trim_matches(char::from(0));
        log::trace!("Received hostname: {}", hostname);

        let client_info = config.client_infos.get(hostname).expect("Client not found");
        log::trace!("Client found: {}", hostname);

        let signing_key = client_info.keypair.signing_key.clone();
        let verifying_key = client_info.keypair.verifying_key;
        let cipher_key = client_info.cipher_key;
        let hostname = hostname.to_string();
        let backup_dir = config.backup_dir.clone();

        tokio::spawn(async move {
            if let Err(e) = {
                log::trace!("Handling client {}", hostname);
                forgedbackup::handle_client(
                    Client {
                        hostname,
                        info: ClientInfo {
                            keypair: fsas::KeyPair {
                                signing_key,
                                verifying_key,
                            },
                            cipher_key,
                        },
                    },
                    stream,
                    backup_dir,
                )
                .await
            } {
                log::error!("Error handling client: {}", e);
            }
        });
    }
}

async fn start_client(config: &config::ClientConfig) -> io::Result<()> {
    let mut backup_made = false;

    for server_info in config.servers.clone() {
        let result: io::Result<()> = {
            let mut stream = TcpStream::connect(server_info.addr).await?;
            log::debug!("Connected to server {}.", server_info.hostname);

            stream.write_all(config.hostname.as_bytes()).await?;
            log::trace!("Hostname sent: {}", config.hostname);

            fsas::receive_and_answer_challenge(&mut stream, &server_info.keypair.signing_key)
                .await?;
            log::debug!("Authenticated to server {}", server_info.hostname);

            fsas::send_and_verify_challenge(&mut stream, &server_info.keypair.verifying_key)
                .await?;
            log::debug!("Server {} verified", server_info.hostname);

            let start = std::time::Instant::now();
            log::info!("Starting backup on server {}", server_info.hostname);

            let dir_path = config.backed_up_dir.clone();
            let (mut tx, mut rx) = duplex(forgedbackup::DUPLEX_BUFFER_SIZE);

            let dir_handle = tokio::spawn(async move {
                fadc::read_dir(dir_path, &mut tx).await.unwrap();
            });
            let cipher_handle = tokio::spawn(async move {
                fdgse::cipher_stream(&mut rx, &mut stream, &server_info.cipher_key)
                    .await
                    .unwrap();
            });

            dir_handle.await?;
            cipher_handle.await?;

            let duration = start.elapsed();
            log::info!(
                "Backup on server {} finished in {:?}",
                server_info.hostname,
                duration
            );

            backup_made = true;

            Ok(())
        };

        if let Err(e) = result {
            log::error!(
                "Error while attempting to backup on {}: {}",
                server_info.hostname,
                e
            );
        }
    }

    if !backup_made {
        panic!("No backup made!");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let args = std::env::args().collect::<Vec<String>>();

    if args.len() < 3 {
        panic!("Usage: {} <server|client|admin> <init|start>", args[0]);
    }

    let mode = Mode::try_from(args[1].clone()).expect("Invalid mode");
    let submode = SubMode::try_from(args[2].clone()).expect("Invalid submode");

    match mode {
        Mode::Server => match submode {
            SubMode::Init => {
                let fsas::KeyPair {
                    signing_key,
                    verifying_key,
                } = fsas::generate_keypair();

                let dest_dir = if args.len() >= 3 {
                    args[2].clone()
                } else {
                    "./".to_string()
                };
                let dest_dir = std::path::Path::new(&dest_dir);
                tokio::fs::create_dir_all(dest_dir).await?;

                let signing_key_path = dest_dir.join("ed25519");
                let verifying_key_path = dest_dir.join("ed25519.pub");
                tokio::fs::write(signing_key_path, signing_key.to_bytes()).await?;
                tokio::fs::write(verifying_key_path, verifying_key.to_bytes()).await?;
            }
            SubMode::Start => {
                let server_config = config::ServerConfig::read("config.toml");
                start_server(&server_config).await?;
            }
            _ => panic!("Invalid submode for operator mode."),
        },
        Mode::Client => match submode {
            SubMode::Init => {
                let fsas::KeyPair {
                    signing_key,
                    verifying_key,
                } = fsas::generate_keypair();

                let dest_dir = if args.len() == 3 {
                    args[2].clone()
                } else {
                    "./".to_string()
                };
                let dest_dir = std::path::Path::new(&dest_dir);
                tokio::fs::create_dir_all(dest_dir).await?;

                let signing_key_path = dest_dir.join("ed25519");
                let verifying_key_path = dest_dir.join("ed25519.pub");
                tokio::fs::write(signing_key_path, signing_key.to_bytes()).await?;
                tokio::fs::write(verifying_key_path, verifying_key.to_bytes()).await?;

                let key = fdgse::generate_key();
                let key_path = dest_dir.join("key.aes");
                tokio::fs::write(key_path, key).await?;

                log::info!(
                    "Keys successfully generated in directory {}",
                    dest_dir.display()
                );
            }
            SubMode::Start => {
                let client_config = config::ClientConfig::read("config.toml");
                start_client(&client_config).await?;
            }
            _ => panic!("Invalid submode for operator mode."),
        },
        Mode::Admin => match submode {
            SubMode::List => {
                let server_config = config::ServerConfig::read("config.toml");
                let mut backup_dir = tokio::fs::read_dir(server_config.backup_dir).await?;

                while let Some(server) = backup_dir.next_entry().await? {
                    let filename = server.file_name();
                    let filename = filename.to_str().unwrap();
                    println!("Backups for {}:", filename);
                    let backups = std::fs::read_dir(server.path())?;
                    for (i, backup) in backups.enumerate() {
                        let backup = backup?;
                        let metadata = backup.metadata()?;
                        let size = metadata.len();
                        let last_modified = metadata.modified()?.elapsed().unwrap();

                        let pretty_time = {
                            let minutes = last_modified.as_secs() / 60;
                            let hours = minutes / 60;
                            let days = hours / 24;

                            if days > 0 {
                                format!("{} days", days)
                            } else if hours > 0 {
                                format!("{} hours", hours)
                            } else {
                                format!("{} minutes", minutes)
                            }
                        };

                        println!("  [{}] {} ago, {} B", i, pretty_time, size);
                    }
                }
            }
            SubMode::Decompress => {
                let server_config = config::ServerConfig::read("config.toml");

                if args.len() < 5 {
                    panic!(
                        "Usage: {} admin decompress <server> <backup-number> [dest-dir]",
                        args[0]
                    );
                }

                let server = args[3].clone();
                let backup_dir = server_config.backup_dir.join(server);

                let mut backup_number = args[4].parse::<usize>().expect("Invalid backup number");

                let mut backups = tokio::fs::read_dir(&backup_dir).await.unwrap();

                let mut backup = None;
                while let Some(entry) = backups.next_entry().await? {
                    if backup_number == 0 {
                        backup = Some(entry);
                        break;
                    }
                    backup_number -= 1;
                }

                let backup = tokio::fs::read(backup.expect("Backup not found").path()).await?;

                let output_dir = PathBuf::from(if args.len() == 6 {
                    args[5].clone()
                } else {
                    "./decompressed".to_string()
                });

                let (mut tx, mut rx) = tokio::io::duplex(forgedbackup::DUPLEX_BUFFER_SIZE);

                let decompress_handle = tokio::spawn(async move {
                    fce::decompress_stream(&mut backup.as_slice(), &mut tx)
                        .await
                        .unwrap();
                });
                let dir_handle = tokio::spawn(async move {
                    fadc::write_dir(&mut rx, output_dir).await.unwrap();
                });

                decompress_handle.await?;
                dir_handle.await?;
            }
            _ => panic!("Invalid submode for admin mode."),
        },
    };

    Ok(())
}

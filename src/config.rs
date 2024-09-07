use core::net::SocketAddr;
use std::{collections::HashMap, path::PathBuf};
use toml::Table;

use crate::fdgse::CipherKey;
use crate::fsas::KeyPair;

pub type Hostname = String;

#[derive(Clone)]
pub struct ServerInfo {
    pub hostname: Hostname,
    pub addr: SocketAddr,
    pub keypair: KeyPair,
    pub cipher_key: CipherKey,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct ClientConfig {
    pub servers: Vec<ServerInfo>,
    pub hostname: Hostname,
    pub backed_up_dir: PathBuf,
}

#[derive(Clone)]
pub struct ClientInfo {
    pub keypair: KeyPair,
    pub cipher_key: CipherKey,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone)]
pub struct ServerConfig {
    pub listening_socker_addr: SocketAddr,
    pub client_infos: HashMap<Hostname, ClientInfo>,
    pub backup_dir: PathBuf,
}

impl ClientConfig {
    #[must_use]
    // ## Panics
    // This function panics if the configuration file is not found,
    // or if the configuration file is not in the correct format.
    pub fn read(file_path: &str) -> Self {
        let config = std::fs::read_to_string(file_path)
            .expect("Could not read configuration file")
            .parse::<Table>()
            .expect("Could not parse configuration file");

        let signing_keys_dir = config["signing_keys_dir"]
            .as_str()
            .expect("Missing signing_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse signing_keys_dir in configuration file");

        let verifying_keys_dir = config["verifying_keys_dir"]
            .as_str()
            .expect("Missing verifying_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse verifying_keys_dir in configuration file");

        let cipher_keys_dir = config["cipher_keys_dir"]
            .as_str()
            .expect("Missing cipher_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse cipher_keys_dir in configuration file");

        let backed_up_dir = config["backed_up_dir"]
            .as_str()
            .expect("Missing backed_up_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse backed_up_dir in configuration file");

        let servers = config["servers"]
            .as_table()
            .expect("Missing servers entry in configuration file")
            .iter()
            .map(|(name, addr)| {
                let addr = addr
                    .as_str()
                    .unwrap()
                    .parse::<SocketAddr>()
                    .expect("Could not parse server socket address in configuration file");
                ServerInfo {
                    hostname: name.clone(),
                    addr,
                    keypair: KeyPair {
                        signing_key: {
                            let signing_key_path =
                                format!("{}/{}", signing_keys_dir.to_str().unwrap(), name);
                            crate::fsas::read_signing_key(&signing_key_path).unwrap()
                        },
                        verifying_key: {
                            let verifying_key_path =
                                format!("{}/{}.pub", verifying_keys_dir.to_str().unwrap(), name);
                            crate::fsas::read_verifying_key(&verifying_key_path).unwrap()
                        },
                    },
                    cipher_key: {
                        let cipher_key_path =
                            format!("{}/{}.aes", cipher_keys_dir.to_str().unwrap(), name);
                        crate::fdgse::read_key(&cipher_key_path)
                    },
                }
            })
            .collect();

        let hostname = config["hostname"]
            .as_str()
            .expect("Missing hostname in configuration file")
            .to_string();

        Self {
            servers,
            hostname,
            backed_up_dir,
        }
    }
}

impl ServerConfig {
    #[must_use]
    // ## Panics
    // This function panics if the configuration file is not found,
    // or if the configuration file is not in the correct format.
    pub fn read(file_path: &str) -> Self {
        let config = std::fs::read_to_string(file_path)
            .expect("Could not read configuration file")
            .parse::<Table>()
            .expect("Could not parse configuration file");

        let listening_socket_addr = config["listening_on"]
            .as_str()
            .expect("Missing listening_socker_addr in configuration file")
            .parse()
            .expect("Could not parse listening_socker_addr in configuration file");

        let signing_keys_dir = config["signing_keys_dir"]
            .as_str()
            .expect("Missing signing_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse signing_keys_dir in configuration file");

        let verifying_keys_dir = config["verifying_keys_dir"]
            .as_str()
            .expect("Missing verifying_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse verifying_keys_dir in configuration file");

        let cipher_keys_dir = config["cipher_keys_dir"]
            .as_str()
            .expect("Missing cipher_keys_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse cipher_keys_dir in configuration file");

        let backup_dir = config["backup_dir"]
            .as_str()
            .expect("Missing backup_dir in configuration file")
            .parse::<PathBuf>()
            .expect("Could not parse backup_dir in configuration file");

        let mut client_infos = HashMap::new();

        for entry in
            std::fs::read_dir(&signing_keys_dir).expect("Could not read signing keys directory")
        {
            let entry = entry.expect("Could not read entry in signing keys directory");
            let path = entry.path();
            let hostname = path
                .file_stem()
                .expect("Could not get file stem")
                .to_str()
                .expect("Could not convert file stem to string")
                .to_string();
            let signing_key = crate::fsas::read_signing_key(
                path.to_str().expect("Could not convert path to string"),
            )
            .unwrap();
            let verifying_key = {
                let verifying_key_path = format!(
                    "{}/{}.pub",
                    verifying_keys_dir
                        .to_str()
                        .expect("Could not convert verifying keys directory to string"),
                    hostname
                );
                crate::fsas::read_verifying_key(&verifying_key_path)
                    .expect("Could not read verifying key")
            };
            let cipher_key = {
                let cipher_key_path = format!(
                    "{}/{}.aes",
                    cipher_keys_dir
                        .to_str()
                        .expect("Could not convert verifying keys directory to string"),
                    hostname
                );
                crate::fdgse::read_key(&cipher_key_path)
            };
            client_infos.insert(
                hostname,
                ClientInfo {
                    keypair: KeyPair {
                        signing_key,
                        verifying_key,
                    },
                    cipher_key,
                },
            );
        }

        Self {
            listening_socker_addr: listening_socket_addr,
            client_infos,
            backup_dir,
        }
    }
}

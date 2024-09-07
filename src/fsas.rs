//! Forged Server Authentication System (fSAS)

use ed25519_dalek::{Signature, Signer, Verifier};
pub use ed25519_dalek::{SigningKey, VerifyingKey};
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SIGNATURE_LENGTH};
use rand::{rngs::OsRng, RngCore};
use std::{fs, io};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHALLENGE_LENGTH: usize = 512;

#[derive(Clone)]
pub struct KeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

#[must_use]
pub fn generate_keypair() -> KeyPair {
    let mut csprng = OsRng {};
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = VerifyingKey::from(&signing_key);
    assert!(
        verifying_key.is_weak(),
        "The generated keypair is weak. Please regenerate the keypair."
    );
    KeyPair {
        signing_key,
        verifying_key,
    }
}

pub fn read_signing_key(bytes_file: &str) -> io::Result<SigningKey> {
    let signing_key = fs::read(bytes_file)?;
    assert_eq!(
        signing_key.len(),
        SECRET_KEY_LENGTH,
        "Invalid signing key length"
    );
    let signing_key: [u8; SECRET_KEY_LENGTH] = signing_key.try_into().unwrap();
    Ok(SigningKey::from_bytes(&signing_key))
}

pub fn read_verifying_key(bytes_file: &str) -> Result<VerifyingKey, io::Error> {
    let verifying_key = fs::read(bytes_file).expect("Failed to read verifying key file");
    assert_eq!(
        verifying_key.len(),
        PUBLIC_KEY_LENGTH,
        "Invalid verifying key length"
    );
    let verifying_key: [u8; PUBLIC_KEY_LENGTH] = verifying_key.try_into().unwrap();
    VerifyingKey::from_bytes(&verifying_key)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to import verifying key"))
}

fn verify_signature(
    verifying_key: &VerifyingKey,
    signature: &Signature,
    message: &[u8],
) -> io::Result<()> {
    verifying_key
        .verify(message, signature)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to authenticate the client"))
}

pub async fn send_and_verify_challenge(
    stream: &mut tokio::net::TcpStream,
    verifying_key: &VerifyingKey,
) -> io::Result<()> {
    let mut challenge = [0u8; CHALLENGE_LENGTH];
    OsRng {}.fill_bytes(&mut challenge[..]);
    let mut signature = [0u8; SIGNATURE_LENGTH];

    stream.write_all(&challenge).await?;

    stream.read_exact(&mut signature).await?;

    verify_signature(
        verifying_key,
        &Signature::from_bytes(&signature),
        &challenge,
    )
}

pub async fn receive_and_answer_challenge(
    stream: &mut tokio::net::TcpStream,
    signing_key: &SigningKey,
) -> Result<(), std::io::Error> {
    let mut challenge = [0u8; CHALLENGE_LENGTH];
    stream.read_exact(&mut challenge).await?;

    let signature = signing_key.sign(&challenge);

    stream.write_all(&signature.to_bytes()).await?;

    Ok(())
}

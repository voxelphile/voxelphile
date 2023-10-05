use std::net::SocketAddr;

use serde_derive::{Deserialize, Serialize};

pub struct User {}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserCredentialDetails {
    Username { username: String },
    Email { email: String },
}

#[derive(Serialize, Deserialize)]
pub struct UserCredentials {
    pub password: String,
    pub details: UserCredentialDetails,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserRegistrationDetails {
    Voxelphile { password: String, email: String },
    Steam,
}

#[derive(Serialize, Deserialize)]
pub struct UserRegistration {
    pub username: String,
    pub details: UserRegistrationDetails,
}

pub const SESSION_BYTES: usize = 64;

#[derive(Serialize, Deserialize)]
pub struct UserConnection {
    #[serde(with = "serde_bytes")]
    pub session: Vec<u8>,
    pub ip: SocketAddr,
}

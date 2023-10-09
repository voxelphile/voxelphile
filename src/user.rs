use std::net::SocketAddr;

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct UserCredentials {
    pub password: String,
    pub email: String,
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

#[derive(Serialize, Deserialize)]
pub struct UserChange {
    pub profile: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

pub const SESSION_BYTES: usize = 64;

#[derive(Serialize, Deserialize)]
pub struct UserConnection {
    #[serde(with = "serde_bytes")]
    pub session: Vec<u8>,
    pub ip: SocketAddr,
}

#[derive(Clone)]
pub struct User {
    pub id: uuid::Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct UserClaims {
    pub id: String,
    pub exp: usize,
}

#[derive(Serialize, Deserialize)]
pub struct UserDetails {
    pub username: String,
    pub email: String,
    pub profile: String,
}

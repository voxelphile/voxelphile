use std::collections::BTreeMap;
use std::io::Cursor;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, iter, mem};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{Salt, SaltString};
use argon2::{Argon2, PasswordHasher};
use async_trait::async_trait;
use base64::Engine;
use tokio_util::codec::{FramedRead, BytesCodec};
// use common::log::{error, Log};
// use common::rand::{self, thread_rng, Rng};
use crate::sol::*;
use crate::user::{
     UserCredentials, UserRegistration, UserRegistrationDetails, UserChange, User,
};
use hmac::Mac;
use http::StatusCode;
use jwt::SignWithKey;
use lazy_static::*;
use serde_derive::*;
use tokio_postgres::error::SqlState;

pub const SALT_LEN: usize = 16;
pub const MAX_USERNAME_LEN: usize = 32;
pub const MAX_PASSWORD_LEN: usize = 128;
pub const MAX_EMAIL_LEN: usize = 128;
pub type Postgres = std::sync::Arc<tokio::sync::Mutex<tokio_postgres::Client>>;
pub type Token = String;

#[derive(Serialize, Deserialize)]
pub enum UserLoginError {
    DbError,
    BadHash,
    NotFound,
    BadPassword,
}

impl UserLoginError {
    pub fn status_code(&self) -> StatusCode {
        use UserLoginError::*;
        match self {
            DbError | BadHash => StatusCode::INTERNAL_SERVER_ERROR,
            NotFound => StatusCode::NOT_FOUND,
            BadPassword => StatusCode::FORBIDDEN,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum UserRegistrationError {
    DbError,
    BadEmail,
    BadPassword,
    BadUsername,
    NotImplemented,
    Duplicate,
}

impl UserRegistrationError {
    pub fn status_code(&self) -> StatusCode {
        use UserRegistrationError::*;
        match self {
            DbError => StatusCode::INTERNAL_SERVER_ERROR,
            BadEmail | BadPassword | BadUsername | Duplicate => StatusCode::UNPROCESSABLE_ENTITY,
            NotImplemented => StatusCode::NOT_IMPLEMENTED,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum UserChangeError {
    DbError,
    ServerError,
    BadEmail,
    BadUsername,
    BadPassword,
    BadProfile,
    Duplicate
}


impl UserChangeError {
    pub fn status_code(&self) -> StatusCode {
        use UserChangeError::*;
        match self {
            DbError | ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            BadEmail | BadPassword | BadUsername | Duplicate | BadProfile => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
#[async_trait]
pub trait Strategy {
    async fn create_db_client_connection() -> Result<Postgres, tokio_postgres::error::Error>;

    async fn login_user(
        postgres: &Postgres,
        credentials: &UserCredentials,
    ) -> Result<Token, UserLoginError>;

    async fn register_user(
        postgres: &Postgres,
        registration: &UserRegistration,
    ) -> Result<Token, UserRegistrationError>;

    async fn change_user(
        postgres: &Postgres,
        registration: &UserChange,
        user: User,
    ) -> Result<(), UserChangeError>;

    async fn retrieve_sol() -> Result<SolAddress, SolError>;
}

pub type Infra = Mockup;

pub struct Mockup;

fn check_password(password: &str) -> bool {
    password.len() <= MAX_PASSWORD_LEN
}

fn check_email(email: &str) -> bool {
    email.len() <= MAX_EMAIL_LEN
}

fn check_username(username: &str) -> bool {
    username.chars().all(|x| x.is_alphanumeric())
        && username.len() <= MAX_USERNAME_LEN
}

async fn hash_password(password: String) -> Option<String> {
    let salt_string = {
        let mut data = [0u8; SALT_LEN];
        let step = mem::size_of::<u128>() / mem::size_of::<u8>();
        for i in (0..SALT_LEN).step_by(step) {
            let micros = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros();
            for j in 0..step {
                data[i + j] = ((micros & u8::MAX as u128) >> 8 * j) as u8;
            }
            tokio::time::sleep(Duration::from_micros(5)).await;
        }
        let mut string = String::new();
        base64::engine::general_purpose::STANDARD.encode_string(&data, &mut string);
        string.pop();
        string.pop();
        string
    };

    let password_hash_string = {
        let salt_string = SaltString::from_b64(&salt_string).ok()?;
        let salt = Salt::from(&salt_string);
        Argon2::default()
            .hash_password(password.as_bytes(), salt)
            .ok()?
            .to_string()
    };

    Some(password_hash_string)
}

#[async_trait]
impl Strategy for Mockup {
    async fn create_db_client_connection() -> Result<Postgres, tokio_postgres::error::Error> {
        let (client, connection) = tokio_postgres::connect(
            &format!(
                "host={} user=voxelphile port={} password={}",
                env::var("VOXELPHILE_POSTGRES_HOST").unwrap(),
                env::var("VOXELPHILE_POSTGRES_PORT").unwrap(),
                env::var("VOXELPHILE_POSTGRES_PASSWORD").unwrap()
            ),
            tokio_postgres::NoTls,
        )
        .await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                //error!("connection error: {}", e);
            }
        });

        Ok(std::sync::Arc::new(tokio::sync::Mutex::new(client)))
    }

    async fn login_user(
        postgres: &Postgres,
        credentials: &UserCredentials,
    ) -> Result<Token, UserLoginError> {
        let postgres = postgres.lock().await;

        use UserLoginError::*;
        let (query, params) =  {
            let email = &credentials.email;
            let query = "select xenotech.users.id, xenotech.user_password_logins.password_hash from xenotech.users
            left join xenotech.user_password_logins on xenotech.users.id = xenotech.user_password_logins.id where email = $1 limit 1;";
            let params = [email as &(dyn tokio_postgres::types::ToSql + Sync)];
            (query, params)
        };

        let Ok(statement) = postgres.prepare(query).await.map_err(|e| dbg!(e)) else {
            Err(DbError)?
        };

        let Ok(row) = postgres.query_one(&statement, &params).await.map_err(|e| dbg!(e))  else {
            Err(NotFound)?
        };

        let id_string = row.get::<_, String>(0);
        let password_hash_string = row.get::<_, String>(1);

        let Ok(password_hash) = PasswordHash::new(&password_hash_string) else {
            Err(BadHash)?
        };

        use argon2::*;

        let Ok(_) = password_hash.verify_password(&[&argon2::Argon2::default()], credentials.password.clone()) else {
            Err(BadPassword)?
        };

        let key: hmac::Hmac<sha2::Sha256> =
            hmac::Hmac::new_from_slice(&env::var("VOXELPHILE_JWT_SECRET").unwrap().as_bytes())
                .unwrap();
        let mut claims = std::collections::HashMap::new();
        claims.insert("id", id_string);

        let token_string = claims.sign_with_key(&key).unwrap();

        Ok(token_string)
    }

    async fn register_user(
        postgres: &Postgres,
        registration: &UserRegistration,
    ) -> Result<Token, UserRegistrationError> {
        let mut postgres = postgres.lock().await;

        use UserRegistrationError::*;

        check_username(&registration.username).then_some(()).ok_or(BadUsername)?;

        enum UserParamData {
            Voxelphile {
                email: String,
                password_hash_string: String,
            },
        };

        let id = uuid::Uuid::new_v4().to_string();

        let param_data = match &registration.details {
            UserRegistrationDetails::Voxelphile { password, email } => {
                check_email(&email).then_some(()).ok_or(BadEmail)?;
                check_password(&password).then_some(()).ok_or(BadPassword)?;

                let password_hash_string = hash_password(password.clone()).await.ok_or(BadPassword)?;

                let param_data = UserParamData::Voxelphile {
                    email: email.clone(),
                    password_hash_string: password_hash_string,
                };

                param_data
            }
            UserRegistrationDetails::Steam => Err(NotImplemented)?,
        };

        let transaction = postgres.transaction().await.map_err(|_| DbError)?;

        match param_data {
            UserParamData::Voxelphile {
                email,
                password_hash_string,
            } => {
                {
                    let statement =
                        "insert into xenotech.users (id, username, email) values ($1, $2, $3);";

                    let email_dyn_ref = &email as &(dyn tokio_postgres::types::ToSql + Sync);

                    let mut params = vec![];

                    params.push(&id as &(dyn tokio_postgres::types::ToSql + Sync));

                    params
                        .push(&registration.username as &(dyn tokio_postgres::types::ToSql + Sync));

                    params.push(email_dyn_ref);

                    transaction.execute(statement, &params).await.map_err(|e| {
                        if let Some(&SqlState::UNIQUE_VIOLATION) = e.code() {
                            Duplicate
                        } else {
                            DbError
                        }
                    })?;
                }
                {
                    let statement = "insert into xenotech.user_password_logins (id, password_hash) values ($1, $2);";

                    let password_hash_dyn_ref =
                        &password_hash_string as &(dyn tokio_postgres::types::ToSql + Sync);

                    let mut params = vec![];

                    params.push(&id as &(dyn tokio_postgres::types::ToSql + Sync));

                    params.push(password_hash_dyn_ref);

                    let Ok(_) = transaction.execute(statement, &params).await else {
                        Err(DbError)?
                    };
                }
            }
        }

        transaction.commit().await.map_err(|e| {
            dbg!(e);
            DbError
        })?;

        let key: hmac::Hmac<sha2::Sha256> =
            hmac::Hmac::new_from_slice(&env::var("VOXELPHILE_JWT_SECRET").unwrap().as_bytes())
                .unwrap();
        let mut claims = std::collections::HashMap::new();
        claims.insert("id", id.to_string());

        let token_string = claims.sign_with_key(&key).unwrap();

        Ok(token_string)
    }

    async fn change_user(
        postgres: &Postgres,
        change: &UserChange,
        user: User,
    ) -> Result<(), UserChangeError> {
        use UserChangeError::*;

        let mut postgres = postgres.lock().await;
        let transaction = postgres.transaction().await.map_err(|_| DbError)?;

        let id = user.id;

        let mut profile_data = vec![];

        if let Some(profile) = &change.profile {
            profile_data = image_base64::from_base64(profile.clone());
        }

        if let Some(_) = &change.profile {
            let _ = image::io::Reader::with_format(Cursor::new(profile_data.clone()), image::ImageFormat::Jpeg).decode().map_err(|_| BadProfile)?;

            let google_cloud_storage_oauth2_token = env::var("GOOGLE_CLOUD_OAUTH2_TOKEN").unwrap();

            let profile_id = uuid::Uuid::new_v4();
            
            let client = reqwest::Client::new();

            let url = format!("https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}{}", "voxelphile-data", "user%2Fprofile%2F", profile_id.to_string());

            client.post(&url)
                .body(profile_data)
                .header("Authorization", "Bearer ".to_owned() + &google_cloud_storage_oauth2_token)
                .header("Content-Type", "image/jpeg")
                .send()
                .await.map_err(|_| ServerError)?;

            
            let statement = "update xenotech.users set profile_id = '$1' where id = '$2';";

            let params = [&profile_id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync), &id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync)];

            transaction.execute(statement, &params).await.map_err(|_| 
                    DbError
            )?;
        }

        let mut password_hash = None;
        
        if let Some(password) = change.password.clone() {
            check_password(&password).then_some(()).ok_or(BadPassword)?;

            password_hash = Some(hash_password(password).await.ok_or(BadPassword)?);
        }

        if let Some(password_hash) = &password_hash {
            let statement = "update xenotech.user_password_logins set password = '$1' where id = '$2';";

            let params = [&password_hash as &(dyn tokio_postgres::types::ToSql + Sync), &id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync)];

            transaction.execute(statement, &params).await.map_err(|_| 
                    DbError
            )?;
        }

        let mut params = vec![];
        let mut updates = Vec::<String>::new();
        let mut prepared = Vec::<String>::new();

        if let Some(username) = &change.username {
            check_username(username).then_some(()).ok_or(BadUsername)?;

            updates.push("username".to_owned());
            prepared.push(format!("${}", updates.len()));

            
            params.push(username as &(dyn tokio_postgres::types::ToSql + Sync));
        }
        
        if let Some(email) = &change.email {
            check_email(email).then_some(()).ok_or(BadEmail)?;
            
            updates.push("email".to_owned());
            prepared.push(format!("${}", updates.len()));

            
            params.push(email as &(dyn tokio_postgres::types::ToSql + Sync));
        }

        if updates.len() > 0 {
            prepared.push(id.to_string());

            let statement =
            format!("update xenotech.users set ({}) = ({}) where id = '{}';", updates.concat(), prepared.concat(), format!("${}", prepared.len()));
 
            transaction.execute(&statement, &params).await.map_err(|e| {
                if let Some(&SqlState::UNIQUE_VIOLATION) = e.code() {
                    Duplicate
                } else {
                    DbError
                }
            })?;
        }

        Ok(())
    }

    async fn retrieve_sol() -> Result<SolAddress, SolError> {
        todo!()
    }
}

use std::collections::BTreeMap;
use std::io::Cursor;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, iter, mem};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{Salt, SaltString};
use argon2::{Argon2, PasswordHasher};
use async_trait::async_trait;
use base64::Engine;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rustc_serialize::base64::FromBase64;
use tokio_util::codec::{BytesCodec, FramedRead};
// use common::log::{error, Log};
// use common::rand::{self, thread_rng, Rng};
use crate::sol::*;
use crate::user::{
    User, UserChange, UserClaims, UserCredentials, UserDetails, UserRegistration,
    UserRegistrationDetails,
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
    Duplicate,
}

impl UserChangeError {
    pub fn status_code(&self) -> StatusCode {
        use UserChangeError::*;
        match self {
            DbError | ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            BadEmail | BadPassword | BadUsername | Duplicate | BadProfile => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum UserGetError {
    DbError,
    ServerError,
    NotFound,
}

impl UserGetError {
    pub fn status_code(&self) -> StatusCode {
        use UserGetError::*;
        match self {
            DbError | ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            NotFound => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}

#[async_trait]
pub trait Strategy {
    async fn create_db_client_connection() -> Result<Postgres, DbError>;

    async fn get_db_ip() -> Result<String, GoogleApiError>;

    async fn get_access_token() -> Result<String, GoogleApiError>;

    async fn remove(path: &str) -> Result<(), ()>;

    async fn upload_bytes(data: Vec<u8>, path: &str) -> Result<(), ()>;

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

    async fn get_user(postgres: &Postgres, user: User) -> Result<UserDetails, UserGetError>;

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
    username.chars().all(|x| x.is_alphanumeric()) && username.len() <= MAX_USERNAME_LEN
}

async fn hash_password(password: String) -> Option<String> {
    let salt_string = {
        let mut data = [0u8; SALT_LEN];
        let step = mem::size_of::<u128>() / mem::size_of::<u8>();
        for i in (0..SALT_LEN).step_by(step) {
            let micros = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros();
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

#[derive(Serialize, Deserialize)]
struct GoogleAccessTokenClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: usize,
    iat: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleApiIdToken {
    id_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleApiAccessTokenResp {
    access_token: String,
    expires_in: usize,
    token_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleCloudGetInstance {
    #[serde(rename = "networkInterfaces")]
    network_interfaces: Vec<GoogleCloudNetworkInterface>,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleCloudNetworkInterface {
    #[serde(rename = "accessConfigs")]
    access_configs: Vec<GoogleCloudAccessConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleCloudAccessConfig {
    #[serde(rename = "natIP")]
    nat_ip: String,
}

#[derive(Debug)]
pub enum DbError {
    Postgres(tokio_postgres::error::Error),
    CouldNotFetchIp,
}

#[derive(Debug)]
pub enum GoogleApiError {
    ServerError,
}

#[async_trait]
impl Strategy for Mockup {
    async fn create_db_client_connection() -> Result<Postgres, DbError> {
        let (client, connection) = tokio_postgres::connect(
            &format!(
                "host={} user=voxelphile port={} password={}",
                Self::get_db_ip()
                    .await
                    .map_err(|_| DbError::CouldNotFetchIp)?,
                env::var("VOXELPHILE_POSTGRES_PORT").unwrap(),
                env::var("VOXELPHILE_POSTGRES_PASSWORD").unwrap()
            ),
            tokio_postgres::NoTls,
        )
        .await
        .map_err(|e| DbError::Postgres(e))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                //error!("connection error: {}", e);
            }
        });

        Ok(std::sync::Arc::new(tokio::sync::Mutex::new(client)))
    }

    async fn get_db_ip() -> Result<String, GoogleApiError> {
        Ok(env::var("VOXELPHILE_POSTGRES_HOST").unwrap())
    }

    async fn remove(path: &str) -> Result<(), ()> {
        let client = reqwest::Client::new();
        let path = path.clone().replace("/", "%2F");
        dbg!(
            client
                .delete(&format!(
                    "https://storage.googleapis.com/storage/v1/b/{}/o/{}",
                    "voxelphile", path
                ))
                .bearer_auth(match Self::get_access_token().await {
                    Ok(x) => x,
                    Err(e) => {
                        dbg!(e);
                        Err(())?
                    }
                })
                .send()
                .await
        )
        .map(|_| ())
        .map_err(|e| {
            dbg!(e);
            ()
        })
    }

    async fn upload_bytes(data: Vec<u8>, path: &str) -> Result<(), ()> {
        let client = reqwest::Client::new();
        let path = path.clone().replace("/", "%2F");
        dbg!(client
            .post(&format!("https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}", "voxelphile", path))
            .body(data)
            .bearer_auth(match Self::get_access_token().await {
                Ok(x) => x,
                Err(e) => { dbg!(e);  Err(())? },
            })
            .header("Content-Type", "image/jpeg")
            .send()
            .await)
            .map(|_| ())
            .map_err(|e| {dbg!(e); ()})
    }

    async fn get_access_token() -> Result<String, GoogleApiError> {
        use GoogleApiError::*;

        let client = reqwest::Client::new();

        let iat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let google_access_token_claims = GoogleAccessTokenClaims {
            iss: "voxelphile@voxelphile.iam.gserviceaccount.com".to_owned(),
            scope: "https://www.googleapis.com/auth/cloud-platform".to_owned(),
            aud: "https://oauth2.googleapis.com/token".to_owned(),
            exp: iat + 60,
            iat,
        };

        let encoding_key =
            EncodingKey::from_rsa_pem(&env::var("GOOGLE_API_CLIENT_SECRET").unwrap().as_bytes())
                .unwrap();
        let token_string = encode(
            &Header::new(Algorithm::RS256),
            &google_access_token_claims,
            &encoding_key,
        )
        .unwrap();

        let mut params = std::collections::HashMap::new();
        params.insert("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer");
        params.insert("assertion", token_string.as_str());

        let GoogleApiAccessTokenResp {
            access_token: google_api_access_token,
            ..
        } = client
            .post("https://oauth2.googleapis.com/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                dbg!(e);
                ServerError
            })?
            .json::<GoogleApiAccessTokenResp>()
            .await
            .map_err(|_| ServerError)?;

        Ok(google_api_access_token)
    }

    async fn get_user(postgres: &Postgres, user: User) -> Result<UserDetails, UserGetError> {
        use UserGetError::*;

        let postgres = postgres.lock().await;
        let statement = "select username, email, profile_id from voxelphile.users where id = $1;";
        let params = [&user.id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync)];
        dbg!(&user.id);
        let Ok(row) = postgres.query_one(statement, &params).await else {
            Err(NotFound)?
        };

        Ok(UserDetails {
            username: row.get::<_, String>(0),
            email: row.get::<_, String>(1),
            profile: row.try_get::<_, String>(2).ok(),
        })
    }

    async fn login_user(
        postgres: &Postgres,
        credentials: &UserCredentials,
    ) -> Result<Token, UserLoginError> {
        let postgres = postgres.lock().await;

        use UserLoginError::*;
        let (query, params) = {
            let email = &credentials.email;
            let query = "select voxelphile.users.id, voxelphile.user_password_logins.password_hash from voxelphile.users
            left join voxelphile.user_password_logins on voxelphile.users.id = voxelphile.user_password_logins.id where email = $1 limit 1;";
            let params = [email as &(dyn tokio_postgres::types::ToSql + Sync)];
            (query, params)
        };

        let Ok(statement) = postgres.prepare(query).await.map_err(|e| e) else {
            Err(DbError)?
        };

        let Ok(row) = postgres.query_one(&statement, &params).await.map_err(|e| e)  else {
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
        };;

        let key: hmac::Hmac<sha2::Sha256> = hmac::Hmac::new_from_slice(
            &dbg!(env::var("VOXELPHILE_JWT_SECRET").unwrap()).as_bytes(),
        )
        .unwrap();

        let exp = SystemTime::now()
            .checked_add(Duration::from_secs(86400))
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f32() as usize;

        dbg!(&id_string);

        let claims = UserClaims { id: id_string, exp };

        let token_string = claims.sign_with_key(&key).unwrap();

        Ok(token_string)
    }

    async fn register_user(
        postgres: &Postgres,
        registration: &UserRegistration,
    ) -> Result<Token, UserRegistrationError> {
        let mut postgres = postgres.lock().await;

        use UserRegistrationError::*;

        check_username(&registration.username)
            .then_some(())
            .ok_or(BadUsername)?;

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

                let password_hash_string =
                    hash_password(password.clone()).await.ok_or(BadPassword)?;

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
                        "insert into voxelphile.users (id, username, email) values ($1, $2, $3);";

                    let email_dyn_ref = &email as &(dyn tokio_postgres::types::ToSql + Sync);

                    let mut params = vec![];

                    params.push(&id as &(dyn tokio_postgres::types::ToSql + Sync));

                    params
                        .push(&registration.username as &(dyn tokio_postgres::types::ToSql + Sync));

                    params.push(email_dyn_ref);

                    dbg!(transaction.execute(statement, &params).await).map_err(|e| {
                        if let Some(&SqlState::UNIQUE_VIOLATION) = e.code() {
                            Duplicate
                        } else {
                            DbError
                        }
                    })?;
                }
                {
                    let statement = "insert into voxelphile.user_password_logins (id, password_hash) values ($1, $2);";

                    let password_hash_dyn_ref =
                        &password_hash_string as &(dyn tokio_postgres::types::ToSql + Sync);

                    let mut params = vec![];

                    params.push(&id as &(dyn tokio_postgres::types::ToSql + Sync));

                    params.push(password_hash_dyn_ref);

                    let Ok(_) = dbg!(transaction.execute(statement, &params).await) else {
                        Err(DbError)?
                    };
                }
            }
        }

        dbg!(transaction.commit().await).map_err(|e| DbError)?;

        let key: hmac::Hmac<sha2::Sha256> =
            hmac::Hmac::new_from_slice(&env::var("VOXELPHILE_JWT_SECRET").unwrap().as_bytes())
                .unwrap();

        let exp = SystemTime::now()
            .checked_add(Duration::from_secs(86400))
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f32() as usize;

        let claims = UserClaims {
            id: id.to_string(),
            exp,
        };

        let token_string = claims.sign_with_key(&key).unwrap();

        Ok(token_string)
    }

    async fn change_user(
        postgres: &Postgres,
        change: &UserChange,
        user: User,
    ) -> Result<(), UserChangeError> {
        use UserChangeError::*;

        let id = user.id;

        let mut postgres = postgres.lock().await;

        let mut profile_data = vec![];

        dbg!("yo");
        if let Some(profile) = &change.profile {
            let offset = profile.find(',').ok_or(BadProfile)? + 1;
            let mut value = profile.clone();
            value.drain(..offset);
            profile_data = value.from_base64().map_err(|_| BadProfile)?;

            let statement = "select voxelphile.users.profile_id from voxelphile.users where voxelphile.users.id = $1;";
            let params = [&id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync)];

            let Ok(row) = dbg!(postgres.query_one(statement, &params).await) else {
                dbg!("yo");
                Err(ServerError)?
            };

            if let Some(profile_id) = dbg!(row.try_get::<_, String>(0)).ok() {
                let profile_path = format!("user/profile/{}.jpeg", profile_id.to_string());

                Self::remove(&profile_path).await.map_err(|e| {
                    dbg!(e);
                    ServerError
                })?;
            }
        }

        let transaction = postgres.transaction().await.map_err(|e| DbError)?;

        dbg!("yo");
        if let Some(_) = &change.profile {
            let image = image::io::Reader::with_format(
                Cursor::new(profile_data.clone()),
                image::ImageFormat::Jpeg,
            )
            .decode()
            .map_err(|_| BadProfile)?;

            if image.width() > 120 || image.height() > 120 {
                Err(BadProfile)?
            }

            let profile_id = uuid::Uuid::new_v4();

            let statement = "update voxelphile.users set profile_id = $1 where id = $2;";

            let params = [
                &profile_id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync),
                &id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync),
            ];
            dbg!("yop");
            dbg!(transaction.execute(statement, &params).await).map_err(|e| {
                dbg!(e);
                DbError
            })?;

            let profile_path = format!("user/profile/{}.jpeg", profile_id.to_string());

            Self::upload_bytes(profile_data, &profile_path)
                .await
                .map_err(|e| {
                    dbg!(e);
                    ServerError
                })?;
        }

        let mut password_hash = None;
        dbg!("yo");
        if let Some(password) = change.password.clone() {
            check_password(&password).then_some(()).ok_or(BadPassword)?;

            password_hash = Some(hash_password(password).await.ok_or(BadPassword)?);
        }
        dbg!("yo");
        if let Some(password_hash) = &password_hash {
            let statement =
                "update voxelphile.user_password_logins set password = $1 where id = $2;";

            let params = [
                &password_hash as &(dyn tokio_postgres::types::ToSql + Sync),
                &id.to_string() as &(dyn tokio_postgres::types::ToSql + Sync),
            ];

            transaction
                .execute(statement, &params)
                .await
                .map_err(|_| DbError)?;
        }

        let mut params = vec![];
        let mut updates = Vec::<String>::new();
        let mut prepared = Vec::<String>::new();
        dbg!(&change.username);
        dbg!(&change.email);

        dbg!("yo");
        if let Some(username) = &change.username {
            check_username(username).then_some(()).ok_or(BadUsername)?;

            updates.push("username".to_owned());
            params.push(username as &(dyn tokio_postgres::types::ToSql + Sync));
            prepared.push(format!("${}", params.len()));
        }
        dbg!("yo");
        if let Some(email) = &change.email {
            check_email(email).then_some(()).ok_or(BadEmail)?;

            updates.push("email".to_owned());
            params.push(email as &(dyn tokio_postgres::types::ToSql + Sync));
            prepared.push(format!("${}", params.len()));
        }

        let id_string = id.to_string();

        dbg!("yo");
        if updates.len() > 0 {
            params.push(&id_string as &(dyn tokio_postgres::types::ToSql + Sync));
            let id_prepared = params.len();

            let mut sub_statements = vec![];

            for i in 0..updates.len() {
                sub_statements.push(format!(
                    "{} = {}",
                    updates.pop().unwrap(),
                    prepared.pop().unwrap()
                ));
            }

            let update_col_expr = sub_statements.join(", ");

            let statement = format!(
                "update voxelphile.users set {} where id = {};",
                update_col_expr,
                format!("${}", id_prepared)
            );

            dbg!(&statement);

            dbg!(transaction.execute(&statement, &params).await).map_err(|e| {
                if let Some(&SqlState::UNIQUE_VIOLATION) = e.code() {
                    Duplicate
                } else {
                    DbError
                }
            })?;
        }

        transaction.commit().await.map_err(|e| DbError)?;

        Ok(())
    }

    async fn retrieve_sol() -> Result<SolAddress, SolError> {
        todo!()
    }
}

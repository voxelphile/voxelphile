pub mod infra;
pub mod sol;
pub mod user;
use axum::extract::State;
use axum::{response::*, routing::*, *};
use user::*;
use http::status::*;
use infra::*;
use std::{net::*, env};

#[tokio::main]
async fn main() {
    let postgres = Infra::create_db_client_connection()
        .await
        .expect("failed to connect to postgres");

    let app = Router::new()
        .route("/", get(root))
        .route("/user/login", post(user_login))
        .route("/user/register", post(user_register))
        .route("/sol/connect", post(sol_connect))
        .with_state(postgres);

    axum::Server::bind(&format!("0.0.0.0:{}", env::var("PORT").unwrap()).parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> Json<&'static str> {
    Json("Hello, Xenotech!")
}

async fn user_login(
    State(postgres): State<Postgres>,
    credentials: Json<UserCredentials>,
) -> impl IntoResponse {
    let result = Infra::login_user(&postgres, &*credentials).await;
    let status = match &result {
        Ok(_) => StatusCode::OK,
        Err(err) => err.status_code(),
    };
    let mut response = match result {
        Ok(token) => Json(token).into_response(),
        Err(err) => Json(err).into_response(),
    };
    *response.status_mut() = status;
    response
}

async fn user_register(
    State(postgres): State<Postgres>,
    registration: Json<UserRegistration>,
) -> impl IntoResponse {
    let result = Infra::register_user(&postgres, &*registration).await;
    let status = match &result {
        Ok(_) => StatusCode::OK,
        Err(err) => err.status_code(),
    };
    let mut response = match result {
        Ok(token) => Json(token).into_response(),
        Err(err) => Json(err).into_response(),
    };
    *response.status_mut() = status;
    response
}

async fn sol_connect() -> impl IntoResponse {

    (StatusCode::OK)
}

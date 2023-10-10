pub mod infra;
pub mod sol;
pub mod user;
use axum::extract::State;
use axum::middleware::Next;
use axum::{response::*, routing::*, *};
use axum_macros::debug_handler;
use http::{status::*, Request};
use infra::*;
use std::io::Write;
use std::{env, io, net::*};
use user::*;

#[tokio::main]
async fn main() {
    let postgres = Infra::create_db_client_connection()
        .await
        .expect("failed to connect to postgres");

    let unprotected_route = Router::new()
        .route("/", get(root))
        .route("/user/login", post(user_login))
        .route("/user/register", post(user_register))
        .route("/sol/connect", post(sol_connect));

    let protected_route = Router::new()
        .route("/user", get(user_get))
        .route("/user/change", post(user_change))
        .layer(middleware::from_fn(jwt_authentification));

    let app = Router::new()
        .merge(unprotected_route)
        .merge(protected_route)
        .with_state(postgres);

    axum::Server::bind(
        &format!("0.0.0.0:{}", env::var("PORT").unwrap())
            .parse()
            .unwrap(),
    )
    .serve(app.into_make_service())
    .await
    .unwrap();
}

async fn root() -> Json<&'static str> {
    Json("Hello, Xenotech!")
}

const AUTHORIZATION: &str = "Authorization";
const BEARER: &str = "Bearer ";

pub async fn jwt_authentification<B>(mut request: Request<B>, next: Next<B>) -> impl IntoResponse {
    let authorization_header = match request.headers().get(AUTHORIZATION) {
        Some(v) => v,
        None => return Err((StatusCode::UNAUTHORIZED, Json("Unauthorized"))),
    };

    let authorization = match authorization_header.to_str() {
        Ok(v) => v,
        Err(_) => return Err((StatusCode::UNAUTHORIZED, Json("Unauthorized"))),
    };

    if !authorization.starts_with(BEARER) {
        return Err((StatusCode::UNAUTHORIZED, Json("Unauthorized")));
    }

    let jwt_token = authorization.trim_start_matches(BEARER);
    dbg!(jwt_token);
    let token_header = match jsonwebtoken::decode_header(&jwt_token) {
        Ok(header) => header,
        _ => return Err((StatusCode::UNAUTHORIZED, Json("Unauthorized"))),
    };

    // Get token header
    let user_claims = match jsonwebtoken::decode::<UserClaims>(
        &jwt_token,
        &jsonwebtoken::DecodingKey::from_secret(
            &dbg!(env::var("VOXELPHILE_JWT_SECRET").unwrap()).as_bytes(),
        ),
        &jsonwebtoken::Validation::new(token_header.alg),
    ) {
        Ok(claims) => claims,
        Err(e) => {
            return {
                dbg!(e);
                Err((StatusCode::UNAUTHORIZED, Json("Unauthorized")))
            }
        }
    };

    let user = User {
        id: uuid::Uuid::parse_str(dbg!(&user_claims.claims.id))
            .map_err(|_| (StatusCode::UNAUTHORIZED, Json("Unauthorized")))?,
    };

    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

async fn user_get(
    State(postgres): State<Postgres>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    let result = Infra::get_user(&postgres, user).await;
    let status = match &result {
        Ok(_) => StatusCode::OK,
        Err(err) => err.status_code(),
    };
    let mut response = match result {
        Ok(user) => Json(user).into_response(),
        Err(err) => Json(err).into_response(),
    };
    *response.status_mut() = status;
    response
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

async fn user_change(
    State(postgres): State<Postgres>,
    Extension(user): Extension<User>,
    change: Json<UserChange>,
) -> impl IntoResponse {
    let result = Infra::change_user(&postgres, &*change, user).await;
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

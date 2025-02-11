use std::usize;

use axum::{extract::{FromRequestParts, Path, State}, http::{request::Parts, StatusCode}, response::{IntoResponse, Response}, Extension, Json};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, SqlitePool};
use uuid::Uuid;

use crate::refs::Count;

const DEFAULT_REF: &str = "NOREF";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Level {
    Normal,
    Admin
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdminUser(User);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub reference: String,
    pub level: Level,
    pub exp: usize,
    pub id: Uuid
}

impl User {
    pub fn new(reference: String, level: Level) -> Self {
        Self {
            reference,
            level,
            exp: usize::MAX,
            id: Uuid::new_v4()
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AuthPassword {
    pub password: String
}

#[derive(Clone)]
pub struct JWTConfig {
    pub secret: String,
}

pub async fn get_ref(user: User) -> impl IntoResponse {
    user.reference
}

fn make_token(user: &User, sec: &str) -> String {
    encode(
        &Header::default(),
        user,
        &EncodingKey::from_secret(sec.as_ref())
    ).unwrap()
}

pub async fn generate_token(State(pool): State<SqlitePool>, Path(reference): Path<String>, Extension(jc): Extension<JWTConfig>) -> Result<Response, Response> {
    let Count { count } = query_as!(
        Count,
        "SELECT COUNT(*) as count FROM refs WHERE refstr = ?",
        reference
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    if count == 0 {
        return Err((StatusCode::BAD_REQUEST, "Reference does not exist").into_response());
    }

    Ok(make_token(&User::new(reference, Level::Normal), &jc.secret).into_response())
}

pub async fn upgrade(Extension(AuthPassword{ password: pw }): Extension<AuthPassword>, Extension(jc): Extension<JWTConfig>, Json(AuthPassword { password }): Json<AuthPassword>) -> Result<Response, Response> {

    // if you're reading this and you aren't me, look away <3
    if password == pw {
        return Ok(make_token(&User::new(DEFAULT_REF.to_string(), Level::Admin), &jc.secret).into_response());
    }

    Err(StatusCode::UNAUTHORIZED.into_response())
}

impl<S> FromRequestParts<S> for User
where
    S: Send + Sync + std::fmt::Debug, 
{
    type Rejection = Response;
    
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token: String = parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED.into_response())?.to_string();

        let jc: &JWTConfig = parts.extensions.get().expect("No JWT Config Set Up");

        let user = decode::<User>(&token, &DecodingKey::from_secret(jc.secret.as_ref()), &Validation::default()).map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

        Ok(user.claims)
    }
}

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync + std::fmt::Debug,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S,) -> Result<Self, Self::Rejection> {
        let user: User = User::from_request_parts(parts, state).await?;

        match user.level {
            Level::Normal => Err((StatusCode::UNAUTHORIZED, "You are not an admin user!").into_response()),
            Level::Admin => Ok(AdminUser(user)),
        }
    }
}
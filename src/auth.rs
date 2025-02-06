use std::usize;

use axum::{extract::{FromRequestParts, Path}, http::{request::Parts, StatusCode}, response::{IntoResponse, Response}, Extension, Json};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_REF: &str = "NOREF";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Level {
    Normal,
    Admin
}

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

pub async fn generate_token(Path(reference): Path<String>, Extension(jc): Extension<JWTConfig>) -> impl IntoResponse {
    make_token(&User::new(reference, Level::Normal), &jc.secret)
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
            .and_then(|h| match h.is_empty() {
                true => None,
                false => Some(h),
            })
            .and_then(|s| s.strip_prefix("jwt=").map(|s| s.to_string()))
            .ok_or(StatusCode::UNAUTHORIZED.into_response())?;

        let jc: &JWTConfig = parts.extensions.get().expect("No JWT Config Set Up");

        let user = decode::<User>(&token, &DecodingKey::from_secret(jc.secret.as_ref()), &Validation::default()).map_err(|_| StatusCode::UNAUTHORIZED.into_response())?;

        Ok(user.claims)
    }
}
use std::hash::{DefaultHasher, Hash, Hasher};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response}
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, SqlitePool};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct EntityReference {
    name: String,
    skills: Vec<String>,
}

struct Count {
    count: i64,
}

#[instrument(skip(pool) err(Debug))]
pub async fn create_ref(
    State(pool): State<SqlitePool>,
    Path(name): Path<String>,
) -> Result<Response, Response> {
    let mut h = DefaultHasher::default();
    name.hash(&mut h);

    let ref_key = base64::engine::general_purpose::URL_SAFE.encode(h.finish().to_be_bytes());

    let Count { count } = query_as!(
        Count,
        "SELECT COUNT(*) as count FROM refs WHERE refstr = ?",
        ref_key
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    if count != 0 {
        return Ok((StatusCode::BAD_REQUEST, "Reference already exists").into_response());
    }

    query!(
        "INSERT INTO refs (refstr, name, relevant_skills) VALUES(?, ?, ?)",
        ref_key,
        name,
        ""
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok((StatusCode::OK, ref_key).into_response())
}

#[instrument(skip(pool) err(Debug))]
pub async fn get_ref_name(
    State(pool): State<SqlitePool>,
    Path(ref_str): Path<String>,
) -> Result<Response, Response> {
    let result = query!("SELECT name FROM refs WHERE refstr = ?", ref_str)
        .fetch_one(&pool)
        .await;

    match result {
        Ok(rec) => Ok((
            StatusCode::OK,
            rec.name
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR.into_response())?,
        )
            .into_response()),
        Err(_) => Ok((StatusCode::BAD_REQUEST, "Ref does not exist").into_response()),
    }
}

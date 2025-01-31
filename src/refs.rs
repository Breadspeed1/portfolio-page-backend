use std::{cmp::Ordering, hash::{DefaultHasher, Hash, Hasher}};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response}, Json
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, SqlitePool};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct EntitySkills {
    name: String,
    skills: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityReference {
    name: Option<String>,
    refstr: Option<String>
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
        "[]"
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok((StatusCode::OK, ref_key).into_response())
}

#[instrument(skip(pool), err(Debug))]
pub async fn list_refs(State(pool): State<SqlitePool>) -> Result<Response, Response> {
    let refs: Vec<EntityReference> = query_as!(
        EntityReference,
        "SELECT refstr, name FROM refs"
    ).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;


    Ok((StatusCode::OK,serde_json::to_string(&refs).unwrap()).into_response())
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

pub async fn create_skill(State(pool): State<SqlitePool>, Path(name): Path<String>) -> Result<Response, Response> {
    let Count { count } = query_as!(
        Count,
        "SELECT COUNT(*) as count FROM skills WHERE skill = ?",
        name
    ).fetch_one(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    if count > 0 {
        return Ok((StatusCode::BAD_REQUEST, "Skill already exists").into_response());
    }

    query!(
        "INSERT INTO skills (skill) VALUES(?)",
        name
    ).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(StatusCode::OK.into_response())
}

pub async fn search_skills(State(pool): State<SqlitePool>, Path(search_term): Path<String>) -> Result<Response, Response> {
    let skills: Vec<String> = query!("SELECT skill FROM skills")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .into_iter()
        .map(|row| row.skill.unwrap())
        .collect();

    let skills: Vec<&str> = skills.iter().map(|s| s.as_ref()).collect();

    let mut res = rust_fuzzy_search::fuzzy_search_threshold(&search_term, skills.as_slice(), 0.5);

    //bad solution
    res.sort_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap_or(Ordering::Equal));

    let res = res.iter().map(|(s, _)| s.to_string()).collect::<Vec<String>>();
    let res = serde_json::ser::to_string(&res).unwrap();

    Ok((StatusCode::OK, res).into_response())
}

pub async fn add_skill_to_ref(State(pool): State<SqlitePool>, Path((refstr, skill)): Path<(String, String)>) -> Result<Response, Response> {
    let Count { count } = query_as!(
        Count,
        "SELECT COUNT(*) as count FROM skills WHERE skill = ?",
        skill
    ).fetch_one(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    if count == 0 {
        return Err((StatusCode::BAD_REQUEST, "Skill does not exist").into_response());
    }

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    let skills_data = query!(
        "SELECT relevant_skills FROM refs WHERE refstr = ?",
        refstr
    ).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    .ok_or((StatusCode::BAD_REQUEST, "Reference does not exist").into_response())?
    .relevant_skills.unwrap();

    let mut skills: Vec<String> = serde_json::de::from_slice(skills_data.as_slice()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    skills.push(skill);

    let skills_str = serde_json::ser::to_string(&skills).unwrap();

    query!(
        "UPDATE refs SET relevant_skills = ? WHERE refstr = ?",
        skills_str,
        refstr
    ).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(StatusCode::OK.into_response())
}

pub async fn get_skills(State(pool): State<SqlitePool>, Path(refstr): Path<String>) -> Result<Response, Response> {
    let skills_data = query!(
        "SELECT relevant_skills FROM refs WHERE refstr = ?",
        refstr
    ).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    .ok_or((StatusCode::BAD_REQUEST, "Ref does not exist").into_response())?
    .relevant_skills.unwrap();

    Ok((StatusCode::OK, String::from_utf8(skills_data).unwrap()).into_response())
}
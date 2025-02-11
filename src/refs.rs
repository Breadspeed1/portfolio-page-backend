use std::hash::{DefaultHasher, Hash, Hasher};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response}, Json
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, SqlitePool};
use tracing::instrument;

const DEFAULT_REF: &str = "NOREF";

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct EntitySkills {
    name: Option<String>,
    relevant_skills: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityReference {
    name: Option<String>,
    refstr: Option<String>
}

pub struct Count {
    pub count: i64,
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

pub async fn delete_ref(
    State(pool): State<SqlitePool>,
    Path(refstr): Path<String>,
) -> Result<Response, Response> {
    if refstr == DEFAULT_REF {
        return Err((StatusCode::BAD_REQUEST, "cannot delete default ref").into_response());
    }

    query!(
        "DELETE FROM refs WHERE refstr = ?",
        refstr,
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(StatusCode::OK.into_response())
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

pub async fn delete_skill(State(pool): State<SqlitePool>, Path(skill): Path<String>) -> Result<Response, Response> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    let Count { count } = query_as!(
        Count,
        "SELECT COUNT(*) as count FROM skills WHERE skill = ?",
        skill
    ).fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    if count == 0 {
        return Ok(StatusCode::OK.into_response());
    }


    let refs: Vec<EntitySkills> = query_as!(
        EntitySkills,
        "SELECT relevant_skills, name FROM refs"
    ).fetch_all(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    for i in 0..refs.len() {
        let mut skills: Vec<String> = serde_json::from_slice(refs[i].relevant_skills.as_ref().unwrap().as_ref()).unwrap();
        skills.retain(|s| !skill.eq(s));
        let new_skills = serde_json::to_vec(&skills).unwrap();
        let name = refs[i].name.as_ref().unwrap();

        query!(
            "UPDATE refs SET relevant_skills = ? WHERE name = ?",
            new_skills,
            name
        ).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    }

    query!(
        "DELETE FROM skills WHERE skill = ?",
        skill
    ).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(StatusCode::OK.into_response())
}

pub async fn list_skills(State(pool): State<SqlitePool>) -> Result<Response, Response> {
    let skills: Vec<String> = query!("SELECT skill FROM skills")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .into_iter()
        .map(|row| row.skill.unwrap())
        .collect();

    Ok((StatusCode::OK, Json(skills)).into_response())
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

    if !skills.contains(&skill) {
        skills.push(skill);
    }

    let skills_str = serde_json::ser::to_string(&skills).unwrap();

    query!(
        "UPDATE refs SET relevant_skills = ? WHERE refstr = ?",
        skills_str,
        refstr
    ).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(StatusCode::OK.into_response())
}

pub async fn remove_skill_from_ref(State(pool): State<SqlitePool>, Path((refstr, skill)): Path<(String, String)>) -> Result<Response, Response> {
    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    let skills_data = query!(
        "SELECT relevant_skills FROM refs WHERE refstr = ?",
        refstr
    ).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
    .ok_or((StatusCode::BAD_REQUEST, "Reference does not exist").into_response())?
    .relevant_skills.unwrap();

    let mut skills: Vec<String> = serde_json::de::from_slice(skills_data.as_slice()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;


    skills.retain(|s| !skill.eq(s));

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

    Ok((StatusCode::OK, Json::<Vec<String>>::from_bytes(skills_data.as_slice()).unwrap()).into_response())
}
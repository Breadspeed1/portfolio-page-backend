use auth::{AdminUser, AuthPassword, JWTConfig, User};
use axum::{http::StatusCode, middleware::from_extractor, routing::{delete, get, post}, Extension, Router};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod refs;
mod auth;

#[tokio::main]
async fn main() {
    let sqlite_addr = dotenv::var("DATABASE_URL").unwrap();

    let jwt_config = JWTConfig {
        secret: dotenv::var("JWT_SECRET").expect("No jwt secret found in environment.")
    };

    let conn_pool = SqlitePool::connect(&sqlite_addr).await.unwrap();

    let pw = dotenv::var("ADMIN_PASSWORD").expect("no admin password set");

    sqlx::migrate!("./migrations")
        .run(&conn_pool)
        .await.unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(dotenv::var("LOG_LEVEL").unwrap()))
        .init();

    let app = Router::new()
        .route("/ref/create/{name}", post(refs::create_ref))
        .route("/ref/delete/{name}", delete(refs::delete_ref))
        .route("/skills/create/{name}", post(refs::create_skill))
        .route("/ref/list", get(refs::list_refs))
        .route("/skills/delete/{skill}", delete(refs::delete_skill))
        .route("/ref/{ref}/add_skill/{skill}", post(refs::add_skill_to_ref))
        .route("/ref/{ref}/remove_skill/{skill}", delete(refs::remove_skill_from_ref))
        .route("/admincheck", get(StatusCode::OK))
        .layer(from_extractor::<AdminUser>())
        .route("/ref/{ref}/skills", get(refs::get_skills))
        .route("/skills/list", get(refs::list_skills))
        .route("/ref/{ref}/name", get(refs::get_ref_name))
        .route("/getref", get(auth::get_ref))
        .layer(from_extractor::<User>())
        .route("/token/{ref}", get(auth::generate_token))
        .route("/token/admin", post(auth::upgrade))
        .layer(Extension(jwt_config))
        .layer(Extension(AuthPassword{ password: pw }))
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(conn_pool);

    let addr = dotenv::var("BIND_ADDR").unwrap();

    info!("Starting server - Listening on {addr}");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

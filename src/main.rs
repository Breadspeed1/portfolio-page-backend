use axum::{routing::get, Router};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

mod refs;

#[tokio::main]
async fn main() {
    let sqlite_addr = dotenv::var("DATABASE_URL").unwrap();

    let conn_pool = SqlitePool::connect(&sqlite_addr).await.unwrap();

    sqlx::query!("CREATE TABLE IF NOT EXISTS refs (refstr TEXT, name TEXT, relevant_skills BLOB)")
        .execute(&conn_pool)
        .await
        .unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(dotenv::var("LOG_LEVEL").unwrap()))
        .init();

    let app = Router::new()
        .route("/ref/{ref}/name", get(refs::get_ref_name))
        .route("/ref/create/{name}", get(refs::create_ref))
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(conn_pool);

    let listener = TcpListener::bind(dotenv::var("BIND_ADDR").unwrap()).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

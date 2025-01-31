use axum::{routing::{delete, get, post}, Router};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

mod refs;

#[tokio::main]
async fn main() {
    let sqlite_addr = dotenv::var("DATABASE_URL").unwrap();

    let conn_pool = SqlitePool::connect(&sqlite_addr).await.unwrap();

    sqlx::migrate!("./migrations")
        .run(&conn_pool)
        .await.unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(dotenv::var("LOG_LEVEL").unwrap()))
        .init();

    let app = Router::new()
        .route("/ref/{ref}/name", get(refs::get_ref_name))
        .route("/ref/create/{name}", post(refs::create_ref))
        .route("/skills/create/{name}", post(refs::create_skill))
        .route("/skills/search/{search_term}", get(refs::search_skills))
        .route("/ref/{ref}/add_skill/{skill}", post(refs::add_skill_to_ref))
        .route("/ref/{ref}/remove_skill/{skill}", delete(refs::remove_skill_from_ref))
        .route("/ref/{ref}/skills", get(refs::get_skills))
        .route("/ref/list", get(refs::list_refs))
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(conn_pool);

    let listener = TcpListener::bind(dotenv::var("BIND_ADDR").unwrap()).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

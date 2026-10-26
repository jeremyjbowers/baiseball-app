mod error;
mod http;
mod store;
mod validate;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::routing::{get, post};
use axum::Router;
use http::AppState;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

#[tokio::main]
async fn main() {
    let db_path = std::env::var("LEAGUE_ADMIN_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/league-admin.sqlite"));
    let web_origin =
        std::env::var("ADMIN_WEB_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:41741".to_string());
    let addr: SocketAddr = std::env::var("ADMIN_API_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:41742".to_string())
        .parse()
        .expect("ADMIN_API_ADDR must be host:port");

    let conn = store::open(&db_path).unwrap_or_else(|err| {
        eprintln!("{}", err.message);
        std::process::exit(1);
    });
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        web_origin: web_origin.clone(),
    };

    let mut origins = vec![
        "http://127.0.0.1:41741".parse().unwrap(),
        "http://localhost:41741".parse().unwrap(),
    ];
    if let Ok(extra) = web_origin.parse() {
        if !origins.contains(&extra) {
            origins.push(extra);
        }
    }
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/auth/magic-links", post(http::magic_link))
        .route("/auth/sessions", post(http::exchange).delete(http::logout))
        .route("/auth/session", get(http::current_session))
        .route("/leagues", get(http::list_leagues).post(http::create_league))
        .route(
            "/leagues/:league_id",
            get(http::get_league)
                .patch(http::update_league)
                .delete(http::delete_league),
        )
        .route(
            "/leagues/:league_id/divisions",
            get(http::list_divisions).post(http::create_division),
        )
        .route(
            "/leagues/:league_id/teams",
            get(http::list_league_teams).post(http::create_team),
        )
        .route(
            "/divisions/:division_id",
            get(http::get_division)
                .patch(http::update_division)
                .delete(http::delete_division),
        )
        .route("/teams", get(http::list_controlled))
        .route(
            "/teams/:team_id",
            get(http::get_team)
                .patch(http::update_team)
                .delete(http::delete_team),
        )
        .route(
            "/teams/:team_id/lineups",
            get(http::list_lineups).post(http::create_lineup),
        )
        .route(
            "/lineups/:lineup_id",
            get(http::get_lineup)
                .patch(http::update_lineup)
                .delete(http::delete_lineup),
        )
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!("admin-api listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve");
}

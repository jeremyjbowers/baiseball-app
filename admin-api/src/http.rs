use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::store;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub web_origin: String,
}

fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, ApiError> {
    state.db.lock().map_err(|_| ApiError::internal())
}

fn bearer(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let token = value.strip_prefix("Bearer ").unwrap_or("").trim();
    if token.is_empty() {
        return Err(ApiError::unauthorized());
    }
    Ok(token.to_string())
}

async fn read_json(request: Request) -> Result<Value, ApiError> {
    let bytes = axum::body::to_bytes(request.into_body(), 1_048_576)
        .await
        .map_err(|_| ApiError::bad("The request body must be JSON."))?;
    if bytes.is_empty() {
        return Err(ApiError::bad("The request body must be JSON."));
    }
    serde_json::from_slice(&bytes).map_err(|_| ApiError::bad("The request body must be JSON."))
}

fn json_response(status: StatusCode, value: Value) -> Result<Response, ApiError> {
    Ok((status, Json(value)).into_response())
}

pub async fn magic_link(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let mut db = lock(&state)?;
    let (email, token, expires_at) = store::issue_magic_link(&mut db, &body)?;
    let origin = state.web_origin.trim_end_matches('/');
    let magic_link = format!("{origin}/auth/verify?token={token}");
    println!("league-office magic-link email={email} url={magic_link}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    json_response(
        StatusCode::OK,
        json!({
            "email": email,
            "magicLink": magic_link,
            "expiresAt": expires_at,
            "devOnly": true,
        }),
    )
}

pub async fn exchange(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let mut db = lock(&state)?;
    let Some((session_token, expires_at, owner)) = store::redeem_magic_link(&mut db, &token)? else {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "That sign-in link is expired or already used.",
        ));
    };
    json_response(
        StatusCode::OK,
        json!({
            "sessionToken": session_token,
            "expiresAt": expires_at,
            "owner": { "id": owner.id, "email": owner.email },
        }),
    )
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let db = lock(&state)?;
    store::delete_session(&db, &token)?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn current_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &token)?;
    let (leagues, teams) = store::session_counts(&db, &owner.id)?;
    json_response(
        StatusCode::OK,
        json!({
            "owner": { "id": owner.id, "email": owner.email },
            "administeredLeagueCount": leagues,
            "controlledTeamCount": teams,
        }),
    )
}

pub async fn list_leagues(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let leagues = store::list_leagues(&db, &owner.id)?;
    json_response(StatusCode::OK, json!({ "leagues": leagues }))
}

pub async fn create_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let league = store::create_league(&db, &owner.id, &body)?;
    json_response(StatusCode::CREATED, json!({ "league": league }))
}

pub async fn get_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let league = store::get_league(&db, &owner.id, &league_id)?;
    let teams = store::list_league_teams(&db, &owner.id, &league_id)?;
    json_response(StatusCode::OK, json!({ "league": league, "teams": teams }))
}

pub async fn update_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let league = store::update_league(&db, &owner.id, &league_id, &body)?;
    json_response(StatusCode::OK, json!({ "league": league }))
}

pub async fn delete_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let mut db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    store::delete_league(&mut db, &owner.id, &league_id)?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn list_divisions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let divisions = store::list_divisions(&db, &owner.id, &league_id)?;
    json_response(StatusCode::OK, json!({ "divisions": divisions }))
}

pub async fn create_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let division = store::create_division(&db, &owner.id, &league_id, &body)?;
    json_response(StatusCode::CREATED, json!({ "division": division }))
}

pub async fn get_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let division = store::get_division(&db, &owner.id, &division_id)?;
    json_response(StatusCode::OK, json!({ "division": division }))
}

pub async fn update_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let division = store::update_division(&db, &owner.id, &division_id, &body)?;
    json_response(StatusCode::OK, json!({ "division": division }))
}

pub async fn delete_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    store::delete_division(&db, &owner.id, &division_id)?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn list_controlled(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let (teams, leagues) = store::list_controlled(&db, &owner.id)?;
    json_response(StatusCode::OK, json!({ "teams": teams, "leagues": leagues }))
}

pub async fn list_league_teams(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let teams = store::list_league_teams(&db, &owner.id, &league_id)?;
    json_response(StatusCode::OK, json!({ "teams": teams }))
}

pub async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let mut db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let (team, team_count, in_bounds, bound_label) =
        store::create_team(&mut db, &owner.id, &league_id, &body)?;
    json_response(
        StatusCode::CREATED,
        json!({
            "team": team,
            "teamCount": team_count,
            "inBounds": in_bounds,
            "boundLabel": bound_label,
        }),
    )
}

pub async fn get_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let page = store::get_team_page(&db, &owner.id, &team_id)?;
    json_response(StatusCode::OK, page)
}

pub async fn update_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let team = store::update_team(&db, &owner.id, &team_id, &body)?;
    json_response(StatusCode::OK, json!({ "team": team }))
}

pub async fn delete_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Response, ApiError> {
    let mut db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    store::delete_team(&mut db, &owner.id, &team_id)?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn list_lineups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let lineups = store::list_lineups(&db, &owner.id, &team_id)?;
    json_response(StatusCode::OK, json!({ "lineups": lineups }))
}

pub async fn create_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let lineup = store::create_lineup(&db, &owner.id, &team_id, &body)?;
    json_response(StatusCode::CREATED, json!({ "lineup": lineup }))
}

pub async fn get_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let page = store::get_lineup_page(&db, &owner.id, &lineup_id)?;
    json_response(StatusCode::OK, page)
}

pub async fn update_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    let lineup = store::update_lineup(&db, &owner.id, &lineup_id, &body)?;
    json_response(StatusCode::OK, json!({ "lineup": lineup }))
}

pub async fn delete_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = lock(&state)?;
    let owner = store::require_owner(&db, &bearer(&headers)?)?;
    store::delete_lineup(&db, &owner.id, &lineup_id)?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

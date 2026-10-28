use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::db::Db;

use crate::error::ApiError;
use crate::store;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub web_origin: String,
}

async fn with_db<T: Send + 'static>(
    state: &AppState,
    work: impl FnOnce(&mut Db) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    let db = Arc::clone(&state.db);
    match tokio::task::spawn_blocking(move || {
        let mut guard = db.lock().map_err(|_| ApiError::internal())?;
        work(&mut guard)
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(ApiError::internal()),
    }
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
    let issued = with_db(&state, move |db| store::issue_magic_link(db, &body)).await?;
    let origin = state.web_origin.trim_end_matches('/');
    let magic_link = format!("{origin}/auth/verify?token={}", issued.token);
    let dev_only = if issued.channel == "sms" {
        crate::notify::send_sms(&issued.destination, &magic_link)?
    } else {
        crate::notify::send_email(&issued.destination, &magic_link)?
    };
    let mut payload = json!({
        "channel": issued.channel,
        "destination": issued.destination,
        "email": if issued.channel == "email" { Value::String(issued.destination.clone()) } else { Value::Null },
        "phone": if issued.channel == "sms" { Value::String(issued.destination) } else { Value::Null },
        "expiresAt": issued.expires_at,
        "devOnly": dev_only,
    });
    if dev_only {
        payload["magicLink"] = json!(magic_link);
    }
    json_response(StatusCode::OK, payload)
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
    let Some((session_token, expires_at, owner)) =
        with_db(&state, move |db| store::redeem_magic_link(db, &token)).await?
    else {
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
            "owner": { "id": owner.id, "email": owner.email, "phone": owner.phone, "isAdmin": owner.is_admin },
        }),
    )
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    with_db(&state, move |db| store::delete_session(db, &token)).await?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn current_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let (owner, leagues, teams) = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        let (leagues, teams) = store::session_counts(db, &owner.id)?;
        Ok((owner, leagues, teams))
    })
    .await?;
    json_response(
        StatusCode::OK,
        json!({
            "owner": { "id": owner.id, "email": owner.email, "phone": owner.phone, "isAdmin": owner.is_admin },
            "administeredLeagueCount": leagues,
            "controlledTeamCount": teams,
        }),
    )
}

pub async fn update_owner(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let phone = body
        .get("phone")
        .and_then(Value::as_str)
        .map(str::to_string);
    let owner = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::set_phone(db, &owner.id, phone.as_deref())
    })
    .await?;
    json_response(
        StatusCode::OK,
        json!({ "owner": { "id": owner.id, "email": owner.email, "phone": owner.phone, "isAdmin": owner.is_admin } }),
    )
}

pub async fn list_leagues(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let leagues = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::list_leagues(db, &owner.id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "leagues": leagues }))
}

pub async fn create_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let league = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::create_league(db, &owner.id, &body)
    })
    .await?;
    json_response(StatusCode::CREATED, json!({ "league": league }))
}

pub async fn import_roster(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let imported = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::import_roster(db, &owner.id, &league_id, &body)
    })
    .await?;
    json_response(StatusCode::CREATED, imported)
}

pub async fn get_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let (league, teams) = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        let league = store::get_league(db, &owner.id, &league_id)?;
        let teams = store::list_league_teams(db, &owner.id, &league_id)?;
        Ok((league, teams))
    })
    .await?;
    json_response(StatusCode::OK, json!({ "league": league, "teams": teams }))
}

pub async fn update_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let league = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::update_league(db, &owner.id, &league_id, &body)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "league": league }))
}

pub async fn delete_league(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::delete_league(db, &owner.id, &league_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn read_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let schedule = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::read_schedule(db, &owner.id, &league_id)
    })
    .await?;
    json_response(StatusCode::OK, schedule)
}

pub async fn build_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let bytes = axum::body::to_bytes(request.into_body(), 1_048_576)
        .await
        .map_err(|_| ApiError::bad("The request body must be JSON."))?;
    if !bytes.is_empty() {
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::bad("The request body must be JSON."))?;
        let record = crate::validate::object(&value)?;
        if !record.is_empty() {
            return Err(ApiError::bad(
                "The schedule is built from the saved season length, the games inside the division, the games outside it, and the rivalry pairs.",
            ));
        }
    }
    let token = bearer(&headers)?;
    let schedule = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::build_schedule(db, &owner.id, &league_id)
    })
    .await?;
    json_response(StatusCode::CREATED, schedule)
}

pub async fn list_divisions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let divisions = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::list_divisions(db, &owner.id, &league_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "divisions": divisions }))
}

pub async fn create_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let division = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::create_division(db, &owner.id, &league_id, &body)
    })
    .await?;
    json_response(StatusCode::CREATED, json!({ "division": division }))
}

pub async fn get_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let division = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::get_division(db, &owner.id, &division_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "division": division }))
}

pub async fn update_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let division = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::update_division(db, &owner.id, &division_id, &body)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "division": division }))
}

pub async fn delete_division(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(division_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::delete_division(db, &owner.id, &division_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn list_controlled(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let (teams, leagues) = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::list_controlled(db, &owner.id)
    })
    .await?;
    json_response(
        StatusCode::OK,
        json!({ "teams": teams, "leagues": leagues }),
    )
}

pub async fn list_league_teams(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let teams = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::list_league_teams(db, &owner.id, &league_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "teams": teams }))
}

pub async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(league_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let (team, team_count, in_bounds, bound_label) = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::create_team(db, &owner.id, &league_id, &body)
    })
    .await?;
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
    let token = bearer(&headers)?;
    let page = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::get_team_page(db, &owner.id, &team_id)
    })
    .await?;
    json_response(StatusCode::OK, page)
}

pub async fn update_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let team = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::update_team(db, &owner.id, &team_id, &body)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "team": team }))
}

pub async fn delete_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::delete_team(db, &owner.id, &team_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

pub async fn list_lineups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let lineups = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::list_lineups(db, &owner.id, &team_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "lineups": lineups }))
}

pub async fn create_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let lineup = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::create_lineup(db, &owner.id, &team_id, &body)
    })
    .await?;
    json_response(StatusCode::CREATED, json!({ "lineup": lineup }))
}

pub async fn get_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    let page = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::get_lineup_page(db, &owner.id, &lineup_id)
    })
    .await?;
    json_response(StatusCode::OK, page)
}

pub async fn update_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
    request: Request<Body>,
) -> Result<Response, ApiError> {
    let body = read_json(request).await?;
    let token = bearer(&headers)?;
    let lineup = with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::update_lineup(db, &owner.id, &lineup_id, &body)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "lineup": lineup }))
}

pub async fn delete_lineup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lineup_id): Path<String>,
) -> Result<Response, ApiError> {
    let token = bearer(&headers)?;
    with_db(&state, move |db| {
        let owner = store::require_owner(db, &token)?;
        store::delete_lineup(db, &owner.id, &lineup_id)
    })
    .await?;
    json_response(StatusCode::OK, json!({ "ok": true }))
}

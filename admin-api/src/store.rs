use chrono::{Duration, SecondsFormat, Utc};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};

use crate::error::{db_err, ApiError};
use crate::schedule;
use crate::validate::{self, object};

const OFFICE_EMAIL: &str = "office@localhost";

pub struct Owner {
    pub id: String,
    pub email: String,
}

pub fn open(path: &std::path::Path) -> Result<Connection, ApiError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            eprintln!("data dir: {err}");
            ApiError::internal()
        })?;
    }
    let conn = Connection::open(path).map_err(db_err)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS owners (
          id TEXT PRIMARY KEY,
          email TEXT NOT NULL UNIQUE,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS magic_links (
          token TEXT PRIMARY KEY,
          owner_id TEXT NOT NULL REFERENCES owners(id),
          expires_at TEXT NOT NULL,
          used_at TEXT,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
          token TEXT PRIMARY KEY,
          owner_id TEXT NOT NULL REFERENCES owners(id),
          expires_at TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS leagues (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          lineup_lock TEXT NOT NULL CHECK (lineup_lock IN ('game', 'week', 'half', 'series')),
          created_by TEXT NOT NULL REFERENCES owners(id),
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          league_type TEXT,
          season_length INTEGER,
          division_games INTEGER,
          outside_games INTEGER
        );
        CREATE TABLE IF NOT EXISTS divisions (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          name TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (league_id, name)
        );
        CREATE TABLE IF NOT EXISTS teams (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          division_id TEXT NOT NULL REFERENCES divisions(id),
          owner_id TEXT NOT NULL REFERENCES owners(id),
          name TEXT NOT NULL,
          home_park TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (league_id, name)
        );
        CREATE TABLE IF NOT EXISTS lineups (
          id TEXT PRIMARY KEY,
          team_id TEXT NOT NULL REFERENCES teams(id),
          name TEXT NOT NULL,
          vs_lhp TEXT NOT NULL,
          vs_rhp TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE (team_id, name)
        );
        CREATE TABLE IF NOT EXISTS rivalries (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          team_id TEXT NOT NULL REFERENCES teams(id),
          opponent_team_id TEXT NOT NULL REFERENCES teams(id),
          games INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          UNIQUE (league_id, team_id, opponent_team_id)
        );
        CREATE TABLE IF NOT EXISTS schedules (
          league_id TEXT PRIMARY KEY REFERENCES leagues(id),
          built_at TEXT NOT NULL,
          snapshot TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS schedule_games (
          id TEXT PRIMARY KEY,
          league_id TEXT NOT NULL REFERENCES leagues(id),
          round INTEGER NOT NULL,
          home_team_id TEXT NOT NULL REFERENCES teams(id),
          away_team_id TEXT NOT NULL REFERENCES teams(id),
          same_division INTEGER NOT NULL,
          rivalry INTEGER NOT NULL,
          created_at TEXT NOT NULL
        );
        ",
    )
    .map_err(db_err)?;
    migrate(&conn)?;
    office_owner(&conn)?;
    Ok(conn)
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, ApiError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(db_err)?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(db_err)?;
    for name in names {
        if name.map_err(db_err)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate(conn: &Connection) -> Result<(), ApiError> {
    if !has_column(conn, "leagues", "league_type")? {
        conn.execute_batch(
            "
            ALTER TABLE leagues ADD COLUMN league_type TEXT;
            ALTER TABLE leagues ADD COLUMN season_length INTEGER;
            ALTER TABLE leagues ADD COLUMN division_games INTEGER;
            ALTER TABLE leagues ADD COLUMN outside_games INTEGER;
            ",
        )
        .map_err(db_err)?;
    }
    Ok(())
}

pub fn office_owner(conn: &Connection) -> Result<Owner, ApiError> {
    upsert_owner(conn, OFFICE_EMAIL)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn later_iso(duration: Duration) -> String {
    (Utc::now() + duration).to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::thread_rng().fill(buffer.as_mut_slice());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes * 2);
    for byte in buffer {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn new_id() -> String {
    random_hex(16)
}

fn new_token() -> String {
    random_hex(32)
}

fn count(conn: &Connection, sql: &str, param: &str) -> Result<i64, ApiError> {
    conn.query_row(sql, [param], |row| row.get(0))
        .map_err(db_err)
}

pub fn upsert_owner(conn: &Connection, email: &str) -> Result<Owner, ApiError> {
    if let Some(owner) = conn
        .query_row(
            "SELECT id, email FROM owners WHERE email = ?1",
            [email],
            |row| {
                Ok(Owner {
                    id: row.get(0)?,
                    email: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(db_err)?
    {
        return Ok(owner);
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO owners (id, email, created_at) VALUES (?1, ?2, ?3)",
        params![id, email, now_iso()],
    )
    .map_err(db_err)?;
    Ok(Owner {
        id,
        email: email.to_string(),
    })
}

pub fn issue_magic_link(
    conn: &mut Connection,
    body: &Value,
) -> Result<(String, String, String), ApiError> {
    let email = validate::parse_email(object(body)?.get("email"))?;
    let owner = upsert_owner(conn, &email)?;
    let token = new_token();
    let created_at = now_iso();
    let expires_at = later_iso(Duration::minutes(15));
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute(
        "DELETE FROM magic_links WHERE owner_id = ?1 AND used_at IS NULL",
        [&owner.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "INSERT INTO magic_links (token, owner_id, expires_at, used_at, created_at) VALUES (?1, ?2, ?3, NULL, ?4)",
        params![token, owner.id, expires_at, created_at],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok((email, token, expires_at))
}

pub fn redeem_magic_link(
    conn: &mut Connection,
    token: &str,
) -> Result<Option<(String, String, Owner)>, ApiError> {
    if token.is_empty() {
        return Ok(None);
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let row = tx
        .query_row(
            "SELECT m.owner_id, m.expires_at, m.used_at, o.email
             FROM magic_links m JOIN owners o ON o.id = m.owner_id
             WHERE m.token = ?1",
            [token],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let Some((owner_id, expires_at, used_at, email)) = row else {
        return Ok(None);
    };
    if used_at.is_some() || expires_at <= now_iso() {
        return Ok(None);
    }
    let used = now_iso();
    tx.execute(
        "UPDATE magic_links SET used_at = ?1 WHERE token = ?2",
        params![used, token],
    )
    .map_err(db_err)?;
    let session = new_token();
    let session_expires = later_iso(Duration::days(7));
    tx.execute(
        "INSERT INTO sessions (token, owner_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![session, owner_id, session_expires, used],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(Some((
        session,
        session_expires,
        Owner {
            id: owner_id,
            email,
        },
    )))
}

pub fn owner_from_session(conn: &Connection, token: &str) -> Result<Option<Owner>, ApiError> {
    if token.is_empty() {
        return Ok(None);
    }
    let row = conn
        .query_row(
            "SELECT o.id, o.email, s.expires_at FROM sessions s
             JOIN owners o ON o.id = s.owner_id WHERE s.token = ?1",
            [token],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let Some((id, email, expires_at)) = row else {
        return Ok(None);
    };
    if expires_at <= now_iso() {
        conn.execute("DELETE FROM sessions WHERE token = ?1", [token])
            .map_err(db_err)?;
        return Ok(None);
    }
    Ok(Some(Owner { id, email }))
}

pub fn require_owner(conn: &Connection, token: &str) -> Result<Owner, ApiError> {
    owner_from_session(conn, token)?.ok_or_else(ApiError::unauthorized)
}

pub fn delete_session(conn: &Connection, token: &str) -> Result<(), ApiError> {
    conn.execute("DELETE FROM sessions WHERE token = ?1", [token])
        .map_err(db_err)?;
    Ok(())
}

fn league_row(conn: &Connection, league_id: &str) -> Result<Option<Value>, ApiError> {
    let row = conn
        .query_row(
            "SELECT l.id, l.name, l.lineup_lock, l.created_by, l.created_at, l.updated_at,
                    (SELECT COUNT(*) FROM teams t WHERE t.league_id = l.id),
                    (SELECT COUNT(*) FROM divisions d WHERE d.league_id = l.id),
                    l.league_type, l.season_length, l.division_games, l.outside_games,
                    (SELECT COUNT(*) FROM schedule_games g WHERE g.league_id = l.id)
             FROM leagues l WHERE l.id = ?1",
            [league_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, i64>(12)?,
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let Some((
        id,
        name,
        lock,
        created_by,
        created_at,
        updated_at,
        team_count,
        division_count,
        league_type,
        season_length,
        division_games,
        outside_games,
        scheduled_game_count,
    )) = row
    else {
        return Ok(None);
    };
    let (in_bounds, bound_label) = validate::team_bound(team_count);
    let mut divisions = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, league_id, name, created_at, updated_at FROM divisions WHERE league_id = ?1 ORDER BY name")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([league_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "leagueId": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "createdAt": row.get::<_, String>(3)?,
                "updatedAt": row.get::<_, String>(4)?,
            }))
        })
        .map_err(db_err)?;
    for division in rows {
        divisions.push(division.map_err(db_err)?);
    }
    let rivalries = rivalry_rows(conn, league_id)?;
    Ok(Some(json!({
        "id": id,
        "name": name,
        "leagueType": league_type,
        "seasonLength": season_length,
        "divisionGames": division_games,
        "outsideGames": outside_games,
        "lineupLock": lock,
        "createdBy": created_by,
        "teamCount": team_count,
        "divisionCount": division_count,
        "scheduledGameCount": scheduled_game_count,
        "inBounds": in_bounds,
        "boundLabel": bound_label,
        "createdAt": created_at,
        "updatedAt": updated_at,
        "divisions": divisions,
        "rivalries": rivalries,
    })))
}

fn rivalry_rows(conn: &Connection, league_id: &str) -> Result<Vec<Value>, ApiError> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.team_id, ta.name, r.opponent_team_id, tb.name, r.games
             FROM rivalries r
             JOIN teams ta ON ta.id = r.team_id
             JOIN teams tb ON tb.id = r.opponent_team_id
             WHERE r.league_id = ?1
             ORDER BY ta.name, tb.name",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([league_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "teamId": row.get::<_, String>(1)?,
                "teamName": row.get::<_, String>(2)?,
                "opponentTeamId": row.get::<_, String>(3)?,
                "opponentTeamName": row.get::<_, String>(4)?,
                "games": row.get::<_, i64>(5)?,
            }))
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn require_league(conn: &Connection, league_id: &str) -> Result<Value, ApiError> {
    league_row(conn, league_id)?
        .ok_or_else(|| ApiError::not_found("That league is not in the office."))
}

pub fn list_leagues(conn: &Connection, _viewer_id: &str) -> Result<Vec<Value>, ApiError> {
    let ids = {
        let mut stmt = conn
            .prepare("SELECT id FROM leagues ORDER BY name")
            .map_err(db_err)?;
        let mapped = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(db_err)?;
        mapped.collect::<Result<Vec<_>, _>>().map_err(db_err)?
    };
    let mut leagues = Vec::new();
    for id in ids {
        if let Some(league) = league_row(conn, &id)? {
            leagues.push(league);
        }
    }
    Ok(leagues)
}

pub fn create_league(conn: &Connection, viewer_id: &str, body: &Value) -> Result<Value, ApiError> {
    let record = object(body)?;
    let name = validate::required_text(record.get("name"), "League name")?;
    let league_type = validate::parse_league_type(record.get("leagueType"))?;
    let season_length = validate::parse_whole_games(record.get("seasonLength"), "Season length")?;
    let division_games =
        validate::parse_whole_games(record.get("divisionGames"), "Games inside the division")?;
    let outside_games =
        validate::parse_whole_games(record.get("outsideGames"), "Games outside the division")?;
    validate::check_season_counts(season_length, division_games, outside_games)?;
    if let Some(lock) = record.get("lineupLock") {
        let lock = validate::parse_lock(Some(lock))?;
        if lock != validate::lock_for_type(&league_type) {
            return Err(ApiError::bad(
                "Lineup lock follows the league type. ULMG locks by game. NPL locks by week.",
            ));
        }
    }
    if record
        .get("rivalries")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return Err(ApiError::bad(
            "Add the teams before naming rivalry opponents.",
        ));
    }
    let lock = validate::lock_for_type(&league_type);
    let id = new_id();
    let timestamp = now_iso();
    conn.execute(
        "INSERT INTO leagues (id, name, lineup_lock, created_by, created_at, updated_at, league_type, season_length, division_games, outside_games)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, name, lock, viewer_id, timestamp, timestamp, league_type, season_length, division_games, outside_games],
    )
    .map_err(db_err)?;
    require_league(conn, &id)
}

pub fn get_league(conn: &Connection, _viewer_id: &str, league_id: &str) -> Result<Value, ApiError> {
    require_league(conn, league_id)
}

fn json_count(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|number| u32::try_from(number).ok())
}

fn team_divisions(conn: &Connection, league_id: &str) -> Result<Vec<(String, String)>, ApiError> {
    let mut stmt = conn
        .prepare("SELECT id, division_id FROM teams WHERE league_id = ?1 ORDER BY id")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([league_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn stored_rivalries(
    conn: &Connection,
    league_id: &str,
) -> Result<Vec<validate::RivalryInput>, ApiError> {
    let mut stmt = conn
        .prepare("SELECT team_id, opponent_team_id, games FROM rivalries WHERE league_id = ?1 ORDER BY team_id, opponent_team_id")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([league_id], |row| {
            Ok(validate::RivalryInput {
                team_id: row.get(0)?,
                opponent_team_id: row.get(1)?,
                games: row.get::<_, i64>(2)? as u32,
            })
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn ensure_rivalries_fit(
    teams: &[(String, String)],
    division_games: u32,
    outside_games: u32,
    rivalries: &[validate::RivalryInput],
) -> Result<(), ApiError> {
    let divisions: std::collections::HashMap<&str, &str> = teams
        .iter()
        .map(|(id, division)| (id.as_str(), division.as_str()))
        .collect();
    let mut intra: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut inter: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for rivalry in rivalries {
        let Some(left) = divisions.get(rivalry.team_id.as_str()) else {
            return Err(ApiError::bad(
                "Both rivalry opponents have to be teams in this league.",
            ));
        };
        let Some(right) = divisions.get(rivalry.opponent_team_id.as_str()) else {
            return Err(ApiError::bad(
                "Both rivalry opponents have to be teams in this league.",
            ));
        };
        let bucket = if left == right {
            &mut intra
        } else {
            &mut inter
        };
        *bucket.entry(rivalry.team_id.as_str()).or_insert(0) += rivalry.games;
        *bucket.entry(rivalry.opponent_team_id.as_str()).or_insert(0) += rivalry.games;
    }
    if intra.values().any(|games| *games > division_games) {
        return Err(ApiError::bad(
            "A rivalry asks for more games inside the division than the season allows that team.",
        ));
    }
    if inter.values().any(|games| *games > outside_games) {
        return Err(ApiError::bad(
            "A rivalry asks for more games outside the division than the season allows that team.",
        ));
    }
    Ok(())
}

pub fn update_league(
    conn: &mut Connection,
    _viewer_id: &str,
    league_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    let league = require_league(conn, league_id)?;
    let record = object(body)?;
    let name = if record.contains_key("name") {
        validate::required_text(record.get("name"), "League name")?
    } else {
        league["name"].as_str().unwrap_or("").to_string()
    };
    let mut league_type = league["leagueType"].as_str().map(str::to_string);
    if record.contains_key("leagueType") {
        league_type = Some(validate::parse_league_type(record.get("leagueType"))?);
    }
    let mut season_length = json_count(&league["seasonLength"]);
    let mut division_games = json_count(&league["divisionGames"]);
    let mut outside_games = json_count(&league["outsideGames"]);
    let counts_sent = record.contains_key("seasonLength")
        || record.contains_key("divisionGames")
        || record.contains_key("outsideGames");
    if record.contains_key("seasonLength") {
        season_length = Some(validate::parse_whole_games(
            record.get("seasonLength"),
            "Season length",
        )?);
    }
    if record.contains_key("divisionGames") {
        division_games = Some(validate::parse_whole_games(
            record.get("divisionGames"),
            "Games inside the division",
        )?);
    }
    if record.contains_key("outsideGames") {
        outside_games = Some(validate::parse_whole_games(
            record.get("outsideGames"),
            "Games outside the division",
        )?);
    }
    if let (Some(season), Some(inside), Some(outside)) =
        (season_length, division_games, outside_games)
    {
        if counts_sent || record.contains_key("leagueType") {
            validate::check_season_counts(season, inside, outside)?;
        }
    } else if counts_sent {
        return Err(ApiError::bad(
            "Set the season length, the games inside the division, and the games outside it together.",
        ));
    }
    let lock = if let Some(league_type) = league_type.as_deref() {
        let derived = validate::lock_for_type(league_type);
        if let Some(sent) = record.get("lineupLock") {
            let sent = validate::parse_lock(Some(sent))?;
            if sent != derived {
                return Err(ApiError::bad(
                    "Lineup lock follows the league type. ULMG locks by game. NPL locks by week.",
                ));
            }
        }
        derived.to_string()
    } else if record.contains_key("lineupLock") {
        return Err(ApiError::bad(
            "Lineup lock follows the league type. ULMG locks by game. NPL locks by week.",
        ));
    } else {
        league["lineupLock"].as_str().unwrap_or("game").to_string()
    };
    let incoming = if record.contains_key("rivalries") {
        Some(validate::parse_rivalries(&record["rivalries"])?)
    } else {
        None
    };
    let existing = stored_rivalries(conn, league_id)?;
    let effective: &[validate::RivalryInput] = incoming.as_deref().unwrap_or(&existing);
    if !effective.is_empty() {
        let (Some(inside), Some(outside)) = (division_games, outside_games) else {
            return Err(ApiError::bad(
                "Set the season length before naming rivalry opponents.",
            ));
        };
        ensure_rivalries_fit(
            &team_divisions(conn, league_id)?,
            inside,
            outside,
            effective,
        )?;
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute(
        "UPDATE leagues SET name = ?1, lineup_lock = ?2, league_type = ?3, season_length = ?4, division_games = ?5, outside_games = ?6, updated_at = ?7 WHERE id = ?8",
        params![name, lock, league_type, season_length.map(i64::from), division_games.map(i64::from), outside_games.map(i64::from), now_iso(), league_id],
    )
    .map_err(db_err)?;
    if let Some(items) = incoming.as_ref() {
        tx.execute("DELETE FROM rivalries WHERE league_id = ?1", [league_id])
            .map_err(db_err)?;
        let timestamp = now_iso();
        for rivalry in items {
            tx.execute(
                "INSERT INTO rivalries (id, league_id, team_id, opponent_team_id, games, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![new_id(), league_id, rivalry.team_id, rivalry.opponent_team_id, rivalry.games, timestamp],
            )
            .map_err(db_err)?;
        }
    }
    tx.commit().map_err(db_err)?;
    require_league(conn, league_id)
}

pub fn delete_league(
    conn: &mut Connection,
    _viewer_id: &str,
    league_id: &str,
) -> Result<(), ApiError> {
    require_league(conn, league_id)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute(
        "DELETE FROM schedule_games WHERE league_id = ?1",
        [league_id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM schedules WHERE league_id = ?1", [league_id])
        .map_err(db_err)?;
    tx.execute("DELETE FROM rivalries WHERE league_id = ?1", [league_id])
        .map_err(db_err)?;
    tx.execute(
        "DELETE FROM lineups WHERE team_id IN (SELECT id FROM teams WHERE league_id = ?1)",
        [league_id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM teams WHERE league_id = ?1", [league_id])
        .map_err(db_err)?;
    tx.execute("DELETE FROM divisions WHERE league_id = ?1", [league_id])
        .map_err(db_err)?;
    tx.execute("DELETE FROM leagues WHERE id = ?1", [league_id])
        .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(())
}

fn division_json(
    row_id: String,
    league_id: String,
    name: String,
    created_at: String,
    updated_at: String,
) -> Value {
    json!({
        "id": row_id,
        "leagueId": league_id,
        "name": name,
        "createdAt": created_at,
        "updatedAt": updated_at,
    })
}

fn load_division(
    conn: &Connection,
    division_id: &str,
) -> Result<Option<(Value, String)>, ApiError> {
    conn.query_row(
        "SELECT id, league_id, name, created_at, updated_at FROM divisions WHERE id = ?1",
        [division_id],
        |row| {
            let league_id: String = row.get(1)?;
            Ok((
                division_json(
                    row.get(0)?,
                    league_id.clone(),
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ),
                league_id,
            ))
        },
    )
    .optional()
    .map_err(db_err)
}

pub fn list_divisions(
    conn: &Connection,
    _viewer_id: &str,
    league_id: &str,
) -> Result<Vec<Value>, ApiError> {
    let league = require_league(conn, league_id)?;
    Ok(league["divisions"].as_array().cloned().unwrap_or_default())
}

pub fn get_division(
    conn: &Connection,
    _viewer_id: &str,
    division_id: &str,
) -> Result<Value, ApiError> {
    let Some((division, league_id)) = load_division(conn, division_id)? else {
        return Err(ApiError::not_found("That division is not in the office."));
    };
    require_league(conn, &league_id)?;
    Ok(division)
}

pub fn create_division(
    conn: &Connection,
    _viewer_id: &str,
    league_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    require_league(conn, league_id)?;
    let name = validate::required_text(object(body)?.get("name"), "Division name")?;
    if conn
        .query_row(
            "SELECT id FROM divisions WHERE league_id = ?1 AND name = ?2",
            params![league_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This league already has a division with that name.",
        ));
    }
    let id = new_id();
    let timestamp = now_iso();
    conn.execute(
        "INSERT INTO divisions (id, league_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, league_id, name, timestamp, timestamp],
    )
    .map_err(db_err)?;
    Ok(load_division(conn, &id)?.unwrap().0)
}

pub fn update_division(
    conn: &Connection,
    _viewer_id: &str,
    division_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    let Some((_, league_id)) = load_division(conn, division_id)? else {
        return Err(ApiError::not_found("That division is not in the office."));
    };
    require_league(conn, &league_id)?;
    let name = validate::required_text(object(body)?.get("name"), "Division name")?;
    if conn
        .query_row(
            "SELECT id FROM divisions WHERE league_id = ?1 AND name = ?2 AND id != ?3",
            params![league_id, name, division_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This league already has a division with that name.",
        ));
    }
    conn.execute(
        "UPDATE divisions SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now_iso(), division_id],
    )
    .map_err(db_err)?;
    Ok(load_division(conn, division_id)?.unwrap().0)
}

pub fn delete_division(
    conn: &Connection,
    _viewer_id: &str,
    division_id: &str,
) -> Result<(), ApiError> {
    let Some((_, league_id)) = load_division(conn, division_id)? else {
        return Err(ApiError::not_found("That division is not in the office."));
    };
    require_league(conn, &league_id)?;
    let teams = count(
        conn,
        "SELECT COUNT(*) FROM teams WHERE division_id = ?1",
        division_id,
    )?;
    if teams > 0 {
        return Err(ApiError::conflict(
            "Move every team out of this division before deleting it. Deleting the league removes the division with the teams.",
        ));
    }
    conn.execute("DELETE FROM divisions WHERE id = ?1", [division_id])
        .map_err(db_err)?;
    Ok(())
}

const TEAM_SQL: &str = "
    SELECT t.id, t.league_id, l.name, l.lineup_lock, l.created_by, t.division_id, d.name,
           t.owner_id, o.email, t.name, t.home_park,
           (SELECT COUNT(*) FROM lineups lu WHERE lu.team_id = t.id),
           t.created_at, t.updated_at, l.league_type
    FROM teams t
    JOIN leagues l ON l.id = t.league_id
    JOIN divisions d ON d.id = t.division_id
    JOIN owners o ON o.id = t.owner_id
";

fn map_team(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, String>(0)?,
        "leagueId": row.get::<_, String>(1)?,
        "leagueName": row.get::<_, String>(2)?,
        "lineupLock": row.get::<_, String>(3)?,
        "divisionId": row.get::<_, String>(5)?,
        "divisionName": row.get::<_, String>(6)?,
        "ownerId": row.get::<_, String>(7)?,
        "ownerEmail": row.get::<_, String>(8)?,
        "name": row.get::<_, String>(9)?,
        "homePark": row.get::<_, String>(10)?,
        "lineupCount": row.get::<_, i64>(11)?,
        "createdAt": row.get::<_, String>(12)?,
        "updatedAt": row.get::<_, String>(13)?,
        "leagueType": row.get::<_, Option<String>>(14)?,
    }))
}

struct TeamAccess {
    team: Value,
}

fn load_team(conn: &Connection, team_id: &str) -> Result<Option<TeamAccess>, ApiError> {
    conn.query_row(&format!("{TEAM_SQL} WHERE t.id = ?1"), [team_id], |row| {
        Ok(TeamAccess {
            team: map_team(row)?,
        })
    })
    .optional()
    .map_err(db_err)
}

fn visible(_access: &TeamAccess, _viewer_id: &str) -> bool {
    true
}

pub fn list_league_teams(
    conn: &Connection,
    _viewer_id: &str,
    league_id: &str,
) -> Result<Vec<Value>, ApiError> {
    require_league(conn, league_id)?;
    let mut stmt = conn
        .prepare(&format!(
            "{TEAM_SQL} WHERE t.league_id = ?1 ORDER BY t.name"
        ))
        .map_err(db_err)?;
    let rows = stmt.query_map([league_id], map_team).map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

pub fn list_controlled(
    conn: &Connection,
    viewer_id: &str,
) -> Result<(Vec<Value>, Vec<Value>), ApiError> {
    let teams = {
        let mut stmt = conn
            .prepare(&format!("{TEAM_SQL} ORDER BY l.name, t.name"))
            .map_err(db_err)?;
        let mapped = stmt.query_map([], map_team).map_err(db_err)?;
        mapped.collect::<Result<Vec<_>, _>>().map_err(db_err)?
    };
    let leagues = list_leagues(conn, viewer_id)?;
    Ok((teams, leagues))
}

fn division_in_league(
    conn: &Connection,
    league_id: &str,
    division_id: &str,
) -> Result<(), ApiError> {
    let found = conn
        .query_row(
            "SELECT id FROM divisions WHERE id = ?1 AND league_id = ?2",
            params![division_id, league_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?;
    if found.is_none() {
        return Err(ApiError::bad("Choose a division in this league."));
    }
    Ok(())
}

pub fn create_team(
    conn: &mut Connection,
    viewer_id: &str,
    league_id: &str,
    body: &Value,
) -> Result<(Value, i64, bool, String), ApiError> {
    require_league(conn, league_id)?;
    let record = object(body)?;
    let name = validate::required_text(record.get("name"), "Team name")?;
    let home_park = validate::required_text(record.get("homePark"), "Home park")?;
    let division_id = validate::required_text(record.get("divisionId"), "Division")?;
    let supplied = record
        .get("ownerEmail")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let owner_email = if supplied.is_empty() {
        conn.query_row(
            "SELECT email FROM owners WHERE id = ?1",
            [viewer_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(db_err)?
    } else {
        validate::parse_email(record.get("ownerEmail"))?
    };
    division_in_league(conn, league_id, &division_id)?;
    if conn
        .query_row(
            "SELECT id FROM teams WHERE league_id = ?1 AND name = ?2",
            params![league_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This league already has a team with that name.",
        ));
    }
    let owner = upsert_owner(conn, &owner_email)?;
    let id = new_id();
    let timestamp = now_iso();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let team_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM teams WHERE league_id = ?1",
            [league_id],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    if team_count >= 30 {
        return Err(ApiError::conflict("This league already has 30 teams."));
    }
    tx.execute(
        "INSERT INTO teams (id, league_id, division_id, owner_id, name, home_park, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, league_id, division_id, owner.id, name, home_park, timestamp, timestamp],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    let team = load_team(conn, &id)?.unwrap().team;
    let league = require_league(conn, league_id)?;
    Ok((
        team,
        league["teamCount"].as_i64().unwrap_or(0),
        league["inBounds"].as_bool().unwrap_or(false),
        league["boundLabel"].as_str().unwrap_or("").to_string(),
    ))
}

pub fn get_team_page(conn: &Connection, viewer_id: &str, team_id: &str) -> Result<Value, ApiError> {
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That team is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That team is not in the office."));
    }
    let league_id = access.team["leagueId"].as_str().unwrap_or("");
    let league = league_row(conn, league_id)?.unwrap_or(json!({}));
    let lineups = lineup_rows(conn, team_id)?;
    Ok(json!({
        "team": access.team,
        "divisions": league["divisions"].clone(),
        "lineups": lineups,
        "viewerIsAdmin": true,
        "canEditLineups": true,
        "teamCount": league["teamCount"].clone(),
        "inBounds": league["inBounds"].clone(),
        "boundLabel": league["boundLabel"].clone(),
    }))
}

pub fn update_team(
    conn: &Connection,
    viewer_id: &str,
    team_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That team is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That team is not in the office."));
    }
    let record = object(body)?;
    let name = if record.contains_key("name") {
        validate::required_text(record.get("name"), "Team name")?
    } else {
        access.team["name"].as_str().unwrap_or("").to_string()
    };
    let home_park = if record.contains_key("homePark") {
        validate::required_text(record.get("homePark"), "Home park")?
    } else {
        access.team["homePark"].as_str().unwrap_or("").to_string()
    };
    let division_id = if record.contains_key("divisionId") {
        validate::required_text(record.get("divisionId"), "Division")?
    } else {
        access.team["divisionId"].as_str().unwrap_or("").to_string()
    };
    let owner_email = if record.contains_key("ownerEmail") {
        validate::parse_email(record.get("ownerEmail"))?
    } else {
        access.team["ownerEmail"].as_str().unwrap_or("").to_string()
    };
    let league_id = access.team["leagueId"].as_str().unwrap_or("").to_string();
    division_in_league(conn, &league_id, &division_id)?;
    if conn
        .query_row(
            "SELECT id FROM teams WHERE league_id = ?1 AND name = ?2 AND id != ?3",
            params![league_id, name, team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This league already has a team with that name.",
        ));
    }
    let owner = upsert_owner(conn, &owner_email)?;
    conn.execute(
        "UPDATE teams SET name = ?1, home_park = ?2, division_id = ?3, owner_id = ?4, updated_at = ?5 WHERE id = ?6",
        params![name, home_park, division_id, owner.id, now_iso(), team_id],
    )
    .map_err(db_err)?;
    Ok(load_team(conn, team_id)?.unwrap().team)
}

pub fn delete_team(conn: &mut Connection, viewer_id: &str, team_id: &str) -> Result<(), ApiError> {
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That team is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That team is not in the office."));
    }
    let league_id = access.team["leagueId"].as_str().unwrap_or("").to_string();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let team_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM teams WHERE league_id = ?1",
            [&league_id],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    if team_count == 10 {
        return Err(ApiError::conflict(
            "A league that has reached 10 teams stays at 10 or more. Change this team, or delete the league if you need to start over.",
        ));
    }
    tx.execute(
        "DELETE FROM schedule_games WHERE league_id = ?1",
        [&league_id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM schedules WHERE league_id = ?1", [&league_id])
        .map_err(db_err)?;
    tx.execute(
        "DELETE FROM rivalries WHERE team_id = ?1 OR opponent_team_id = ?1",
        [team_id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM lineups WHERE team_id = ?1", [team_id])
        .map_err(db_err)?;
    tx.execute("DELETE FROM teams WHERE id = ?1", [team_id])
        .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(())
}

fn lineup_value(
    id: String,
    team_id: String,
    name: String,
    vs_lhp: String,
    vs_rhp: String,
    created_at: String,
    updated_at: String,
) -> Result<Value, ApiError> {
    let left: Value = serde_json::from_str(&vs_lhp).map_err(|_| ApiError::internal())?;
    let right: Value = serde_json::from_str(&vs_rhp).map_err(|_| ApiError::internal())?;
    Ok(json!({
        "id": id,
        "teamId": team_id,
        "name": name,
        "vsLhp": left,
        "vsRhp": right,
        "createdAt": created_at,
        "updatedAt": updated_at,
    }))
}

fn lineup_rows(conn: &Connection, team_id: &str) -> Result<Vec<Value>, ApiError> {
    let mut stmt = conn
        .prepare("SELECT id, team_id, name, vs_lhp, vs_rhp, created_at, updated_at FROM lineups WHERE team_id = ?1 ORDER BY name")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([team_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(db_err)?;
    let mut lineups = Vec::new();
    for row in rows {
        let (id, team_id, name, left, right, created_at, updated_at) = row.map_err(db_err)?;
        lineups.push(lineup_value(
            id, team_id, name, left, right, created_at, updated_at,
        )?);
    }
    Ok(lineups)
}

fn load_lineup(conn: &Connection, lineup_id: &str) -> Result<Option<Value>, ApiError> {
    let row = conn
        .query_row(
            "SELECT id, team_id, name, vs_lhp, vs_rhp, created_at, updated_at FROM lineups WHERE id = ?1",
            [lineup_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let Some((id, team_id, name, left, right, created_at, updated_at)) = row else {
        return Ok(None);
    };
    Ok(Some(lineup_value(
        id, team_id, name, left, right, created_at, updated_at,
    )?))
}

fn require_lineup_owner<'a>(
    conn: &'a Connection,
    viewer_id: &str,
    team_id: &str,
) -> Result<TeamAccess, ApiError> {
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That team is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That team is not in the office."));
    }
    Ok(access)
}

fn lineup_fields(body: &Value) -> Result<(String, Value, Value), ApiError> {
    let record = object(body)?;
    let name = validate::required_text(record.get("name"), "Lineup name")?;
    let vs_lhp = validate::parse_card(record.get("vsLhp"), "The card versus left-handed pitching")?;
    let vs_rhp =
        validate::parse_card(record.get("vsRhp"), "The card versus right-handed pitching")?;
    Ok((name, Value::Array(vs_lhp), Value::Array(vs_rhp)))
}

pub fn list_lineups(
    conn: &Connection,
    viewer_id: &str,
    team_id: &str,
) -> Result<Vec<Value>, ApiError> {
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That team is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That team is not in the office."));
    }
    lineup_rows(conn, team_id)
}

pub fn create_lineup(
    conn: &Connection,
    viewer_id: &str,
    team_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    require_lineup_owner(conn, viewer_id, team_id)?;
    let (name, left, right) = lineup_fields(body)?;
    if conn
        .query_row(
            "SELECT id FROM lineups WHERE team_id = ?1 AND name = ?2",
            params![team_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This team already has a lineup with that name.",
        ));
    }
    let id = new_id();
    let timestamp = now_iso();
    conn.execute(
        "INSERT INTO lineups (id, team_id, name, vs_lhp, vs_rhp, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, team_id, name, left.to_string(), right.to_string(), timestamp, timestamp],
    )
    .map_err(db_err)?;
    Ok(load_lineup(conn, &id)?.unwrap())
}

pub fn get_lineup_page(
    conn: &Connection,
    viewer_id: &str,
    lineup_id: &str,
) -> Result<Value, ApiError> {
    let Some(lineup) = load_lineup(conn, lineup_id)? else {
        return Err(ApiError::not_found("That lineup is not in the office."));
    };
    let team_id = lineup["teamId"].as_str().unwrap_or("");
    let Some(access) = load_team(conn, team_id)? else {
        return Err(ApiError::not_found("That lineup is not in the office."));
    };
    if !visible(&access, viewer_id) {
        return Err(ApiError::not_found("That lineup is not in the office."));
    }
    Ok(json!({
        "lineup": lineup,
        "team": access.team,
        "canEditLineups": true,
    }))
}

pub fn update_lineup(
    conn: &Connection,
    viewer_id: &str,
    lineup_id: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    let Some(existing) = load_lineup(conn, lineup_id)? else {
        return Err(ApiError::not_found("That lineup is not in the office."));
    };
    let team_id = existing["teamId"].as_str().unwrap_or("").to_string();
    require_lineup_owner(conn, viewer_id, &team_id)?;
    let (name, left, right) = lineup_fields(body)?;
    if conn
        .query_row(
            "SELECT id FROM lineups WHERE team_id = ?1 AND name = ?2 AND id != ?3",
            params![team_id, name, lineup_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "This team already has a lineup with that name.",
        ));
    }
    conn.execute(
        "UPDATE lineups SET name = ?1, vs_lhp = ?2, vs_rhp = ?3, updated_at = ?4 WHERE id = ?5",
        params![
            name,
            left.to_string(),
            right.to_string(),
            now_iso(),
            lineup_id
        ],
    )
    .map_err(db_err)?;
    Ok(load_lineup(conn, lineup_id)?.unwrap())
}

pub fn delete_lineup(conn: &Connection, viewer_id: &str, lineup_id: &str) -> Result<(), ApiError> {
    let Some(existing) = load_lineup(conn, lineup_id)? else {
        return Err(ApiError::not_found("That lineup is not in the office."));
    };
    let team_id = existing["teamId"].as_str().unwrap_or("");
    require_lineup_owner(conn, viewer_id, team_id)?;
    conn.execute("DELETE FROM lineups WHERE id = ?1", [lineup_id])
        .map_err(db_err)?;
    Ok(())
}

fn configuration_snapshot(
    league_type: &str,
    season_length: u32,
    division_games: u32,
    outside_games: u32,
    teams: &[(String, String)],
    rivalries: &[validate::RivalryInput],
) -> String {
    let mut teams = teams.to_vec();
    teams.sort();
    let mut rivalries: Vec<&validate::RivalryInput> = rivalries.iter().collect();
    rivalries.sort_by(|left, right| {
        (&left.team_id, &left.opponent_team_id).cmp(&(&right.team_id, &right.opponent_team_id))
    });
    json!({
        "leagueType": league_type,
        "seasonLength": season_length,
        "divisionGames": division_games,
        "outsideGames": outside_games,
        "teams": teams.iter().map(|(id, division)| json!({"divisionId": division, "id": id})).collect::<Vec<_>>(),
        "rivalries": rivalries.iter().map(|rivalry| json!({
            "games": rivalry.games,
            "opponentTeamId": rivalry.opponent_team_id,
            "teamId": rivalry.team_id,
        })).collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn read_schedule(conn: &Connection, league_id: &str) -> Result<Value, ApiError> {
    let league = require_league(conn, league_id)?;
    let header = conn
        .query_row(
            "SELECT built_at, snapshot FROM schedules WHERE league_id = ?1",
            [league_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(db_err)?;
    let Some((built_at, snapshot)) = header else {
        return Ok(json!({
            "leagueId": league_id,
            "builtAt": null,
            "matchesConfiguration": true,
            "leagueType": league["leagueType"].clone(),
            "seasonLength": league["seasonLength"].clone(),
            "divisionGames": league["divisionGames"].clone(),
            "outsideGames": league["outsideGames"].clone(),
            "games": [],
        }));
    };
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.round, g.home_team_id, th.name, g.away_team_id, ta.name, g.same_division, g.rivalry
             FROM schedule_games g
             JOIN teams th ON th.id = g.home_team_id
             JOIN teams ta ON ta.id = g.away_team_id
             WHERE g.league_id = ?1
             ORDER BY g.round, th.name, ta.name",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([league_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "round": row.get::<_, i64>(1)?,
                "homeTeamId": row.get::<_, String>(2)?,
                "homeTeamName": row.get::<_, String>(3)?,
                "awayTeamId": row.get::<_, String>(4)?,
                "awayTeamName": row.get::<_, String>(5)?,
                "sameDivision": row.get::<_, i64>(6)? == 1,
                "rivalry": row.get::<_, i64>(7)? == 1,
            }))
        })
        .map_err(db_err)?;
    let games = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
    let built: Value = serde_json::from_str(&snapshot).unwrap_or_else(|_| json!({}));
    let current = current_snapshot(conn, league_id)?;
    Ok(json!({
        "leagueId": league_id,
        "builtAt": built_at,
        "matchesConfiguration": current.as_deref() == Some(snapshot.as_str()),
        "leagueType": league["leagueType"].clone(),
        "seasonLength": league["seasonLength"].clone(),
        "divisionGames": league["divisionGames"].clone(),
        "outsideGames": league["outsideGames"].clone(),
        "builtLeagueType": built.get("leagueType").cloned().unwrap_or(Value::Null),
        "builtSeasonLength": built.get("seasonLength").cloned().unwrap_or(Value::Null),
        "builtDivisionGames": built.get("divisionGames").cloned().unwrap_or(Value::Null),
        "builtOutsideGames": built.get("outsideGames").cloned().unwrap_or(Value::Null),
        "games": games,
    }))
}

fn current_snapshot(conn: &Connection, league_id: &str) -> Result<Option<String>, ApiError> {
    let league = require_league(conn, league_id)?;
    let Some(league_type) = league["leagueType"].as_str() else {
        return Ok(None);
    };
    let Some(season_length) = json_count(&league["seasonLength"]) else {
        return Ok(None);
    };
    let Some(division_games) = json_count(&league["divisionGames"]) else {
        return Ok(None);
    };
    let Some(outside_games) = json_count(&league["outsideGames"]) else {
        return Ok(None);
    };
    let teams = team_divisions(conn, league_id)?;
    let rivalries = stored_rivalries(conn, league_id)?;
    Ok(Some(configuration_snapshot(
        league_type,
        season_length,
        division_games,
        outside_games,
        &teams,
        &rivalries,
    )))
}

pub fn build_schedule(conn: &mut Connection, league_id: &str) -> Result<Value, ApiError> {
    let league = require_league(conn, league_id)?;
    let Some(league_type) = league["leagueType"].as_str().map(str::to_string) else {
        return Err(ApiError::bad(
            "Choose a league type before building a schedule.",
        ));
    };
    let Some(season_length) = json_count(&league["seasonLength"]) else {
        return Err(ApiError::bad(
            "Set the season length before building a schedule.",
        ));
    };
    let Some(division_games) = json_count(&league["divisionGames"]) else {
        return Err(ApiError::bad(
            "Set the games inside the division before building a schedule.",
        ));
    };
    let Some(outside_games) = json_count(&league["outsideGames"]) else {
        return Err(ApiError::bad(
            "Set the games outside the division before building a schedule.",
        ));
    };
    validate::check_season_counts(season_length, division_games, outside_games)?;
    let mut teams = team_divisions(conn, league_id)?;
    teams.sort_by(|left, right| left.0.cmp(&right.0));
    let index_of: std::collections::HashMap<&str, usize> = teams
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id.as_str(), index))
        .collect();
    let team_ids: Vec<String> = teams.iter().map(|(id, _)| id.clone()).collect();
    let division_ids: Vec<String> = teams.iter().map(|(_, division)| division.clone()).collect();
    let stored = stored_rivalries(conn, league_id)?;
    ensure_rivalries_fit(&teams, division_games, outside_games, &stored)?;
    let mut rivalries = Vec::new();
    for rivalry in &stored {
        let Some(&left) = index_of.get(rivalry.team_id.as_str()) else {
            return Err(ApiError::bad(
                "Both rivalry opponents have to be teams in this league.",
            ));
        };
        let Some(&right) = index_of.get(rivalry.opponent_team_id.as_str()) else {
            return Err(ApiError::bad(
                "Both rivalry opponents have to be teams in this league.",
            ));
        };
        rivalries.push((left, right, rivalry.games));
    }
    let planned = schedule::build_season(
        &team_ids,
        &division_ids,
        season_length,
        division_games,
        outside_games,
        &rivalries,
    )
    .map_err(ApiError::bad)?;
    let snapshot = configuration_snapshot(
        &league_type,
        season_length,
        division_games,
        outside_games,
        &teams,
        &stored,
    );
    let timestamp = now_iso();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute(
        "DELETE FROM schedule_games WHERE league_id = ?1",
        [league_id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM schedules WHERE league_id = ?1", [league_id])
        .map_err(db_err)?;
    for game in &planned {
        tx.execute(
            "INSERT INTO schedule_games (id, league_id, round, home_team_id, away_team_id, same_division, rivalry, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new_id(),
                league_id,
                game.round,
                team_ids[game.home],
                team_ids[game.away],
                if game.same_division { 1 } else { 0 },
                if game.rivalry { 1 } else { 0 },
                timestamp,
            ],
        )
        .map_err(db_err)?;
    }
    tx.execute(
        "INSERT INTO schedules (league_id, built_at, snapshot) VALUES (?1, ?2, ?3)",
        params![league_id, timestamp, snapshot],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    read_schedule(conn, league_id)
}

pub fn session_counts(conn: &Connection, owner_id: &str) -> Result<(i64, i64), ApiError> {
    let leagues = count(
        conn,
        "SELECT COUNT(*) FROM leagues WHERE created_by = ?1",
        owner_id,
    )?;
    let teams = count(
        conn,
        "SELECT COUNT(*) FROM teams WHERE owner_id = ?1",
        owner_id,
    )?;
    Ok((leagues, teams))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> (std::path::PathBuf, Connection) {
        let path = std::env::temp_dir().join(format!("league-schedule-{}.sqlite", new_id()));
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).expect("open");
        (path, conn)
    }

    fn owner_id(conn: &Connection) -> String {
        office_owner(conn).unwrap().id
    }

    #[test]
    fn build_reads_back_and_a_failed_build_keeps_the_previous_one() {
        let (path, mut conn) = scratch();
        let actor = owner_id(&conn);
        let league = create_league(
            &conn,
            &actor,
            &json!({
                "name": "Harbor ULMG",
                "leagueType": "ulmg",
                "seasonLength": 6,
                "divisionGames": 4,
                "outsideGames": 2
            }),
        )
        .unwrap();
        assert_eq!(league["leagueType"], "ulmg");
        assert_eq!(league["lineupLock"], "game");
        let league_id = league["id"].as_str().unwrap().to_string();
        let east = create_division(&conn, &actor, &league_id, &json!({"name": "East"})).unwrap()
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        let west = create_division(&conn, &actor, &league_id, &json!({"name": "West"})).unwrap()
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        let add = |conn: &mut Connection, division: &str, name: &str| -> String {
            create_team(
                conn,
                &actor,
                &league_id,
                &json!({"name": name, "divisionId": division, "homePark": format!("{name} Park")}),
            )
            .unwrap()
            .0["id"]
                .as_str()
                .unwrap()
                .to_string()
        };
        let east_a = add(&mut conn, &east, "Anchors");
        let east_b = add(&mut conn, &east, "Buoys");
        let west_a = add(&mut conn, &west, "Clippers");
        let _west_b = add(&mut conn, &west, "Dories");
        update_league(
            &mut conn,
            &actor,
            &league_id,
            &json!({
                "rivalries": [
                    {"teamId": east_a, "opponentTeamId": east_b, "games": 4},
                    {"teamId": east_a, "opponentTeamId": west_a, "games": 2}
                ]
            }),
        )
        .unwrap();
        let first = build_schedule(&mut conn, &league_id).unwrap();
        assert_eq!(first["games"].as_array().unwrap().len(), 12);
        assert_eq!(first["matchesConfiguration"], true);
        let first_ids: Vec<String> = first["games"]
            .as_array()
            .unwrap()
            .iter()
            .map(|game| game["id"].as_str().unwrap().to_string())
            .collect();
        let read = read_schedule(&conn, &league_id).unwrap();
        assert_eq!(read["games"].as_array().unwrap().len(), 12);
        let second = build_schedule(&mut conn, &league_id).unwrap();
        let second_ids: Vec<String> = second["games"]
            .as_array()
            .unwrap()
            .iter()
            .map(|game| game["id"].as_str().unwrap().to_string())
            .collect();
        assert!(second_ids.iter().all(|id| !first_ids.contains(id)));

        let narrow = create_league(
            &conn,
            &actor,
            &json!({
                "name": "Narrow",
                "leagueType": "npl",
                "seasonLength": 2,
                "divisionGames": 2,
                "outsideGames": 0
            }),
        )
        .unwrap();
        assert_eq!(narrow["lineupLock"], "week");
        let narrow_id = narrow["id"].as_str().unwrap();
        let only = create_division(&conn, &actor, narrow_id, &json!({"name": "Only"})).unwrap()
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        for name in ["One", "Two", "Three"] {
            create_team(
                &mut conn,
                &actor,
                narrow_id,
                &json!({"name": name, "divisionId": only, "homePark": "Park"}),
            )
            .unwrap();
        }
        let built = build_schedule(&mut conn, narrow_id).unwrap();
        assert_eq!(built["games"].as_array().unwrap().len(), 3);
        let kept: Vec<String> = built["games"]
            .as_array()
            .unwrap()
            .iter()
            .map(|game| game["id"].as_str().unwrap().to_string())
            .collect();
        update_league(
            &mut conn,
            &actor,
            narrow_id,
            &json!({"seasonLength": 1, "divisionGames": 1, "outsideGames": 0}),
        )
        .unwrap();
        let error = build_schedule(&mut conn, narrow_id).unwrap_err();
        assert!(
            error.message.contains("would not pair"),
            "{}",
            error.message
        );
        let still = read_schedule(&conn, narrow_id).unwrap();
        let still_ids: Vec<String> = still["games"]
            .as_array()
            .unwrap()
            .iter()
            .map(|game| game["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(still_ids, kept);
        assert_eq!(still["matchesConfiguration"], false);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn counts_that_do_not_add_up_are_rejected() {
        let (path, conn) = scratch();
        let actor = owner_id(&conn);
        let error = create_league(
            &conn,
            &actor,
            &json!({
                "name": "Short",
                "leagueType": "ulmg",
                "seasonLength": 6,
                "divisionGames": 4,
                "outsideGames": 1
            }),
        )
        .unwrap_err();
        assert!(error.message.contains("add up"), "{}", error.message);
        let _ = std::fs::remove_file(path);
    }
}

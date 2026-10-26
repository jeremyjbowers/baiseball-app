use serde_json::Value;

use crate::error::ApiError;

pub const POSITIONS: [&str; 9] = ["C", "1B", "2B", "3B", "SS", "LF", "CF", "RF", "DH"];

pub fn object(value: &Value) -> Result<&serde_json::Map<String, Value>, ApiError> {
    value
        .as_object()
        .ok_or_else(|| ApiError::bad("The request body must be an object."))
}

pub fn required_text(value: Option<&Value>, label: &str) -> Result<String, ApiError> {
    let Some(value) = value else {
        return Err(ApiError::bad(format!("{label} is required.")));
    };
    let Some(text) = value.as_str() else {
        return Err(ApiError::bad(format!("{label} is required.")));
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad(format!("{label} is required.")));
    }
    if trimmed.chars().count() > 80 {
        return Err(ApiError::bad(format!("{label} must be 80 characters or fewer.")));
    }
    Ok(trimmed.to_string())
}

pub fn parse_email(value: Option<&Value>) -> Result<String, ApiError> {
    let Some(value) = value else {
        return Err(ApiError::bad("Enter an email address."));
    };
    let Some(text) = value.as_str() else {
        return Err(ApiError::bad("Enter an email address."));
    };
    let email = text.trim().to_lowercase();
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or("");
    let host = parts.next().unwrap_or("");
    let extra = parts.next();
    let host_dot = host.find('.');
    let ok = extra.is_none()
        && !local.is_empty()
        && !host.is_empty()
        && !local.contains(char::is_whitespace)
        && !host.contains(char::is_whitespace)
        && host_dot.is_some_and(|index| index > 0 && index + 1 < host.len())
        && email.chars().count() <= 160;
    if !ok {
        return Err(if email.chars().count() > 160 {
            ApiError::bad("That email address is too long.")
        } else {
            ApiError::bad("Enter an email address.")
        });
    }
    Ok(email)
}

pub fn parse_lock(value: Option<&Value>) -> Result<String, ApiError> {
    let Some(value) = value.and_then(Value::as_str) else {
        return Err(ApiError::bad(
            "Choose when lineups lock: game, week, half, or series.",
        ));
    };
    if !matches!(value, "game" | "week" | "half" | "series") {
        return Err(ApiError::bad(
            "Choose when lineups lock: game, week, half, or series.",
        ));
    }
    Ok(value.to_string())
}

pub fn team_bound(count: i64) -> (bool, String) {
    if count < 10 {
        return (
            false,
            format!("{count} teams. A league needs at least 10, and no more than 30."),
        );
    }
    if count == 26 || count == 28 {
        return (
            false,
            format!("{count} teams. 26 and 28 are closed counts. Add or remove a team."),
        );
    }
    if count > 30 {
        return (
            false,
            format!("{count} teams. A league cannot have more than 30."),
        );
    }
    (
        true,
        format!("{count} teams. Inside the 10 to 30 bound."),
    )
}

pub fn parse_card(value: Option<&Value>, label: &str) -> Result<Vec<Value>, ApiError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(ApiError::bad(format!("{label} needs nine batters.")));
    };
    if items.len() != 9 {
        return Err(ApiError::bad(format!("{label} needs nine batters.")));
    }
    let mut slots = Vec::with_capacity(9);
    for (index, entry) in items.iter().enumerate() {
        let Some(record) = entry.as_object() else {
            return Err(ApiError::bad(format!(
                "{label}, batting order {}, is incomplete.",
                index + 1
            )));
        };
        let order = record
            .get("battingOrder")
            .and_then(Value::as_i64)
            .filter(|order| (1..=9).contains(order))
            .ok_or_else(|| ApiError::bad(format!("{label} must use batting order 1 through 9.")))?;
        let player_name = required_text(
            record.get("playerName"),
            &format!("{label}, batting order {order}, player name"),
        )?;
        let position = record
            .get("position")
            .and_then(Value::as_str)
            .filter(|position| POSITIONS.contains(position))
            .ok_or_else(|| {
                ApiError::bad(format!(
                    "{label}, batting order {order}, needs a defensive position."
                ))
            })?;
        slots.push(serde_json::json!({
            "battingOrder": order,
            "playerName": player_name,
            "position": position,
        }));
    }
    slots.sort_by_key(|slot| slot["battingOrder"].as_i64().unwrap_or(0));
    let mut orders = slots
        .iter()
        .map(|slot| slot["battingOrder"].as_i64().unwrap_or(0))
        .collect::<Vec<_>>();
    orders.sort_unstable();
    orders.dedup();
    if orders.len() != 9 {
        return Err(ApiError::bad(format!(
            "{label} must use each batting order from 1 through 9 once."
        )));
    }
    let mut positions = slots
        .iter()
        .filter_map(|slot| slot["position"].as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    positions.sort();
    positions.dedup();
    if positions.len() != 9 {
        return Err(ApiError::bad(format!(
            "{label} must use each defensive position once."
        )));
    }
    let mut names = slots
        .iter()
        .filter_map(|slot| slot["playerName"].as_str())
        .map(|name| name.to_lowercase())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    if names.len() != 9 {
        return Err(ApiError::bad(format!(
            "{label} lists the same player more than once."
        )));
    }
    Ok(slots)
}

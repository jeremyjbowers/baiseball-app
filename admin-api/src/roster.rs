//! Roster CSV. One row is one team. League / division / team are created from the columns.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::error::ApiError;
use crate::validate::{self, MAX_SEASON_GAMES};

#[derive(Debug)]
pub struct RosterTeam {
    pub line: usize,
    pub subleague: String,
    pub division: String,
    pub name: String,
    pub home_park: String,
    pub managers: Vec<String>,
}

#[derive(Debug)]
pub struct RosterRivalry {
    pub line: usize,
    pub team: String,
    pub opponent: String,
    pub games: u32,
}

#[derive(Debug)]
pub struct Roster {
    pub teams: Vec<RosterTeam>,
    pub rivalries: Vec<RosterRivalry>,
}

pub fn fold_key(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn parse(text: &str) -> Result<Roster, ApiError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let records = split_records(text)?;
    if records.is_empty() {
        return Err(ApiError::bad(
            "The roster file has no teams. Keep the header row and add one row per team.",
        ));
    }
    let (_, header) = &records[0];
    let mut columns: HashMap<&'static str, usize> = HashMap::new();
    for (index, cell) in header.iter().enumerate() {
        let Some(name) = canonical_header(cell) else {
            continue;
        };
        if columns.insert(name, index).is_some() {
            return Err(ApiError::bad(format!(
                "The roster has two columns for {}. Download the template and keep a single header row.",
                column_label(name)
            )));
        }
    }
    let Some(&league_index) = columns.get("league") else {
        return Err(missing_header());
    };
    let Some(&division_index) = columns.get("division") else {
        return Err(missing_header());
    };
    let Some(&team_index) = columns.get("team") else {
        return Err(missing_header());
    };
    let Some(&park_index) = columns.get("home_park") else {
        return Err(missing_header());
    };
    let managers_index = columns.get("managers").copied();
    let rivalries_index = columns.get("rivalries").copied();

    let mut teams = Vec::new();
    let mut seen_names = HashSet::new();
    let mut pending = Vec::new();
    for (line, row) in records.iter().skip(1) {
        let subleague = text_field(*line, "League name", cell_at(row, Some(league_index)))?;
        let division = text_field(*line, "Division name", cell_at(row, Some(division_index)))?;
        let name = text_field(*line, "Team name", cell_at(row, Some(team_index)))?;
        let home_park = text_field(*line, "Home park", cell_at(row, Some(park_index)))?;
        let managers = parse_manager_cell(*line, cell_at(row, managers_index))?;
        if !seen_names.insert(fold_key(&name)) {
            return Err(ApiError::bad(format!(
                "Row {line}: {name} is listed more than once."
            )));
        }
        if let Some(index) = rivalries_index {
            pending.extend(parse_rivalry_cell(*line, &name, cell_at(row, Some(index)))?);
        }
        teams.push(RosterTeam {
            line: *line,
            subleague,
            division,
            name,
            home_park,
            managers,
        });
    }
    if teams.is_empty() {
        return Err(ApiError::bad(
            "The roster file has no teams. Keep the header row and add one row per team.",
        ));
    }
    Ok(Roster {
        teams,
        rivalries: dedupe_rivalries(pending)?,
    })
}

fn missing_header() -> ApiError {
    ApiError::bad(
        "The roster needs a header row with league, division, team, and home_park. managers and rivalries are optional. Download the template and keep that header.",
    )
}

fn canonical_header(raw: &str) -> Option<&'static str> {
    let key = fold_key(raw).replace(' ', "_").replace('-', "_");
    match key.as_str() {
        "league" | "league_name" | "subleague" | "sub_league" => Some("league"),
        "division" | "division_name" => Some("division"),
        "team" | "team_name" => Some("team"),
        "home_park" | "homepark" | "park" => Some("home_park"),
        "managers" | "manager" | "manager_emails" | "manager_email" | "emails" => Some("managers"),
        "rivalries" | "rivalry" => Some("rivalries"),
        _ => None,
    }
}

fn column_label(name: &str) -> &str {
    match name {
        "home_park" => "home park",
        other => other,
    }
}

fn cell_at(row: &[String], index: Option<usize>) -> &str {
    index
        .and_then(|index| row.get(index))
        .map(String::as_str)
        .unwrap_or("")
}

fn text_field(line: usize, label: &str, value: &str) -> Result<String, ApiError> {
    validate::required_text(Some(&Value::String(value.to_string())), label)
        .map_err(|err| ApiError::bad(format!("Row {line}: {}", err.message)))
}

fn parse_manager_cell(line: usize, cell: &str) -> Result<Vec<String>, ApiError> {
    let mut emails = Vec::new();
    for piece in cell.split(|ch| ch == ';' || ch == ',') {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let email = validate::parse_email(Some(&Value::String(piece.to_string())))
            .map_err(|_| ApiError::bad(format!("Row {line}: {piece} is not an email address.")))?;
        if !emails.contains(&email) {
            emails.push(email);
        }
    }
    if emails.len() > 8 {
        return Err(ApiError::bad(format!(
            "Row {line}: a team can have at most 8 managers."
        )));
    }
    Ok(emails)
}

fn parse_rivalry_cell(
    line: usize,
    team: &str,
    cell: &str,
) -> Result<Vec<(usize, String, String, u32)>, ApiError> {
    let mut pairs = Vec::new();
    for piece in cell.split(';') {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let Some((name, games)) = piece.rsplit_once(':') else {
            return Err(ApiError::bad(format!(
                "Row {line}: a rivalry looks like Wharf:4."
            )));
        };
        let opponent = text_field(line, "Rivalry opponent", name.trim())?;
        if fold_key(team) == fold_key(&opponent) {
            return Err(ApiError::bad(format!(
                "Row {line}: {team} cannot be its own rival."
            )));
        }
        let games = games.trim();
        if games.is_empty() || !games.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(ApiError::bad(format!(
                "Row {line}: rivalry games have to be a whole number."
            )));
        }
        let games: u32 = games.parse().map_err(|_| {
            ApiError::bad(format!(
                "Row {line}: rivalry games have to be a whole number."
            ))
        })?;
        if games < 1 {
            return Err(ApiError::bad(format!(
                "Row {line}: say how many games the rivalry plays."
            )));
        }
        if games > MAX_SEASON_GAMES {
            return Err(ApiError::bad(format!(
                "Row {line}: rivalry games must be {MAX_SEASON_GAMES} or fewer."
            )));
        }
        pairs.push((line, team.to_string(), opponent, games));
    }
    Ok(pairs)
}

fn dedupe_rivalries(
    pending: Vec<(usize, String, String, u32)>,
) -> Result<Vec<RosterRivalry>, ApiError> {
    let mut order = Vec::new();
    let mut games_for: HashMap<(String, String), (u32, usize, String, String)> = HashMap::new();
    for (line, team, opponent, games) in pending {
        let left = fold_key(&team);
        let right = fold_key(&opponent);
        let key = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if let Some((previous, previous_line, _, _)) = games_for.get(&key) {
            if *previous != games {
                return Err(ApiError::bad(format!(
                    "Row {line}: {team} and {opponent} disagree on the rivalry games. Row {previous_line} says {previous} and this row says {games}."
                )));
            }
            continue;
        }
        games_for.insert(key.clone(), (games, line, team, opponent));
        order.push(key);
    }
    Ok(order
        .into_iter()
        .filter_map(|key| {
            games_for
                .remove(&key)
                .map(|(games, line, team, opponent)| RosterRivalry {
                    line,
                    team,
                    opponent,
                    games,
                })
        })
        .collect())
}

fn split_records(text: &str) -> Result<Vec<(usize, Vec<String>)>, ApiError> {
    let mut records = Vec::new();
    let mut field = String::new();
    let mut record = Vec::new();
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut record_line = 1usize;
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if in_quotes {
            if ch == '"' {
                if chars.get(index + 1) == Some(&'"') {
                    field.push('"');
                    index += 2;
                    continue;
                }
                in_quotes = false;
                index += 1;
                continue;
            }
            field.push(ch);
            if ch == '\n' {
                line += 1;
            }
            index += 1;
            continue;
        }
        match ch {
            '"' if field.trim().is_empty() => {
                field.clear();
                in_quotes = true;
                index += 1;
            }
            '"' => {
                return Err(ApiError::bad(format!(
                    "Row {record_line}: a quote is in the wrong place."
                )));
            }
            ',' => {
                record.push(std::mem::take(&mut field));
                index += 1;
            }
            '\n' | '\r' => {
                if ch == '\r' && chars.get(index + 1) == Some(&'\n') {
                    index += 1;
                }
                record.push(std::mem::take(&mut field));
                push_record(&mut records, record_line, &mut record);
                index += 1;
                line += 1;
                record_line = line;
            }
            _ => {
                field.push(ch);
                index += 1;
            }
        }
    }
    if in_quotes {
        return Err(ApiError::bad(format!(
            "Row {record_line}: a quoted value is missing its closing quote."
        )));
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        push_record(&mut records, record_line, &mut record);
    }
    Ok(records)
}

fn push_record(records: &mut Vec<(usize, Vec<String>)>, line: usize, record: &mut Vec<String>) {
    let trimmed: Vec<String> = record.iter().map(|item| item.trim().to_string()).collect();
    record.clear();
    if trimmed.iter().any(|item| !item.is_empty()) {
        records.push((line, trimmed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_has_four_teams_two_managers_and_one_rivalry() {
        let roster = parse(include_str!("../../web/public/league-roster-template.csv")).unwrap();
        assert_eq!(roster.teams.len(), 4);
        assert_eq!(
            roster.teams[0].managers,
            ["ada@league.test", "bea@league.test"]
        );
        assert_eq!(roster.teams[0].subleague, "National League");
        assert_eq!(roster.teams[0].division, "West");
        assert_eq!(roster.teams[2].subleague, "American League");
        assert_eq!(roster.teams[3].managers.len(), 0);
        assert_eq!(roster.teams[3].name, "Dune Foxes");
        assert_eq!(roster.rivalries.len(), 1);
        assert_eq!(roster.rivalries[0].games, 4);
        assert_eq!(fold_key(&roster.rivalries[0].team), "harbor hawks");
        assert_eq!(fold_key(&roster.rivalries[0].opponent), "wharf cats");
    }

    #[test]
    fn quoted_park_names_and_blank_lines_parse() {
        let roster = parse("\u{feff}league,division,team,home_park,managers\r\nNational League,West,Harbor,\"Oracle Park, San Francisco\",ada@league.test\r\n\r\n").unwrap();
        assert_eq!(roster.teams.len(), 1);
        assert_eq!(roster.teams[0].home_park, "Oracle Park, San Francisco");
    }

    #[test]
    fn missing_header_disagreement_and_duplicate_team_are_row_errors() {
        let missing = parse("division,team,home_park\nEast,Harbor,Park\n").unwrap_err();
        assert!(missing.message.contains("header"), "{}", missing.message);
        let duplicate = parse(
            "league,division,team,home_park\nNational League,West,Harbor,Park\nNational League,West,Harbor,Other\n",
        )
        .unwrap_err();
        assert!(
            duplicate.message.contains("listed more than once"),
            "{}",
            duplicate.message
        );
        let disagree = parse(
            "league,division,team,home_park,rivalries\nNational League,West,Harbor,Park,Wharf:4\nNational League,West,Wharf,Park,Harbor:6\n",
        )
        .unwrap_err();
        assert!(
            disagree.message.contains("disagree"),
            "{}",
            disagree.message
        );
        let bad_email = parse(
            "league,division,team,home_park,managers\nNational League,West,Harbor,Park,not-an-email\n",
        )
        .unwrap_err();
        assert!(
            bad_email.message.contains("not an email"),
            "{}",
            bad_email.message
        );
    }
}

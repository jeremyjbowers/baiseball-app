//! Season builder. Counts and rivalry pairs in, games out.
//! A request that cannot be paired is an error. Nothing is shortened or dropped to make it fit.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedGame {
    pub round: u32,
    pub home: usize,
    pub away: usize,
    pub same_division: bool,
    pub rivalry: bool,
}

pub fn build_season(
    team_ids: &[String],
    division_ids: &[String],
    season_length: u32,
    division_games: u32,
    outside_games: u32,
    rivalries: &[(usize, usize, u32)],
) -> Result<Vec<PlannedGame>, String> {
    if team_ids.len() != division_ids.len() {
        return Err("The schedule is missing a division for a team.".into());
    }
    if season_length < 1 {
        return Err("Season length must be at least 1 game.".into());
    }
    if division_games + outside_games != season_length {
        return Err("Division games and outside games must add up to the season length.".into());
    }
    let n = team_ids.len();
    if n < 2 {
        return Err("Add at least two teams before building a schedule.".into());
    }

    let mut division_keys: Vec<&str> = division_ids.iter().map(String::as_str).collect();
    division_keys.sort_unstable();
    division_keys.dedup();
    let division_of: Vec<usize> = division_ids
        .iter()
        .map(|id| {
            division_keys
                .iter()
                .position(|key| *key == id.as_str())
                .expect("division key")
        })
        .collect();
    let mut groups = vec![Vec::new(); division_keys.len()];
    for (index, &division) in division_of.iter().enumerate() {
        groups[division].push(index);
    }

    if division_games > 0 {
        for group in &groups {
            if group.len() < 2 {
                return Err(
                    "A team in a division by itself cannot play games inside the division.".into(),
                );
            }
            let slots = group.len() as u32 * division_games;
            if slots % 2 != 0 {
                return Err(format!(
                    "A division of {} teams cannot give every team {} games inside the division. Those games would not pair.",
                    group.len(),
                    division_games
                ));
            }
        }
    }
    if outside_games > 0 {
        let occupied = groups.iter().filter(|group| !group.is_empty()).count();
        if occupied < 2 {
            return Err("Games outside the division need a team in another division.".into());
        }
        let total = n as u32 * outside_games;
        for group in &groups {
            if group.is_empty() {
                continue;
            }
            let stubs = group.len() as u32 * outside_games;
            let elsewhere = total - stubs;
            if stubs > elsewhere {
                return Err(
                    "The teams outside a division do not have enough games to face everyone inside it.".into(),
                );
            }
        }
        if total % 2 != 0 {
            return Err("Games outside the division cannot be paired.".into());
        }
    }

    let normalized =
        normalize_rivalries(n, division_ids, division_games, outside_games, rivalries)?;

    let mut division_left = vec![division_games; n];
    let mut outside_left = vec![outside_games; n];
    let mut pairs: Vec<(usize, usize, bool)> = Vec::new();
    for (a, b, games) in normalized {
        let inside = division_ids[a] == division_ids[b];
        for _ in 0..games {
            if inside {
                division_left[a] -= 1;
                division_left[b] -= 1;
            } else {
                outside_left[a] -= 1;
                outside_left[b] -= 1;
            }
            pairs.push((a.min(b), a.max(b), true));
        }
    }

    for group in &groups {
        let local: Vec<u32> = group.iter().map(|&index| division_left[index]).collect();
        if local.iter().all(|left| *left == 0) {
            continue;
        }
        let edges = pair_degrees(&local).map_err(|_| {
            "Games inside the division cannot be paired once the rivalries are placed.".to_string()
        })?;
        for (left, right) in edges {
            let a = group[left];
            let b = group[right];
            division_left[a] -= 1;
            division_left[b] -= 1;
            pairs.push((a.min(b), a.max(b), false));
        }
    }
    if division_left.iter().any(|left| *left != 0) {
        return Err(
            "Games inside the division cannot be paired once the rivalries are placed.".into(),
        );
    }

    if outside_left.iter().any(|left| *left != 0) {
        let edges = pair_outside(&division_of, &outside_left)?;
        for (a, b) in edges {
            pairs.push((a.min(b), a.max(b), false));
        }
    }

    let undirected: Vec<Undirected> = pairs
        .into_iter()
        .map(|(a, b, rivalry)| Undirected {
            a,
            b,
            rivalry,
            round: 0,
        })
        .collect();
    let placed = assign_rounds(undirected)?;
    let planned = assign_homes(placed, division_ids);

    verify(
        &planned,
        n,
        season_length,
        division_games,
        outside_games,
        rivalries,
        division_ids,
    )?;
    Ok(planned)
}

fn normalize_rivalries(
    teams: usize,
    division_ids: &[String],
    division_games: u32,
    outside_games: u32,
    rivalries: &[(usize, usize, u32)],
) -> Result<Vec<(usize, usize, u32)>, String> {
    let mut seen = HashSet::new();
    let mut intra = vec![0u32; teams];
    let mut inter = vec![0u32; teams];
    let mut normalized = Vec::with_capacity(rivalries.len());
    for &(a, b, games) in rivalries {
        if a == b {
            return Err("A team cannot be its own rival.".into());
        }
        if a >= teams || b >= teams {
            return Err("A rivalry names a team that is not in this league.".into());
        }
        if games < 1 {
            return Err("Say how many games the rivalry plays.".into());
        }
        let (lo, hi) = (a.min(b), a.max(b));
        if !seen.insert((lo, hi)) {
            return Err("Name each rivalry once.".into());
        }
        if division_ids[a] == division_ids[b] {
            intra[a] = intra[a].saturating_add(games);
            intra[b] = intra[b].saturating_add(games);
        } else {
            inter[a] = inter[a].saturating_add(games);
            inter[b] = inter[b].saturating_add(games);
        }
        normalized.push((lo, hi, games));
    }
    if intra.iter().any(|games| *games > division_games) {
        return Err(
            "A rivalry asks for more games inside the division than the season allows that team."
                .into(),
        );
    }
    if inter.iter().any(|games| *games > outside_games) {
        return Err(
            "A rivalry asks for more games outside the division than the season allows that team."
                .into(),
        );
    }
    Ok(normalized)
}

/// Loopless multigraph with the given degrees. Sum must be even, and no degree may exceed the others combined.
fn pair_degrees(degree: &[u32]) -> Result<Vec<(usize, usize)>, String> {
    let mut degree = degree.to_vec();
    let mut edges = Vec::new();
    let mut guard = degree.iter().sum::<u32>() + 1;
    loop {
        if guard == 0 {
            return Err("The games cannot be paired.".into());
        }
        guard -= 1;
        let total: u32 = degree.iter().sum();
        if total == 0 {
            return Ok(edges);
        }
        if total % 2 != 0 {
            return Err("The games cannot be paired.".into());
        }
        let mut order: Vec<usize> = (0..degree.len()).collect();
        order.sort_by(|&left, &right| degree[right].cmp(&degree[left]).then(left.cmp(&right)));
        let first = order[0];
        let rest = total - degree[first];
        if degree[first] > rest {
            return Err("The games cannot be paired.".into());
        }
        let second = *order.get(1).ok_or("The games cannot be paired.")?;
        if degree[second] == 0 {
            return Err("The games cannot be paired.".into());
        }
        degree[first] -= 1;
        degree[second] -= 1;
        edges.push((first.min(second), first.max(second)));
    }
}

fn pair_outside(division_of: &[usize], remaining: &[u32]) -> Result<Vec<(usize, usize)>, String> {
    let mut remaining = remaining.to_vec();
    let mut edges = Vec::new();
    let mut guard = remaining.iter().sum::<u32>() + 1;
    loop {
        if guard == 0 {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        }
        guard -= 1;
        let total: u32 = remaining.iter().sum();
        if total == 0 {
            return Ok(edges);
        }
        if total % 2 != 0 {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        }
        let mut stubs: HashMap<usize, u32> = HashMap::new();
        for (index, &left) in remaining.iter().enumerate() {
            *stubs.entry(division_of[index]).or_insert(0) += left;
        }
        for (&_division, &own) in &stubs {
            if own > total - own {
                return Err(
                    "Games outside the division cannot be paired once the rivalries are placed."
                        .into(),
                );
            }
        }
        let tight: Vec<usize> = stubs
            .iter()
            .filter(|(_, own)| **own > 0 && total - *own == **own)
            .map(|(division, _)| *division)
            .collect();
        let source = if tight.is_empty() {
            stubs
                .iter()
                .filter(|(_, own)| **own > 0)
                .max_by_key(|(division, own)| (*own, std::cmp::Reverse(**division)))
                .map(|(division, _)| *division)
        } else {
            tight
                .into_iter()
                .max_by_key(|division| (stubs[division], std::cmp::Reverse(*division)))
        };
        let Some(source) = source else {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        };
        let opponent = stubs
            .iter()
            .filter(|(division, own)| **division != source && **own > 0)
            .min_by_key(|(division, own)| {
                let slack = total - 2 * *own;
                (slack, std::cmp::Reverse(**own), **division)
            })
            .map(|(division, _)| *division);
        let Some(opponent) = opponent else {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        };
        let Some(left) = pick_team(&remaining, division_of, source) else {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        };
        let Some(right) = pick_team(&remaining, division_of, opponent) else {
            return Err(
                "Games outside the division cannot be paired once the rivalries are placed.".into(),
            );
        };
        remaining[left] -= 1;
        remaining[right] -= 1;
        edges.push((left.min(right), left.max(right)));
    }
}

fn pick_team(remaining: &[u32], division_of: &[usize], division: usize) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (index, &left) in remaining.iter().enumerate() {
        if division_of[index] != division || left == 0 {
            continue;
        }
        best = Some(match best {
            None => index,
            Some(current) => {
                if left > remaining[current] || (left == remaining[current] && index < current) {
                    index
                } else {
                    current
                }
            }
        });
    }
    best
}

struct Undirected {
    a: usize,
    b: usize,
    rivalry: bool,
    round: u32,
}

fn assign_rounds(mut games: Vec<Undirected>) -> Result<Vec<Undirected>, String> {
    let mut pending: Vec<usize> = (0..games.len()).collect();
    let mut round = 1u32;
    let limit = games.len() as u32 + 2;
    while !pending.is_empty() {
        if round > limit {
            return Err("The schedule could not be placed into rounds.".into());
        }
        let mut team_load: HashMap<usize, u32> = HashMap::new();
        for &index in &pending {
            *team_load.entry(games[index].a).or_insert(0) += 1;
            *team_load.entry(games[index].b).or_insert(0) += 1;
        }
        let mut order = pending.clone();
        order.sort_by(|&left, &right| {
            let left_weight = team_load[&games[left].a] + team_load[&games[left].b];
            let right_weight = team_load[&games[right].a] + team_load[&games[right].b];
            right_weight
                .cmp(&left_weight)
                .then(games[left].a.cmp(&games[right].a))
                .then(games[left].b.cmp(&games[right].b))
                .then(left.cmp(&right))
        });
        let mut used = HashSet::new();
        let mut next = Vec::new();
        let mut taken = 0;
        for index in order {
            let (a, b) = (games[index].a, games[index].b);
            if used.contains(&a) || used.contains(&b) {
                next.push(index);
            } else {
                used.insert(a);
                used.insert(b);
                games[index].round = round;
                taken += 1;
            }
        }
        if taken == 0 {
            return Err("The schedule could not be placed into rounds.".into());
        }
        pending = next;
        round += 1;
    }
    Ok(games)
}

fn assign_homes(games: Vec<Undirected>, division_ids: &[String]) -> Vec<PlannedGame> {
    let mut groups: HashMap<(usize, usize), Vec<Undirected>> = HashMap::new();
    for game in games {
        groups.entry((game.a, game.b)).or_default().push(game);
    }
    let mut keys: Vec<(usize, usize)> = groups.keys().copied().collect();
    keys.sort_unstable();
    let mut planned = Vec::new();
    for key in keys {
        let mut pair = groups.remove(&key).unwrap_or_default();
        pair.sort_by(|left, right| {
            left.round
                .cmp(&right.round)
                .then(left.rivalry.cmp(&right.rivalry).reverse())
        });
        for (index, game) in pair.into_iter().enumerate() {
            let (home, away) = if index % 2 == 0 {
                (game.a, game.b)
            } else {
                (game.b, game.a)
            };
            planned.push(PlannedGame {
                round: game.round,
                home,
                away,
                same_division: division_ids[home] == division_ids[away],
                rivalry: game.rivalry,
            });
        }
    }
    planned.sort_by(|left, right| {
        left.round
            .cmp(&right.round)
            .then(left.home.cmp(&right.home))
            .then(left.away.cmp(&right.away))
    });
    planned
}

fn verify(
    games: &[PlannedGame],
    teams: usize,
    season_length: u32,
    division_games: u32,
    outside_games: u32,
    rivalries: &[(usize, usize, u32)],
    division_ids: &[String],
) -> Result<(), String> {
    let mut played = vec![0u32; teams];
    let mut inside = vec![0u32; teams];
    let mut outside = vec![0u32; teams];
    let mut rounds: HashMap<u32, HashSet<usize>> = HashMap::new();
    for game in games {
        if game.home == game.away {
            return Err("The schedule could not be placed into rounds.".into());
        }
        let round = rounds.entry(game.round).or_default();
        if !round.insert(game.home) || !round.insert(game.away) {
            return Err("The schedule could not be placed into rounds.".into());
        }
        played[game.home] += 1;
        played[game.away] += 1;
        if game.same_division {
            inside[game.home] += 1;
            inside[game.away] += 1;
        } else {
            outside[game.home] += 1;
            outside[game.away] += 1;
        }
        if game.same_division != (division_ids[game.home] == division_ids[game.away]) {
            return Err("The schedule could not be placed into rounds.".into());
        }
    }
    if played.iter().any(|count| *count != season_length)
        || inside.iter().any(|count| *count != division_games)
        || outside.iter().any(|count| *count != outside_games)
    {
        return Err("The schedule does not meet the season length.".into());
    }
    for &(a, b, expected) in rivalries {
        let (lo, hi) = (a.min(b), a.max(b));
        let marked = games
            .iter()
            .filter(|game| {
                let pair = (game.home.min(game.away), game.home.max(game.away));
                pair == (lo, hi) && game.rivalry
            })
            .count() as u32;
        if marked != expected {
            return Err("The schedule does not meet a rivalry.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|index| format!("t{index:02}")).collect()
    }

    fn assert_exact(
        games: &[PlannedGame],
        teams: usize,
        season: u32,
        division_games: u32,
        outside_games: u32,
        rivalries: &[(usize, usize, u32)],
    ) {
        let mut played = vec![0u32; teams];
        let mut inside = vec![0u32; teams];
        let mut outside = vec![0u32; teams];
        let mut seen: HashMap<u32, HashSet<usize>> = HashMap::new();
        for game in games {
            assert!(seen.entry(game.round).or_default().insert(game.home));
            assert!(seen.get_mut(&game.round).unwrap().insert(game.away));
            played[game.home] += 1;
            played[game.away] += 1;
            if game.same_division {
                inside[game.home] += 1;
                inside[game.away] += 1;
            } else {
                outside[game.home] += 1;
                outside[game.away] += 1;
            }
        }
        assert!(played.iter().all(|count| *count == season));
        assert!(inside.iter().all(|count| *count == division_games));
        assert!(outside.iter().all(|count| *count == outside_games));
        for &(a, b, expected) in rivalries {
            let marked = games
                .iter()
                .filter(|game| {
                    let pair = (game.home.min(game.away), game.home.max(game.away));
                    pair == (a.min(b), a.max(b)) && game.rivalry
                })
                .count() as u32;
            assert_eq!(marked, expected);
        }
    }

    #[test]
    fn two_teams_split_home_and_away() {
        let teams = ids(2);
        let divisions = vec!["east".to_string(); 2];
        let games = build_season(&teams, &divisions, 3, 3, 0, &[]).unwrap();
        assert_exact(&games, 2, 3, 3, 0, &[]);
        let homes_for_first = games.iter().filter(|game| game.home == 0).count();
        assert_eq!(homes_for_first, 2);
        assert_eq!(games.len(), 3);
    }

    #[test]
    fn four_team_division_plays_a_round_robin() {
        let teams = ids(4);
        let divisions = vec!["only".to_string(); 4];
        let games = build_season(&teams, &divisions, 3, 3, 0, &[]).unwrap();
        assert_exact(&games, 4, 3, 3, 0, &[]);
        assert!(games.iter().all(|game| game.same_division));
    }

    #[test]
    fn odd_division_count_is_rejected() {
        let teams = ids(3);
        let divisions = vec!["only".to_string(); 3];
        let error = build_season(&teams, &divisions, 1, 1, 0, &[]).unwrap_err();
        assert!(error.contains("would not pair"), "{error}");
    }

    #[test]
    fn ulmg_shape_honors_inside_outside_and_rivalries() {
        let teams = ids(4);
        let divisions = vec![
            "east".to_string(),
            "east".to_string(),
            "west".to_string(),
            "west".to_string(),
        ];
        let rivalries = [(0, 1, 4), (0, 2, 2)];
        let games = build_season(&teams, &divisions, 6, 4, 2, &rivalries).unwrap();
        assert_exact(&games, 4, 6, 4, 2, &rivalries);
        let cross = games
            .iter()
            .filter(|game| {
                let pair = (game.home.min(game.away), game.home.max(game.away));
                pair == (1, 3)
            })
            .count();
        assert_eq!(cross, 2);
    }

    #[test]
    fn npl_shape_with_a_cross_rivalry() {
        let teams = ids(6);
        let divisions = vec![
            "east".to_string(),
            "east".to_string(),
            "east".to_string(),
            "west".to_string(),
            "west".to_string(),
            "west".to_string(),
        ];
        let rivalries = [(0, 3, 2)];
        let games = build_season(&teams, &divisions, 4, 2, 2, &rivalries).unwrap();
        assert_exact(&games, 6, 4, 2, 2, &rivalries);
    }

    #[test]
    fn three_separate_divisions_play_outside_only() {
        let teams = ids(3);
        let divisions = vec!["a".into(), "b".into(), "c".into()];
        let games = build_season(&teams, &divisions, 2, 0, 2, &[]).unwrap();
        assert_exact(&games, 3, 2, 0, 2, &[]);
        assert!(games.iter().all(|game| !game.same_division));
    }

    #[test]
    fn outside_games_with_one_division_are_rejected() {
        let teams = ids(4);
        let divisions = vec!["only".to_string(); 4];
        let error = build_season(&teams, &divisions, 2, 0, 2, &[]).unwrap_err();
        assert!(error.contains("another division"), "{error}");
    }

    #[test]
    fn a_heavy_division_cannot_export_its_games() {
        let teams = ids(5);
        let divisions = vec![
            "big".into(),
            "big".into(),
            "big".into(),
            "small".into(),
            "small".into(),
        ];
        let error = build_season(&teams, &divisions, 1, 0, 1, &[]).unwrap_err();
        assert!(error.contains("do not have enough games"), "{error}");
    }

    #[test]
    fn rivalry_over_the_division_budget_is_rejected() {
        let teams = ids(3);
        let divisions = vec!["only".to_string(); 3];
        let error = build_season(&teams, &divisions, 2, 2, 0, &[(0, 1, 3)]).unwrap_err();
        assert!(error.contains("more games inside"), "{error}");
    }

    #[test]
    fn counts_that_do_not_add_up_are_rejected() {
        let teams = ids(2);
        let divisions = vec!["east".to_string(); 2];
        let error = build_season(&teams, &divisions, 6, 4, 1, &[]).unwrap_err();
        assert!(error.contains("add up"), "{error}");
    }

    #[test]
    fn eight_team_mixed_season_meets_every_count() {
        let teams = ids(8);
        let divisions: Vec<String> = (0..8)
            .map(|index| if index < 4 { "east" } else { "west" }.to_string())
            .collect();
        let games = build_season(&teams, &divisions, 5, 3, 2, &[]).unwrap();
        assert_exact(&games, 8, 5, 3, 2, &[]);
    }
}

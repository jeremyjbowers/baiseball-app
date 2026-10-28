const base = process.env.LEAGUE_OFFICE_URL ?? "http://127.0.0.1:41742";
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const email = `prover-${stamp}@league-office.test`;
let sessionToken = "";

function assert(condition, message) {
  if (!condition) {
    console.error(`FAIL ${message}`);
    process.exit(1);
  }
}

async function send(pathname, { method = "GET", body, token = sessionToken } = {}) {
  const response = await fetch(`${base}${pathname}`, {
    method,
    headers: {
      ...(body === undefined ? {} : { "content-type": "application/json" }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let json = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = text;
    }
  }
  return { status: response.status, json };
}

function tally(games) {
  const teams = new Map();
  for (const game of games) {
    for (const [id, name] of [
      [game.homeTeamId, game.homeTeamName],
      [game.awayTeamId, game.awayTeamName],
    ]) {
      const row = teams.get(id) ?? { name, games: 0, inside: 0, outside: 0 };
      row.games += 1;
      if (game.sameDivision) row.inside += 1;
      else row.outside += 1;
      teams.set(id, row);
    }
  }
  return teams;
}

function pairCount(games, left, right, rivalryOnly) {
  return games.filter((game) => {
    const pair = [game.homeTeamId, game.awayTeamId].sort().join(":");
    const wanted = [left, right].sort().join(":");
    return pair === wanted && (!rivalryOnly || game.rivalry);
  }).length;
}

async function divisionAndTeams(leagueId, groups) {
  const ids = {};
  for (const [divisionName, teamNames] of Object.entries(groups)) {
    const division = await send(`/leagues/${leagueId}/divisions`, {
      method: "POST",
      body: { name: divisionName },
    });
    assert(division.status === 201, `${divisionName} ${division.status} ${JSON.stringify(division.json)}`);
    ids[divisionName] = [];
    for (const name of teamNames) {
      const team = await send(`/leagues/${leagueId}/teams`, {
        method: "POST",
        body: { name, divisionId: division.json.division.id, homePark: `${name} Park` },
      });
      assert(team.status === 201, `${name} ${team.status} ${JSON.stringify(team.json)}`);
      ids[divisionName].push(team.json.team.id);
    }
  }
  return ids;
}

const closed = await send("/leagues");
assert(closed.status === 401, `leagues without a session should be 401, got ${closed.status}`);

async function signIn(body) {
  const issued = await send("/auth/magic-links", { method: "POST", body });
  assert(issued.status === 200 && issued.json.devOnly === true && typeof issued.json.magicLink === "string", `magic link ${issued.status} ${JSON.stringify(issued.json)}`);
  const token = new URL(issued.json.magicLink).searchParams.get("token");
  const session = await send("/auth/sessions", { method: "POST", body: { token } });
  assert(session.status === 200 && session.json.sessionToken, `session ${session.status}`);
  return { token: session.json.sessionToken, owner: session.json.owner, issued: issued.json };
}

const admin = await signIn({ email: "jeremyjbowers@gmail.com" });
assert(admin.owner.isAdmin === true, "bootstrap owner is an admin");

const signed = await signIn({ email });
sessionToken = signed.token;
const bearer = sessionToken;
const phone = "+15555550100";
const savedPhone = await send("/owners/me", { method: "PATCH", token: bearer, body: { phone } });
assert(savedPhone.status === 200 && savedPhone.json.owner.phone === phone, `phone ${savedPhone.status} ${JSON.stringify(savedPhone.json)}`);
const texted = await send("/auth/magic-links", { method: "POST", body: { phone } });
assert(texted.status === 200 && texted.json.channel === "sms" && texted.json.devOnly === true, `sms ${texted.status} ${JSON.stringify(texted.json)}`);

const open = await send("/leagues", { token: bearer });
assert(open.status === 200 && Array.isArray(open.json.leagues), `leagues ${open.status}`);

const badSum = await send("/leagues", {
  method: "POST",
  body: { name: "Bad sum", leagueType: "ulmg", seasonLength: 6, divisionGames: 4, outsideGames: 1 },
});
assert(badSum.status === 400, `mismatched counts should be 400, got ${badSum.status}`);

const ulmg = await send("/leagues", {
  method: "POST",
  body: {
    name: `Harbor ULMG ${stamp}`,
    leagueType: "ulmg",
    seasonLength: 6,
    divisionGames: 4,
    outsideGames: 2,
  },
});
assert(ulmg.status === 201, `ulmg ${ulmg.status} ${JSON.stringify(ulmg.json)}`);
assert(ulmg.json.league.leagueType === "ulmg" && ulmg.json.league.lineupLock === "game", "ulmg lock");
const ulmgId = ulmg.json.league.id;
const ulmgTeams = await divisionAndTeams(ulmgId, { East: ["Anchors", "Buoys"], West: ["Clippers", "Dories"] });
const saved = await send(`/leagues/${ulmgId}`, {
  method: "PATCH",
  body: {
    rivalries: [
      { teamId: ulmgTeams.East[0], opponentTeamId: ulmgTeams.East[1], games: 4 },
      { teamId: ulmgTeams.East[0], opponentTeamId: ulmgTeams.West[0], games: 2 },
    ],
  },
});
assert(saved.status === 200 && saved.json.league.rivalries.length === 2, "ulmg rivalries");

const built = await send(`/leagues/${ulmgId}/schedule`, { method: "POST" });
assert(built.status === 201, `ulmg build ${built.status} ${JSON.stringify(built.json)}`);
assert(built.json.games.length === 12 && built.json.matchesConfiguration === true, "ulmg game count");
const ulmgTotals = tally(built.json.games);
assert(ulmgTotals.size === 4, "ulmg teams on the schedule");
for (const row of ulmgTotals.values()) {
  assert(row.games === 6 && row.inside === 4 && row.outside === 2, `${row.name} counts ${JSON.stringify(row)}`);
}
assert(pairCount(built.json.games, ulmgTeams.East[0], ulmgTeams.East[1], true) === 4, "inside rivalry");
assert(pairCount(built.json.games, ulmgTeams.East[0], ulmgTeams.West[0], true) === 2, "outside rivalry");
const read = await send(`/leagues/${ulmgId}/schedule`);
assert(read.status === 200 && read.json.games.length === 12, "ulmg read back");
const firstIds = read.json.games.map((game) => game.id);

const replaced = await send(`/leagues/${ulmgId}/schedule`, { method: "POST" });
assert(replaced.status === 201, "ulmg rebuild");
const secondIds = replaced.json.games.map((game) => game.id);
assert(secondIds.every((id) => !firstIds.includes(id)), "rebuild replaced the games");
assert(replaced.json.games.length === 12, "rebuild length");

const prose = await send(`/leagues/${ulmgId}/schedule`, {
  method: "POST",
  body: { request: "build me a schedule that has every team play twice as many games in their division as outside it" },
});
assert(prose.status === 400, `natural language should be 400, got ${prose.status} ${JSON.stringify(prose.json)}`);
const afterProse = await send(`/leagues/${ulmgId}/schedule`);
assert(
  afterProse.json.games.map((game) => game.id).join() === secondIds.join(),
  "a rejected build left the schedule in place",
);

const npl = await send("/leagues", {
  method: "POST",
  body: {
    name: `Harbor NPL ${stamp}`,
    leagueType: "npl",
    seasonLength: 4,
    divisionGames: 2,
    outsideGames: 2,
  },
});
assert(npl.status === 201 && npl.json.league.lineupLock === "week", `npl ${npl.status} ${JSON.stringify(npl.json)}`);
const nplId = npl.json.league.id;
const nplTeams = await divisionAndTeams(nplId, {
  Atlantic: ["Pines", "Quills", "Rails"],
  Pacific: ["Sloops", "Tides", "Umiaks"],
});
const nplSaved = await send(`/leagues/${nplId}`, {
  method: "PATCH",
  body: {
    rivalries: [{ teamId: nplTeams.Atlantic[0], opponentTeamId: nplTeams.Pacific[0], games: 2 }],
  },
});
assert(nplSaved.status === 200, "npl rivalry");
const nplBuilt = await send(`/leagues/${nplId}/schedule`, { method: "POST" });
assert(nplBuilt.status === 201 && nplBuilt.json.games.length === 12, `npl build ${nplBuilt.status} ${JSON.stringify(nplBuilt.json?.error ?? nplBuilt.json?.games?.length)}`);
const nplRead = await send(`/leagues/${nplId}/schedule`);
assert(nplRead.json.games.length === 12 && nplRead.json.matchesConfiguration === true, "npl read back");
const nplTotals = tally(nplRead.json.games);
assert(nplTotals.size === 6, "npl teams");
for (const row of nplTotals.values()) {
  assert(row.games === 4 && row.inside === 2 && row.outside === 2, `${row.name} counts ${JSON.stringify(row)}`);
}
assert(
  pairCount(nplRead.json.games, nplTeams.Atlantic[0], nplTeams.Pacific[0], true) === 2,
  "npl rivalry games",
);

const narrow = await send("/leagues", {
  method: "POST",
  body: { name: `Narrow ${stamp}`, leagueType: "ulmg", seasonLength: 2, divisionGames: 2, outsideGames: 0 },
});
const narrowId = narrow.json.league.id;
const only = await send(`/leagues/${narrowId}/divisions`, { method: "POST", body: { name: "Only" } });
for (const name of ["One", "Two", "Three"]) {
  const added = await send(`/leagues/${narrowId}/teams`, {
    method: "POST",
    body: { name, divisionId: only.json.division.id, homePark: "Park" },
  });
  assert(added.status === 201, `narrow team ${name}`);
}
const narrowBuilt = await send(`/leagues/${narrowId}/schedule`, { method: "POST" });
assert(narrowBuilt.status === 201 && narrowBuilt.json.games.length === 3, "narrow build");
const kept = narrowBuilt.json.games.map((game) => game.id);
const retarget = await send(`/leagues/${narrowId}`, {
  method: "PATCH",
  body: { seasonLength: 1, divisionGames: 1, outsideGames: 0 },
});
assert(retarget.status === 200, "retarget counts");
const refused = await send(`/leagues/${narrowId}/schedule`, { method: "POST" });
assert(refused.status === 400, `impossible build should be 400, got ${refused.status} ${JSON.stringify(refused.json)}`);
const keptRead = await send(`/leagues/${narrowId}/schedule`);
assert(keptRead.json.games.map((game) => game.id).join() === kept.join(), "failed build kept the previous schedule");
assert(keptRead.json.matchesConfiguration === false, "stale schedule is marked");

const positions = ["CF", "SS", "LF", "1B", "DH", "RF", "3B", "2B", "C"];
const card = (hand) => positions.map((position, index) => ({ battingOrder: index + 1, playerName: `${hand} Batter ${index + 1}`, position }));
const lineup = await send(`/teams/${ulmgTeams.East[0]}/lineups`, {
  method: "POST",
  body: { name: "Opening card", vsLhp: card("Left"), vsRhp: card("Right") },
});
assert(lineup.status === 201 && lineup.json.lineup.vsLhp.length === 9, `lineup ${lineup.status}`);
const lineupRead = await send(`/lineups/${lineup.json.lineup.id}`);
assert(lineupRead.status === 200 && lineupRead.json.canEditLineups === true, "lineup read");

const rosterCsv = [
  "division,team,home_park,managers,rivalries",
  `East,Harbor,Camden Yards,ada-${stamp}@league.test;bea-${stamp}@league.test,Wharf:4`,
  `East,Wharf,Fenway Park,cy-${stamp}@league.test,Harbor:4`,
  `West,Mesa,"Oracle Park, San Francisco",dee-${stamp}@league.test,`,
  "West,Dunes,Oracle Park,,",
].join("\n");
const roster = await send("/leagues", {
  method: "POST",
  body: {
    name: `Roster ${stamp}`,
    leagueType: "ulmg",
    seasonLength: 6,
    divisionGames: 4,
    outsideGames: 2,
    rosterCsv,
  },
});
assert(roster.status === 201 && roster.json.league.teamCount === 4 && roster.json.league.divisionCount === 2, `roster ${roster.status} ${JSON.stringify(roster.json)}`);
assert(roster.json.league.rivalries.length === 1 && roster.json.league.rivalries[0].games === 4, "roster rivalry");
const rosterTeams = await send(`/leagues/${roster.json.league.id}/teams`);
const harbor = rosterTeams.json.teams.find((team) => team.name === "Harbor");
assert(harbor && harbor.managers.length === 2, `harbor managers ${JSON.stringify(harbor)}`);
const mesa = rosterTeams.json.teams.find((team) => team.name === "Mesa");
assert(mesa && mesa.homePark === "Oracle Park, San Francisco", "quoted park");
const patched = await send(`/teams/${harbor.id}`, {
  method: "PATCH",
  body: { managerEmails: [`ada-${stamp}@league.test`, `bea-${stamp}@league.test`, `cy-${stamp}@league.test`] },
});
assert(patched.status === 200 && patched.json.team.managers.length === 3, `managers ${patched.status} ${JSON.stringify(patched.json)}`);
const again = await send(`/leagues/${roster.json.league.id}/roster`, { method: "POST", body: { csv: rosterCsv } });
assert(again.status === 409, `duplicate roster should be 409, got ${again.status} ${JSON.stringify(again.json)}`);
const still = await send(`/leagues/${roster.json.league.id}`);
assert(still.json.teams.length === 4, "a rejected roster left the league unchanged");

console.log(`proved ULMG ${ulmgId}, NPL ${nplId}, roster ${roster.json.league.id}, and a rejected rebuild on ${narrowId}`);

import path from "path";
import { fileURLToPath } from "url";
import { DatabaseSync } from "node:sqlite";

const base = process.env.LEAGUE_OFFICE_URL ?? "http://127.0.0.1:41742";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dbPath = process.env.LEAGUE_ADMIN_DB ?? path.join(root, "data", "league-admin.sqlite");
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const email = `prover-${stamp}@league-office.test`.toLowerCase();
const positions = ["CF", "SS", "LF", "1B", "DH", "RF", "3B", "2B", "C"];

function card(hand) {
  return positions.map((position, index) => ({
    battingOrder: index + 1,
    playerName: `${hand} Batter ${index + 1}`,
    position,
  }));
}

function assert(condition, message) {
  if (!condition) {
    console.error(`FAIL ${message}`);
    process.exit(1);
  }
}

async function send(pathname, { method = "GET", body, token } = {}) {
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

const unauth = await send("/leagues", { method: "POST", body: { name: "Closed", lineupLock: "game" } });
assert(unauth.status === 401, `unsigned request should be 401, got ${unauth.status}`);

const issued = await send("/auth/magic-links", { method: "POST", body: { email } });
assert(issued.status === 200, `magic link status ${issued.status} ${JSON.stringify(issued.json)}`);
assert(typeof issued.json.magicLink === "string" && issued.json.magicLink.includes("/auth/verify?token="), "magic link missing");
const magicToken = new URL(issued.json.magicLink).searchParams.get("token");
assert(magicToken && magicToken.length >= 32, "magic-link token missing");

const db = new DatabaseSync(dbPath, { readOnly: true });
const stored = db.prepare("SELECT token, used_at, owner_id FROM magic_links WHERE token = ?").get(magicToken);
assert(stored?.token === magicToken, "magic-link token was not stored");
assert(stored.used_at == null, "magic-link token was already used");
const owner = db.prepare("SELECT email FROM owners WHERE id = ?").get(stored.owner_id);
assert(owner?.email === email, `owner email was not stored: ${owner?.email}`);

const session = await send("/auth/sessions", { method: "POST", body: { token: magicToken } });
assert(session.status === 200, `exchange ${session.status} ${JSON.stringify(session.json)}`);
const bearer = session.json.sessionToken;
assert(bearer, "session token missing");

const again = await send("/auth/sessions", { method: "POST", body: { token: magicToken } });
assert(again.status === 401, `used magic link should be 401, got ${again.status}`);

const who = await send("/auth/session", { token: bearer });
assert(who.status === 200 && who.json.owner.email === email, "session read");

const leagueResponse = await send("/leagues", {
  method: "POST",
  token: bearer,
  body: { name: `Proof League ${stamp}`, lineupLock: "game" },
});
assert(leagueResponse.status === 201, `league ${leagueResponse.status} ${JSON.stringify(leagueResponse.json)}`);
const leagueId = leagueResponse.json.league.id;
assert(leagueResponse.json.league.inBounds === false, "an empty league should be out of bounds");

const divisionResponse = await send(`/leagues/${leagueId}/divisions`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof East" },
});
assert(divisionResponse.status === 201, `division ${divisionResponse.status} ${JSON.stringify(divisionResponse.json)}`);
const divisionId = divisionResponse.json.division.id;

const spare = await send(`/leagues/${leagueId}/divisions`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof Spare" },
});
assert(spare.status === 201, "spare division");
const removed = await send(`/divisions/${spare.json.division.id}`, { method: "DELETE", token: bearer });
assert(removed.status === 200, `delete spare division ${removed.status}`);

const teamResponse = await send(`/leagues/${leagueId}/teams`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof Club", divisionId, homePark: "Proof Park", ownerEmail: email },
});
assert(teamResponse.status === 201, `team ${teamResponse.status} ${JSON.stringify(teamResponse.json)}`);
const teamId = teamResponse.json.team.id;
assert(teamResponse.json.team.homePark === "Proof Park", "home park");
assert(teamResponse.json.teamCount === 1, "team count");

const lineupResponse = await send(`/teams/${teamId}/lineups`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof card", vsLhp: card("Left"), vsRhp: card("Right") },
});
assert(lineupResponse.status === 201, `lineup ${lineupResponse.status} ${JSON.stringify(lineupResponse.json)}`);
const lineupId = lineupResponse.json.lineup.id;
assert(lineupResponse.json.lineup.vsLhp.length === 9 && lineupResponse.json.lineup.vsRhp[8].position === "C", "cards");

const renamed = await send(`/leagues/${leagueId}`, {
  method: "PATCH",
  token: bearer,
  body: { name: `Proof League ${stamp} renamed`, lineupLock: "week" },
});
assert(renamed.status === 200 && renamed.json.league.lineupLock === "week", "league update");
assert((await send(`/divisions/${divisionId}`, { method: "PATCH", token: bearer, body: { name: "Proof East renamed" } })).status === 200, "division update");
assert((await send(`/teams/${teamId}`, { method: "PATCH", token: bearer, body: { name: "Proof Club renamed", homePark: "Proof Park renamed", divisionId } })).json.team.homePark === "Proof Park renamed", "team update");
assert((await send(`/lineups/${lineupId}`, { method: "PATCH", token: bearer, body: { name: "Proof card renamed", vsLhp: card("Left"), vsRhp: card("Right") } })).json.lineup.name === "Proof card renamed", "lineup update");

const readLeague = await send(`/leagues/${leagueId}`, { token: bearer });
assert(readLeague.json.teams.length === 1, "team list");
assert(readLeague.json.league.divisions.some((division) => division.name === "Proof East renamed"), "division list");
const readLineup = await send(`/lineups/${lineupId}`, { token: bearer });
assert(readLineup.json.lineup.vsLhp.length === 9 && readLineup.json.lineup.vsRhp.length === 9, "cards on read");

assert((await send(`/lineups/${lineupId}`, { method: "DELETE", token: bearer })).status === 200, "delete lineup");
assert((await send(`/lineups/${lineupId}`, { token: bearer })).status === 404, "deleted lineup");
const restored = await send(`/teams/${teamId}/lineups`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof card restored", vsLhp: card("Left"), vsRhp: card("Right") },
});
assert(restored.status === 201, "restore lineup");

let teamCount = 1;
while (teamCount < 10) {
  const added = await send(`/leagues/${leagueId}/teams`, {
    method: "POST",
    token: bearer,
    body: { name: `Proof Club ${teamCount + 1}`, divisionId, homePark: "Proof Park", ownerEmail: email },
  });
  assert(added.status === 201, `add team ${teamCount + 1}: ${added.status} ${JSON.stringify(added.json)}`);
  teamCount = added.json.teamCount;
}
const blocked = await send(`/teams/${teamId}`, { method: "DELETE", token: bearer });
assert(blocked.status === 409, `delete at 10 should be 409, got ${blocked.status}`);
const eleventh = await send(`/leagues/${leagueId}/teams`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof Club 11", divisionId, homePark: "Proof Park", ownerEmail: email },
});
assert(eleventh.status === 201, "eleventh team");
assert((await send(`/teams/${eleventh.json.team.id}`, { method: "DELETE", token: bearer })).status === 200, "delete above 10");

while (teamCount < 30) {
  const added = await send(`/leagues/${leagueId}/teams`, {
    method: "POST",
    token: bearer,
    body: { name: `Proof Club ${teamCount + 1}`, divisionId, homePark: "Proof Park", ownerEmail: email },
  });
  assert(added.status === 201, `add team ${teamCount + 1}: ${added.status} ${JSON.stringify(added.json)}`);
  teamCount = added.json.teamCount;
  if (teamCount === 26 || teamCount === 28) assert(added.json.inBounds === false, `${teamCount} closed`);
  if (teamCount === 27 || teamCount === 30) assert(added.json.inBounds === true, `${teamCount} in bounds`);
}
const over = await send(`/leagues/${leagueId}/teams`, {
  method: "POST",
  token: bearer,
  body: { name: "Proof Club 31", divisionId, homePark: "Proof Park", ownerEmail: email },
});
assert(over.status === 409, `31st team should be 409, got ${over.status}`);
const finalLeague = await send(`/leagues/${leagueId}`, { token: bearer });
assert(finalLeague.json.league.teamCount === 30 && finalLeague.json.teams.length === 30, "final count");

const used = new DatabaseSync(dbPath, { readOnly: true })
  .prepare("SELECT used_at FROM magic_links WHERE token = ?")
  .get(magicToken);
assert(typeof used?.used_at === "string" && used.used_at.length > 0, "magic link was not marked used");
console.log(`proved magic-link token, league ${leagueId}, division ${divisionId}, team ${teamId}, lineup ${restored.json.lineup.id}`);

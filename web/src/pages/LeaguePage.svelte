<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { countNote, divisionOptionLabel, managerLine, seatLabel, typeDetail } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import Confirm from "../lib/Confirm.svelte";
  import Field from "../lib/Field.svelte";
  import LeagueFields from "../lib/LeagueFields.svelte";
  import ManagersField from "../lib/ManagersField.svelte";
  import RosterFile from "../lib/RosterFile.svelte";

  let { leagueId } = $props();
  let league = $state(null);
  let teams = $state([]);
  let loading = $state(true);
  let error = $state("");
  let formError = $state("");
  let name = $state("");
  let leagueType = $state("");
  let seasonLength = $state("");
  let divisionGames = $state("");
  let outsideGames = $state("");
  let rivalries = $state([]);
  let rivalA = $state("");
  let rivalB = $state("");
  let rivalGames = $state("");
  let subleagueName = $state("");
  let subleagueDrafts = $state({});
  let divisionName = $state("");
  let divisionSubleagueId = $state("");
  let divisionDrafts = $state({});
  let teamName = $state("");
  let divisionId = $state("");
  let homePark = $state("");
  let managerEmails = $state([""]);
  let rosterCsv = $state("");
  let rosterName = $state("");
  let notice = $state("");
  let pending = $state("");
  let note = $derived(countNote(seasonLength, divisionGames, outsideGames));

  function apply(payload, quiet) {
    league = payload.league;
    teams = payload.teams;
    subleagueDrafts = Object.fromEntries((payload.league.subleagues ?? []).map((item) => [item.id, item.name]));
    divisionDrafts = Object.fromEntries(payload.league.divisions.map((division) => [division.id, division.name]));
    if (!(payload.league.subleagues ?? []).some((item) => item.id === divisionSubleagueId)) {
      divisionSubleagueId = payload.league.subleagues?.[0]?.id ?? "";
    }
    if (!payload.league.divisions.some((division) => division.id === divisionId)) {
      divisionId = payload.league.divisions[0]?.id ?? "";
    }
    if (!teams.some((team) => team.id === rivalA)) rivalA = teams[0]?.id ?? "";
    if (!teams.some((team) => team.id === rivalB)) rivalB = teams[1]?.id ?? teams[0]?.id ?? "";
    if (!quiet) {
      name = payload.league.name;
      leagueType = payload.league.leagueType ?? "ulmg";
      seasonLength = payload.league.seasonLength == null ? "" : String(payload.league.seasonLength);
      divisionGames = payload.league.divisionGames == null ? "" : String(payload.league.divisionGames);
      outsideGames = payload.league.outsideGames == null ? "" : String(payload.league.outsideGames);
      rivalries = (payload.league.rivalries ?? []).map((rivalry) => ({
        teamId: rivalry.teamId,
        opponentTeamId: rivalry.opponentTeamId,
        games: rivalry.games,
        teamName: rivalry.teamName,
        opponentTeamName: rivalry.opponentTeamName,
      }));
    }
  }

  async function load(quiet = false) {
    if (!quiet) {
      loading = true;
      error = "";
    }
    try {
      const payload = await api(`/leagues/${leagueId}`);
      apply(payload, quiet);
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "The league could not be loaded.";
      if (quiet) formError = message;
      else error = message;
    } finally {
      loading = false;
    }
  }

  async function run(key, action) {
    pending = key;
    formError = "";
    notice = "";
    try {
      await action();
    } catch (caught) {
      formError = caught instanceof Error ? caught.message : "The office could not complete that.";
    } finally {
      pending = "";
    }
  }

  function settingsBody() {
    if (!note.ok) throw new Error(note.text);
    return {
      name,
      leagueType,
      seasonLength: note.season,
      divisionGames: note.inside,
      outsideGames: note.outside,
      rivalries: rivalries.map((rivalry) => ({
        teamId: rivalry.teamId,
        opponentTeamId: rivalry.opponentTeamId,
        games: Number(rivalry.games),
      })),
    };
  }

  function teamNameOf(id) {
    return teams.find((team) => team.id === id)?.name ?? "Team";
  }

  function addRivalry() {
    formError = "";
    if (!rivalA || !rivalB) {
      formError = "Choose both rivalry opponents.";
      return;
    }
    if (rivalA === rivalB) {
      formError = "A team cannot be its own rival.";
      return;
    }
    if (!/^\d+$/.test(String(rivalGames).trim()) || Number(rivalGames) < 1) {
      formError = "Say how many games the rivalry plays.";
      return;
    }
    const key = [rivalA, rivalB].sort().join(":");
    if (rivalries.some((rivalry) => [rivalry.teamId, rivalry.opponentTeamId].sort().join(":") === key)) {
      formError = "Name each rivalry once.";
      return;
    }
    rivalries = [
      ...rivalries,
      {
        teamId: rivalA,
        opponentTeamId: rivalB,
        games: Number(rivalGames),
        teamName: teamNameOf(rivalA),
        opponentTeamName: teamNameOf(rivalB),
      },
    ];
    rivalGames = "";
  }

  onMount(() => load());
</script>

{#if loading}<div class="status" role="status">Loading the league…</div>
{:else if error || !league}
  <div class="alert error" role="alert">
    <strong>Something went wrong</strong>
    <p>{error || "That league is not in the office."}</p>
    <button class="secondary" type="button" onclick={() => load()}>Try again</button>
  </div>
{:else}
  <div class="stack">
    <a class="link" href="/leagues">All leagues</a>
    <div class="spread">
      <h1>{league.name}</h1>
      <a class="button secondary" href="/leagues/{league.id}/schedule">Schedule{league.scheduledGameCount ? ` · ${league.scheduledGameCount} games` : ""}</a>
    </div>
    <p class:danger={!league.inBounds}>{league.boundLabel}</p>
    <p class="muted">{typeDetail(league.leagueType)}</p>
    <p class="muted">Teams sit in a league and a division. like strat-o-matic is often 16 teams in four divisions. like scoresheet is often 24 teams in six divisions. Those counts are examples. 26 and 28 team counts are closed.</p>
    {#if formError}<div class="alert error" role="alert"><strong>Could not save</strong><p>{formError}</p></div>{/if}
    {#if notice}<div class="alert" role="status"><p>{notice}</p></div>{/if}

    <form class="card stack" onsubmit={(event) => { event.preventDefault(); run("league", async () => {
      const payload = await api(`/leagues/${league.id}`, { method: "PATCH", body: JSON.stringify(settingsBody()) });
      league = payload.league;
      apply({ league: payload.league, teams }, false);
    }); }}>
      <h2>League settings</h2>
      <LeagueFields bind:name bind:leagueType bind:seasonLength bind:divisionGames bind:outsideGames />

      <div class="stack">
        <h3>Rivalry opponents</h3>
        <p class="muted">A rivalry is two teams and how many times they play. Those games count toward the inside total when they share a division, and toward the outside total when they do not. They are not extra games on top of the season.</p>
        {#if rivalries.length === 0}
          <p class="muted">No rivalry pairs yet.</p>
        {/if}
        {#each rivalries as rivalry, index (`${rivalry.teamId}-${rivalry.opponentTeamId}`)}
          <div class="spread">
            <p>{rivalry.teamName} and {rivalry.opponentTeamName}, {rivalry.games} games</p>
            <button class="secondary" type="button" onclick={() => { rivalries = rivalries.filter((_, item) => item !== index); }}>Remove</button>
          </div>
        {/each}
        {#if teams.length >= 2}
          <div class="rivalry-form">
            <Field id="rival-a" label="Team">
              <select id="rival-a" bind:value={rivalA}>
                {#each teams as team (team.id)}<option value={team.id}>{team.name}</option>{/each}
              </select>
            </Field>
            <Field id="rival-b" label="Opponent">
              <select id="rival-b" bind:value={rivalB}>
                {#each teams as team (team.id)}<option value={team.id}>{team.name}</option>{/each}
              </select>
            </Field>
            <Field id="rival-games" label="Games">
              <input id="rival-games" inputmode="numeric" bind:value={rivalGames} />
            </Field>
            <button class="secondary" type="button" onclick={addRivalry}>Add rivalry</button>
          </div>
        {:else}
          <p class="muted">Add at least two teams before naming a rivalry.</p>
        {/if}
      </div>

      <div class="row">
        <button type="submit" disabled={pending === "league" || !note.ok}>{pending === "league" ? "Saving the league…" : "Save league"}</button>
        <button type="button" disabled={pending === "build" || !note.ok || teams.length < 2} onclick={() => run("build", async () => {
          await api(`/leagues/${league.id}`, { method: "PATCH", body: JSON.stringify(settingsBody()) });
          await api(`/leagues/${league.id}/schedule`, { method: "POST" });
          navigate(`/leagues/${league.id}/schedule`);
        })}>{pending === "build" ? "Building the schedule…" : "Build schedule"}</button>
      </div>
      {#if teams.length < 2}<p class="hint">Add at least two teams before building a schedule. Building again replaces the schedule on file.</p>{/if}
    </form>

    <section class="stack">
      <h2>Leagues and divisions</h2>
      <p class="muted">A team sits in a league and a division, for example National League, West. Both National League and American League can have a West division.</p>
      {#if (league.subleagues ?? []).length === 0}
        <div class="empty"><h3>No leagues yet</h3><p class="muted">Add National League or American League, then add a division under it before you add a team.</p></div>
      {/if}
      {#each league.subleagues ?? [] as subleague (subleague.id)}
        <div class="panel stack">
          <form class="form-inline" onsubmit={(event) => { event.preventDefault(); run(`subleague-${subleague.id}`, async () => {
            await api(`/subleagues/${subleague.id}`, { method: "PATCH", body: JSON.stringify({ name: subleagueDrafts[subleague.id] ?? subleague.name }) });
            await load(true);
          }); }}>
            <Field id="subleague-{subleague.id}" label="League name">
              <input id="subleague-{subleague.id}" value={subleagueDrafts[subleague.id] ?? subleague.name} oninput={(event) => { subleagueDrafts = { ...subleagueDrafts, [subleague.id]: event.currentTarget.value }; }} />
            </Field>
            <button class="secondary" type="submit" disabled={pending === `subleague-${subleague.id}`}>{pending === `subleague-${subleague.id}` ? "Saving…" : "Save name"}</button>
          </form>
          {#each league.divisions.filter((division) => division.subleagueId === subleague.id) as division (division.id)}
            <form class="form-inline" onsubmit={(event) => { event.preventDefault(); run(`division-${division.id}`, async () => {
              await api(`/divisions/${division.id}`, { method: "PATCH", body: JSON.stringify({ name: divisionDrafts[division.id] ?? division.name }) });
              await load(true);
            }); }}>
              <Field id="division-{division.id}" label="Division name">
                <input id="division-{division.id}" value={divisionDrafts[division.id] ?? division.name} oninput={(event) => { divisionDrafts = { ...divisionDrafts, [division.id]: event.currentTarget.value }; }} />
              </Field>
              <button class="secondary" type="submit" disabled={pending === `division-${division.id}`}>{pending === `division-${division.id}` ? "Saving…" : "Save name"}</button>
            </form>
            <Confirm
              label="Delete division"
              title={`Delete ${division.name}?`}
              description="Teams have to leave this division first. Deleting the office league removes the division with its teams."
              confirmLabel="Delete division"
              onConfirm={async () => { await api(`/divisions/${division.id}`, { method: "DELETE" }); await load(true); }}
            />
          {/each}
          <Confirm
            label="Delete league"
            title={`Delete ${subleague.name}?`}
            description="Divisions have to leave this league first. Deleting the office league removes it with its divisions and teams."
            confirmLabel="Delete league"
            onConfirm={async () => { await api(`/subleagues/${subleague.id}`, { method: "DELETE" }); await load(true); }}
          />
        </div>
      {/each}
      <form class="panel form-inline" onsubmit={(event) => { event.preventDefault(); run("new-subleague", async () => {
        await api(`/leagues/${league.id}/subleagues`, { method: "POST", body: JSON.stringify({ name: subleagueName }) });
        subleagueName = "";
        await load(true);
      }); }}>
        <Field id="new-subleague" label="New league" hint="National League or American League.">
          <input id="new-subleague" bind:value={subleagueName} />
        </Field>
        <button type="submit" disabled={pending === "new-subleague"}>{pending === "new-subleague" ? "Adding…" : "Add league"}</button>
      </form>
      {#if (league.subleagues ?? []).length > 0}
        <form class="panel form-inline" onsubmit={(event) => { event.preventDefault(); run("new-division", async () => {
          await api(`/leagues/${league.id}/divisions`, { method: "POST", body: JSON.stringify({ name: divisionName, subleagueId: divisionSubleagueId }) });
          divisionName = "";
          await load(true);
        }); }}>
          <Field id="new-division-league" label="League">
            <select id="new-division-league" bind:value={divisionSubleagueId}>
              {#each league.subleagues as item (item.id)}<option value={item.id}>{item.name}</option>{/each}
            </select>
          </Field>
          <Field id="new-division" label="New division"><input id="new-division" bind:value={divisionName} /></Field>
          <button type="submit" disabled={pending === "new-division"}>{pending === "new-division" ? "Adding…" : "Add division"}</button>
        </form>
      {/if}
    </section>

    <section class="stack">
      <h2>Teams</h2>
      {#if teams.length === 0}
        <div class="empty"><h3>No teams yet</h3><p class="muted">A team has a name, a league, a division, a home park, and one or more managers. Leave managers blank and the office holds the team. Sign-in is not on, so this office can edit the lineup.</p></div>
      {/if}
      {#each teams as team (team.id)}
        <article class="card stack">
          <h3><a class="link" href="/teams/{team.id}">{team.name}</a></h3>
          <p>{seatLabel(team)}</p>
          <p>Home park: {team.homePark}</p>
          <p>Managers: {managerLine(team)}</p>
          <p class="muted">{team.lineupCount} {team.lineupCount === 1 ? "lineup" : "lineups"}</p>
          <a class="button secondary" href="/teams/{team.id}">Open team</a>
        </article>
      {/each}
      <form class="panel stack" onsubmit={(event) => { event.preventDefault(); run("roster", async () => {
        if (!rosterCsv.trim()) {
          formError = "Choose a roster CSV.";
          return;
        }
        const payload = await api(`/leagues/${league.id}/roster`, { method: "POST", body: JSON.stringify({ csv: rosterCsv }) });
        apply(payload, true);
        rivalries = (payload.league.rivalries ?? []).map((rivalry) => ({
          teamId: rivalry.teamId,
          opponentTeamId: rivalry.opponentTeamId,
          games: rivalry.games,
          teamName: rivalry.teamName,
          opponentTeamName: rivalry.opponentTeamName,
        }));
        const nested = Number(payload.addedSubleagues ?? 0);
        const divisions = Number(payload.addedDivisions ?? 0);
        const added = Number(payload.addedTeams ?? 0);
        const parts = [];
        if (nested > 0) parts.push(nested === 1 ? "1 league" : `${nested} leagues`);
        if (divisions > 0) parts.push(divisions === 1 ? "1 division" : `${divisions} divisions`);
        parts.push(added === 1 ? "1 team" : `${added} teams`);
        notice = `Added ${parts.join(", ")}.`;
        rosterCsv = "";
        rosterName = "";
      }); }}>
        <RosterFile bind:csv={rosterCsv} bind:fileName={rosterName} idPrefix="league-roster" />
        <button type="submit" disabled={pending === "roster" || !rosterCsv.trim()}>{pending === "roster" ? "Adding teams…" : "Add teams from file"}</button>
      </form>
      {#if league.teamCount >= 30}
        <p class="danger">This league already has 30 teams.</p>
      {:else if league.divisions.length === 0}
        <p class="muted">Add a league and a division before adding a team.</p>
      {:else}
        <form class="panel stack" onsubmit={(event) => { event.preventDefault(); run("new-team", async () => {
          const managers = managerEmails.map((email) => email.trim()).filter(Boolean);
          const body = { name: teamName, divisionId, homePark };
          if (managers.length) body.managerEmails = managers;
          await api(`/leagues/${league.id}/teams`, { method: "POST", body: JSON.stringify(body) });
          teamName = "";
          homePark = "";
          managerEmails = [""];
          await load(true);
        }); }}>
          <h3>Add a team</h3>
          <Field id="team-name" label="Team name"><input id="team-name" bind:value={teamName} /></Field>
          <Field id="team-division" label="Division">
            <select id="team-division" bind:value={divisionId}>
              {#each league.divisions as division (division.id)}<option value={division.id}>{divisionOptionLabel(division)}</option>{/each}
            </select>
          </Field>
          <Field id="home-park" label="Home park" hint="The park this team plays in at home."><input id="home-park" bind:value={homePark} /></Field>
          <ManagersField
            idPrefix="league-manager"
            bind:emails={managerEmails}
            hint="Optional. Add every manager. Leave this blank and the office holds the team."
          />
          <button type="submit" disabled={pending === "new-team"}>{pending === "new-team" ? "Adding the team…" : "Add team"}</button>
        </form>
      {/if}
    </section>

    <Confirm
      label="Delete league"
      title={`Delete ${league.name}?`}
      description="This removes the office league, its leagues, its divisions, its teams, their lineups, and the schedule."
      confirmLabel="Delete league"
      onConfirm={async () => { await api(`/leagues/${league.id}`, { method: "DELETE" }); navigate("/leagues"); }}
    />
  </div>
{/if}

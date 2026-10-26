<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { LOCKS, lockDetail } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import { getSession } from "../lib/session.js";
  import Confirm from "../lib/Confirm.svelte";
  import Field from "../lib/Field.svelte";

  let { leagueId } = $props();
  let league = $state(null);
  let teams = $state([]);
  let loading = $state(true);
  let error = $state("");
  let formError = $state("");
  let name = $state("");
  let lineupLock = $state("");
  let divisionName = $state("");
  let divisionDrafts = $state({});
  let teamName = $state("");
  let divisionId = $state("");
  let homePark = $state("");
  let ownerEmail = $state(getSession()?.email ?? "");
  let pending = $state("");
  let detail = $derived(LOCKS.find((lock) => lock.value === lineupLock)?.detail ?? "");

  function apply(payload, quiet) {
    league = payload.league;
    teams = payload.teams;
    divisionDrafts = Object.fromEntries(payload.league.divisions.map((division) => [division.id, division.name]));
    if (!payload.league.divisions.some((division) => division.id === divisionId)) {
      divisionId = payload.league.divisions[0]?.id ?? "";
    }
    if (!quiet) {
      name = payload.league.name;
      lineupLock = payload.league.lineupLock;
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
    try {
      await action();
    } catch (caught) {
      formError = caught instanceof Error ? caught.message : "The office could not complete that.";
    } finally {
      pending = "";
    }
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
    <h1>{league.name}</h1>
    <p class:danger={!league.inBounds}>{league.boundLabel}</p>
    <p class="muted">{lockDetail(league.lineupLock)}</p>
    <p class="muted">Division size is not fixed. ULMG uses four divisions of four. NPL uses six divisions of four. 26 and 28 team counts are closed.</p>
    {#if formError}<div class="alert error" role="alert"><strong>Could not save</strong><p>{formError}</p></div>{/if}

    <form class="card stack" onsubmit={(event) => { event.preventDefault(); run("league", async () => {
      const payload = await api(`/leagues/${league.id}`, { method: "PATCH", body: JSON.stringify({ name, lineupLock }) });
      league = { ...league, ...payload.league, divisions: league.divisions };
      name = payload.league.name;
      lineupLock = payload.league.lineupLock;
    }); }}>
      <h2>League settings</h2>
      <Field id="league-name" label="League name"><input id="league-name" bind:value={name} /></Field>
      <Field id="lineup-lock" label="Lineup lock" hint={detail}>
        <select id="lineup-lock" bind:value={lineupLock}>
          {#each LOCKS as lock (lock.value)}<option value={lock.value}>{lock.label}</option>{/each}
        </select>
      </Field>
      <button type="submit" disabled={pending === "league"}>{pending === "league" ? "Saving the league…" : "Save league"}</button>
    </form>

    <section class="stack">
      <h2>Divisions</h2>
      {#if league.divisions.length === 0}
        <div class="empty"><h3>No divisions yet</h3><p class="muted">A team has to sit in a division. Add one before you add a team.</p></div>
      {/if}
      {#each league.divisions as division (division.id)}
        <div class="panel stack">
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
            description="Teams have to leave this division first. Deleting the league removes the division with its teams."
            confirmLabel="Delete division"
            onConfirm={async () => { await api(`/divisions/${division.id}`, { method: "DELETE" }); await load(true); }}
          />
        </div>
      {/each}
      <form class="panel form-inline" onsubmit={(event) => { event.preventDefault(); run("new-division", async () => {
        await api(`/leagues/${league.id}/divisions`, { method: "POST", body: JSON.stringify({ name: divisionName }) });
        divisionName = "";
        await load(true);
      }); }}>
        <Field id="new-division" label="New division"><input id="new-division" bind:value={divisionName} /></Field>
        <button type="submit" disabled={pending === "new-division"}>{pending === "new-division" ? "Adding…" : "Add division"}</button>
      </form>
    </section>

    <section class="stack">
      <h2>Teams</h2>
      {#if teams.length === 0}
        <div class="empty"><h3>No teams yet</h3><p class="muted">A team has a name, a division, and a home park. The owner you assign controls the team and writes its lineups.</p></div>
      {/if}
      {#each teams as team (team.id)}
        <article class="card stack">
          <h3><a class="link" href="/teams/{team.id}">{team.name}</a></h3>
          <p>{team.divisionName}</p>
          <p>Home park: {team.homePark}</p>
          <p>Controlled by {team.ownerEmail}</p>
          <p class="muted">{team.lineupCount} {team.lineupCount === 1 ? "lineup" : "lineups"}</p>
          <a class="button secondary" href="/teams/{team.id}">Open team</a>
        </article>
      {/each}
      {#if league.teamCount >= 30}
        <p class="danger">This league already has 30 teams.</p>
      {:else if league.divisions.length === 0}
        <p class="muted">Add a division before adding a team.</p>
      {:else}
        <form class="panel stack" onsubmit={(event) => { event.preventDefault(); run("new-team", async () => {
          await api(`/leagues/${league.id}/teams`, { method: "POST", body: JSON.stringify({ name: teamName, divisionId, homePark, ownerEmail }) });
          teamName = "";
          homePark = "";
          ownerEmail = getSession()?.email ?? "";
          await load(true);
        }); }}>
          <h3>Add a team</h3>
          <Field id="team-name" label="Team name"><input id="team-name" bind:value={teamName} /></Field>
          <Field id="team-division" label="Division">
            <select id="team-division" bind:value={divisionId}>
              {#each league.divisions as division (division.id)}<option value={division.id}>{division.name}</option>{/each}
            </select>
          </Field>
          <Field id="home-park" label="Home park" hint="The park this team plays in at home."><input id="home-park" bind:value={homePark} /></Field>
          <Field id="owner-email" label="Owner email" hint="This owner controls the team and writes its lineups. They sign in with this address.">
            <input id="owner-email" type="email" bind:value={ownerEmail} />
          </Field>
          <button type="submit" disabled={pending === "new-team"}>{pending === "new-team" ? "Adding the team…" : "Add team"}</button>
        </form>
      {/if}
    </section>

    <Confirm
      label="Delete league"
      title={`Delete ${league.name}?`}
      description="This removes the league, its divisions, its teams, and their lineups."
      confirmLabel="Delete league"
      onConfirm={async () => { await api(`/leagues/${league.id}`, { method: "DELETE" }); navigate("/leagues"); }}
    />
  </div>
{/if}

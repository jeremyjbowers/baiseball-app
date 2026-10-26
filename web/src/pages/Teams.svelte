<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { lockDetail } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import { getSession } from "../lib/session.js";
  import Field from "../lib/Field.svelte";

  let teams = $state([]);
  let leagues = $state([]);
  let loading = $state(true);
  let error = $state("");
  let formError = $state("");
  let leagueId = $state("");
  let chosenDivision = $state("");
  let name = $state("");
  let homePark = $state("");
  let ownerEmail = $state(getSession()?.email ?? "");
  let pending = $state(false);
  let league = $derived(leagues.find((item) => item.id === leagueId));
  let divisionId = $derived(league?.divisions.some((division) => division.id === chosenDivision) ? chosenDivision : (league?.divisions[0]?.id ?? ""));

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api("/teams");
      teams = payload.teams;
      leagues = payload.leagues;
      if (!payload.leagues.some((item) => item.id === leagueId)) leagueId = payload.leagues[0]?.id ?? "";
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The teams could not be loaded.";
    } finally {
      loading = false;
    }
  }

  async function submit(event) {
    event.preventDefault();
    pending = true;
    formError = "";
    try {
      const payload = await api(`/leagues/${leagueId}/teams`, {
        method: "POST",
        body: JSON.stringify({ name, divisionId, homePark, ownerEmail }),
      });
      navigate(`/teams/${payload.team.id}`);
    } catch (caught) {
      formError = caught instanceof Error ? caught.message : "The team could not be created.";
      pending = false;
    }
  }

  onMount(load);
</script>

<div class="stack">
  <div>
    <h1>Teams you control</h1>
    <p class="muted">Lineups are the MLB card only, one versus left-handed pitching and one versus right-handed pitching.</p>
  </div>
  {#if loading}<div class="status" role="status">Loading teams…</div>{/if}
  {#if error}
    <div class="alert error" role="alert"><strong>Something went wrong</strong><p>{error}</p><button class="secondary" type="button" onclick={load}>Try again</button></div>
  {/if}
  {#if !loading && !error && teams.length === 0}
    <div class="empty">
      <h2>You do not control a team yet</h2>
      <p class="muted">A league administrator assigns control. If you administer a league, you can create a team below and keep it, or assign it to another owner.</p>
    </div>
  {/if}
  {#if !loading && !error}
    {#each teams as team (team.id)}
      <article class="card stack">
        <h2><a class="link" href="/teams/{team.id}">{team.name}</a></h2>
        <p>{team.leagueName} · {team.divisionName}</p>
        <p>Home park: {team.homePark}</p>
        <p class="muted">{lockDetail(team.lineupLock)}</p>
        <p class="muted">{team.lineupCount} {team.lineupCount === 1 ? "lineup" : "lineups"}</p>
        <a class="button secondary" href="/teams/{team.id}">Open team</a>
      </article>
    {/each}
  {/if}
  {#if !loading && !error && leagues.length > 0}
    <form class="panel stack" onsubmit={submit}>
      <h2>Create a team</h2>
      <p class="muted">You can add a team in a league you administer. The team count for that league stays between 10 and 30.</p>
      {#if formError}<div class="alert error" role="alert"><strong>Could not save</strong><p>{formError}</p></div>{/if}
      <Field id="create-league" label="League">
        <select id="create-league" bind:value={leagueId}>
          {#each leagues as item (item.id)}<option value={item.id}>{item.name}</option>{/each}
        </select>
      </Field>
      {#if league && league.divisions.length === 0}
        <p class="muted">Add a division in {league.name} before creating a team.</p>
      {:else}
        <Field id="create-division" label="Division">
          <select id="create-division" value={divisionId} onchange={(event) => { chosenDivision = event.currentTarget.value; }}>
            {#each league?.divisions ?? [] as division (division.id)}<option value={division.id}>{division.name}</option>{/each}
          </select>
        </Field>
      {/if}
      <Field id="create-team-name" label="Team name"><input id="create-team-name" bind:value={name} /></Field>
      <Field id="create-home-park" label="Home park" hint="The park this team plays in at home."><input id="create-home-park" bind:value={homePark} /></Field>
      <Field id="create-owner-email" label="Owner email" hint="This owner controls the team and writes its lineups.">
        <input id="create-owner-email" type="email" bind:value={ownerEmail} />
      </Field>
      <button type="submit" disabled={pending || !league || league.divisions.length === 0 || league.teamCount >= 30}>
        {pending ? "Creating the team…" : "Create team"}
      </button>
      {#if league && league.teamCount >= 30}<p class="danger">{league.name} already has 30 teams.</p>{/if}
    </form>
  {/if}
</div>

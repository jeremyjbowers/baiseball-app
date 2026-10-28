<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { typeDetail, typeLabel } from "../lib/copy.js";

  let leagues = $state([]);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api("/leagues");
      leagues = payload.leagues;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The leagues could not be loaded.";
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<div class="stack">
  <div class="spread">
    <div>
      <h1>Leagues</h1>
      <p class="muted">A league holds divisions, teams, lineups, and a schedule. like strat-o-matic plays one game at a time, a season behind. like scoresheet simulates a week and mixes current and past stats. You name the leagues and the divisions.</p>
    </div>
    <a class="button" href="/leagues/new">Create a league</a>
  </div>

  {#if loading}<div class="status" role="status">Loading leagues…</div>{/if}
  {#if error}
    <div class="alert error" role="alert">
      <strong>Something went wrong</strong>
      <p>{error}</p>
      <button class="secondary" type="button" onclick={load}>Try again</button>
    </div>
  {/if}
  {#if !loading && !error && leagues.length === 0}
    <div class="empty stack">
      <h2>No leagues yet</h2>
      <p class="muted">Create one, then add divisions and teams and build the schedule.</p>
      <a class="button" href="/leagues/new">Create a league</a>
    </div>
  {/if}
  {#if !loading && !error}
    {#each leagues as league (league.id)}
      <article class="card stack">
        <h2><a class="link" href="/leagues/{league.id}">{league.name}</a></h2>
        <p>{typeLabel(league.leagueType)}</p>
        <p class="muted">{typeDetail(league.leagueType)}</p>
        {#if league.seasonLength != null}
          <p class="muted">{league.seasonLength} games. {league.divisionGames} inside the division, {league.outsideGames} outside it.</p>
        {/if}
        <p class:danger={!league.inBounds}>{league.boundLabel}</p>
        <p class="muted">{league.divisionCount} {league.divisionCount === 1 ? "division" : "divisions"} · {league.scheduledGameCount || 0} scheduled games</p>
        <div class="row">
          <a class="button secondary" href="/leagues/{league.id}">Open league</a>
          <a class="button ghost" href="/leagues/{league.id}/schedule">Schedule</a>
        </div>
      </article>
    {/each}
  {/if}
</div>

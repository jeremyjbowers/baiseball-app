<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { lockDetail } from "../lib/copy.js";

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
      <p class="muted">A league holds divisions and teams. ULMG is 16 teams in four divisions. NPL is 24 teams in six divisions. Those are examples. You name the leagues and the divisions.</p>
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
      <p class="muted">Create one to add divisions and teams. You become the administrator of a league you create.</p>
      <a class="button" href="/leagues/new">Create a league</a>
    </div>
  {/if}
  {#if !loading && !error}
    {#each leagues as league (league.id)}
      <article class="card stack">
        <h2><a class="link" href="/leagues/{league.id}">{league.name}</a></h2>
        <p class="muted">{lockDetail(league.lineupLock)}</p>
        <p class:danger={!league.inBounds}>{league.boundLabel}</p>
        <p class="muted">{league.divisionCount} {league.divisionCount === 1 ? "division" : "divisions"}</p>
        <a class="button secondary" href="/leagues/{league.id}">Open league</a>
      </article>
    {/each}
  {/if}
</div>

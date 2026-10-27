<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { typeLabel } from "../lib/copy.js";

  let { leagueId } = $props();
  let schedule = $state(null);
  let loading = $state(true);
  let error = $state("");

  let rounds = $derived.by(() => {
    const groups = new Map();
    for (const game of schedule?.games ?? []) {
      const list = groups.get(game.round) ?? [];
      list.push(game);
      groups.set(game.round, list);
    }
    return [...groups.entries()]
      .sort((left, right) => left[0] - right[0])
      .map(([round, games]) => ({ round, games }));
  });

  let totals = $derived.by(() => {
    const map = new Map();
    for (const game of schedule?.games ?? []) {
      for (const side of [
        [game.homeTeamId, game.homeTeamName],
        [game.awayTeamId, game.awayTeamName],
      ]) {
        const row = map.get(side[0]) ?? { name: side[1], games: 0, inside: 0, outside: 0, rivalry: 0 };
        row.games += 1;
        if (game.sameDivision) row.inside += 1;
        else row.outside += 1;
        if (game.rivalry) row.rivalry += 1;
        map.set(side[0], row);
      }
    }
    return [...map.values()].sort((left, right) => left.name.localeCompare(right.name));
  });

  async function load() {
    loading = true;
    error = "";
    try {
      schedule = await api(`/leagues/${leagueId}/schedule`);
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The schedule could not be loaded.";
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

{#if loading}
  <div class="status" role="status">Loading the schedule…</div>
{:else if error || !schedule}
  <div class="alert error" role="alert">
    <strong>Something went wrong</strong>
    <p>{error || "That schedule is not in the office."}</p>
    <button class="secondary" type="button" onclick={load}>Try again</button>
  </div>
{:else}
  <div class="stack">
    <a class="link" href="/leagues/{leagueId}">Back to the league</a>
    <h1>Schedule</h1>
    <p class="muted">{typeLabel(schedule.leagueType)}. {schedule.seasonLength ?? "—"} games for each team. {schedule.divisionGames ?? "—"} inside the division, {schedule.outsideGames ?? "—"} outside it.</p>
    {#if schedule.games.length === 0}
      <div class="empty stack">
        <h2>No schedule yet</h2>
        <p class="muted">Save the season settings, add the teams, then build the schedule from the league.</p>
        <a class="button" href="/leagues/{leagueId}">Back to the league</a>
      </div>
    {:else}
      {#if schedule.matchesConfiguration}
        <p>This schedule matches the saved settings. {schedule.games.length} games on file.</p>
      {:else}
        <div class="alert error" role="alert">
          <strong>The saved settings have changed</strong>
          <p>This schedule was built for a {schedule.builtSeasonLength}-game {typeLabel(schedule.builtLeagueType)} season, {schedule.builtDivisionGames} inside and {schedule.builtOutsideGames} outside. Build again to replace it.</p>
        </div>
      {/if}
      <section class="card stack">
        <h2>Each team</h2>
        <ul class="totals">
          {#each totals as team (team.name)}
            <li>{team.name}: {team.games} games, {team.inside} inside, {team.outside} outside{#if team.rivalry}, {team.rivalry} rivalry{/if}</li>
          {/each}
        </ul>
      </section>
      {#each rounds as round (round.round)}
        <section class="stack">
          <h2>Round {round.round}</h2>
          <div class="schedule" role="table" aria-label="Round {round.round}">
            <div class="schedule-head" role="row">
              <span>Away</span>
              <span>Home</span>
              <span>Where</span>
            </div>
            {#each round.games as game (game.id)}
              <div class="schedule-row" role="row">
                <span>{game.awayTeamName}</span>
                <span>{game.homeTeamName}</span>
                <span>{game.sameDivision ? "Inside the division" : "Outside the division"}{game.rivalry ? " · Rivalry" : ""}</span>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    {/if}
  </div>
{/if}

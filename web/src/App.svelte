<script>
  import { match, route } from "./lib/nav.js";
  import LeaguePage from "./pages/LeaguePage.svelte";
  import Leagues from "./pages/Leagues.svelte";
  import LineupPage from "./pages/LineupPage.svelte";
  import NewLeague from "./pages/NewLeague.svelte";
  import SchedulePage from "./pages/SchedulePage.svelte";
  import TeamPage from "./pages/TeamPage.svelte";
  import Teams from "./pages/Teams.svelte";

  let view = $derived(match($route));
  let leaguesOn = $derived(["leagues", "new-league", "league", "schedule"].includes(view.name));
  let teamsOn = $derived(["teams", "team", "new-lineup", "lineup"].includes(view.name));
</script>

<svelte:head>
  <title>League office</title>
</svelte:head>

<header class="header">
  <div class="wrap header-inner" style="padding: 0.8rem 0">
    <a class="brand" href="/leagues">
      <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><path fill="#8d2f2f" d="M2.5 1.5h13v8.4L9 16.5 2.5 9.9V1.5z"/></svg>
      <span>
        <strong>League office</strong>
        <p class="muted">Leagues, teams, lineups, schedule</p>
      </span>
    </a>
    <nav class="row" aria-label="Office">
      <a class="button {leaguesOn ? 'secondary' : 'ghost'}" href="/leagues" aria-current={leaguesOn ? "page" : undefined}>Leagues</a>
      <a class="button {teamsOn ? 'secondary' : 'ghost'}" href="/teams" aria-current={teamsOn ? "page" : undefined}>Teams</a>
    </nav>
  </div>
</header>

<main class="wrap main">
  {#if view.name === "later"}
    <div class="empty stack">
      <h1>Sign-in is later</h1>
      <p class="muted">The office opens on leagues, divisions, teams, lineups, and the schedule.</p>
      <a class="button" href="/leagues">Go to leagues</a>
    </div>
  {:else if view.name === "leagues"}
    <Leagues />
  {:else if view.name === "new-league"}
    <NewLeague />
  {:else if view.name === "league"}
    {#key view.id}<LeaguePage leagueId={view.id} />{/key}
  {:else if view.name === "schedule"}
    {#key view.id}<SchedulePage leagueId={view.id} />{/key}
  {:else if view.name === "teams"}
    <Teams />
  {:else if view.name === "team"}
    {#key view.id}<TeamPage teamId={view.id} />{/key}
  {:else if view.name === "new-lineup"}
    {#key view.teamId}<LineupPage teamId={view.teamId} />{/key}
  {:else if view.name === "lineup"}
    {#key view.lineupId}<LineupPage teamId={view.teamId} lineupId={view.lineupId} />{/key}
  {:else}
    <div class="empty stack">
      <h1>That page is not in the office</h1>
      <a class="button" href="/leagues">Back to leagues</a>
    </div>
  {/if}
</main>

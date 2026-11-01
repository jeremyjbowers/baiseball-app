<script>
  import { api } from "./lib/api.js";
  import { match, navigate, route } from "./lib/nav.js";
  import { clearSession, getSession } from "./lib/session.js";
  import Account from "./pages/Account.svelte";
  import LeaguePage from "./pages/LeaguePage.svelte";
  import Leagues from "./pages/Leagues.svelte";
  import LineupPage from "./pages/LineupPage.svelte";
  import NewLeague from "./pages/NewLeague.svelte";
  import SchedulePage from "./pages/SchedulePage.svelte";
  import Sent from "./pages/Sent.svelte";
  import SignIn from "./pages/SignIn.svelte";
  import TeamPage from "./pages/TeamPage.svelte";
  import Teams from "./pages/Teams.svelte";
  import Verify from "./pages/Verify.svelte";

  let view = $derived(match($route));
  let session = $derived.by(() => {
    $route;
    return getSession();
  });
  let office = $derived(["leagues", "new-league", "league", "schedule", "teams", "team", "new-lineup", "lineup", "account"].includes(view.name));
  let signingOut = $state(false);

  $effect(() => {
    if (office && !getSession()) navigate("/");
  });

  async function signOut() {
    signingOut = true;
    try {
      await api("/auth/sessions", { method: "DELETE" });
    } catch {
      // The local session is cleared either way.
    }
    clearSession();
    signingOut = false;
    navigate("/");
  }
</script>

<svelte:head>
  <title>League office</title>
</svelte:head>

<header class="header">
  <nav class="nav" aria-label="Office">
    <a class="brand" href={office && session ? "/leagues" : "/"}>League office</a>
    {#if office && session}
      <a class:current={view.name.includes("league") || view.name === "schedule"} href="/leagues">Leagues</a>
      <a class:current={view.name.includes("team") || view.name.includes("lineup")} href="/teams">Teams</a>
      <a class:current={view.name === "account"} href="/account">Account</a>
      <span class="who">{session.email}</span>
      <button class="linkish" type="button" disabled={signingOut} onclick={signOut}>{signingOut ? "Signing out…" : "Sign out"}</button>
    {/if}
  </nav>
</header>

<main class="wrap main">
  {#if view.name === "sign-in"}
    <SignIn expired={view.params.get("link") === "expired"} />
  {:else if view.name === "sent"}
    <Sent />
  {:else if view.name === "verify"}
    <Verify token={view.params.get("token") ?? ""} />
  {:else if office && !session}
    <div class="status" role="status">Sign in to continue.</div>
  {:else if view.name === "account"}
    <Account />
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

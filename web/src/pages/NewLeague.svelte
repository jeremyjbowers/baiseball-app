<script>
  import { api } from "../lib/api.js";
  import { countNote } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import LeagueFields from "../lib/LeagueFields.svelte";
  import RosterFile from "../lib/RosterFile.svelte";

  let name = $state("");
  let leagueType = $state("");
  let seasonLength = $state("");
  let divisionGames = $state("");
  let outsideGames = $state("");
  let rosterCsv = $state("");
  let rosterName = $state("");
  let pending = $state(false);
  let error = $state("");
  let note = $derived(countNote(seasonLength, divisionGames, outsideGames));

  async function submit(event) {
    event.preventDefault();
    if (!note.ok) {
      error = note.text;
      return;
    }
    pending = true;
    error = "";
    try {
      const payload = await api("/leagues", {
        method: "POST",
        body: JSON.stringify({
          name,
          leagueType,
          seasonLength: note.season,
          divisionGames: note.inside,
          outsideGames: note.outside,
          ...(rosterCsv.trim() ? { rosterCsv } : {}),
        }),
      });
      navigate(`/leagues/${payload.league.id}`);
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The league could not be created.";
      pending = false;
    }
  }
</script>

<form class="stack narrow" style="margin-top:0" onsubmit={submit}>
  <div>
    <h1>Create a league</h1>
    <p class="muted">like strat-o-matic plays one game at a time, a season behind, and locks lineups by game. like scoresheet simulates a week, mixes current and past stats, and locks lineups by week. Name the divisions yourself, or upload a roster file. Team count stays between 10 and 30. 26 and 28 are closed.</p>
  </div>
  {#if error}<div class="alert error" role="alert"><strong>Could not save</strong><p>{error}</p></div>{/if}
  <LeagueFields bind:name bind:leagueType bind:seasonLength bind:divisionGames bind:outsideGames />
  <RosterFile bind:csv={rosterCsv} bind:fileName={rosterName} />
  <p class="muted">Rivalries in the roster file count toward the inside or outside total. They are not added on top of the season. You can name more rivalries after the league exists.</p>
  <div class="row">
    <button type="submit" disabled={pending || !note.ok || !leagueType}>{pending ? "Creating the league…" : "Create league"}</button>
    <a class="button secondary" href="/leagues">Cancel</a>
  </div>
</form>

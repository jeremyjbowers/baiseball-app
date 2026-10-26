<script>
  import { api } from "../lib/api.js";
  import { LOCKS } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import Field from "../lib/Field.svelte";

  let name = $state("");
  let lineupLock = $state("");
  let pending = $state(false);
  let error = $state("");
  let detail = $derived(LOCKS.find((lock) => lock.value === lineupLock)?.detail ?? "");

  async function submit(event) {
    event.preventDefault();
    pending = true;
    error = "";
    try {
      const payload = await api("/leagues", {
        method: "POST",
        body: JSON.stringify({ name, lineupLock }),
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
    <p class="muted">You name it. ULMG locks a lineup for a game. NPL locks a lineup for a week. Pick the lock this league uses. Team count stays between 10 and 30. 26 and 28 are closed.</p>
  </div>
  {#if error}<div class="alert error" role="alert"><strong>Could not save</strong><p>{error}</p></div>{/if}
  <Field id="league-name" label="League name">
    <input id="league-name" bind:value={name} />
  </Field>
  <Field id="lineup-lock" label="Lineup lock" hint={detail}>
    <select id="lineup-lock" bind:value={lineupLock} required>
      <option value="" disabled>Choose when lineups lock</option>
      {#each LOCKS as lock (lock.value)}
        <option value={lock.value}>{lock.label}</option>
      {/each}
    </select>
  </Field>
  <div class="row">
    <button type="submit" disabled={pending}>{pending ? "Creating the league…" : "Create league"}</button>
    <a class="button secondary" href="/leagues">Cancel</a>
  </div>
</form>

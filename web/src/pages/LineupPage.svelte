<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { emptyCard, lockDetail, POSITIONS } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import Confirm from "../lib/Confirm.svelte";
  import Field from "../lib/Field.svelte";

  let { teamId, lineupId = "" } = $props();
  let loading = $state(true);
  let error = $state("");
  let formError = $state("");
  let pending = $state(false);
  let readOnly = $state(false);
  let teamName = $state("");
  let leagueName = $state("");
  let lockText = $state("");
  let name = $state("");
  let vsLhp = $state(emptyCard());
  let vsRhp = $state(emptyCard());

  async function load() {
    loading = true;
    error = "";
    try {
      if (lineupId) {
        const payload = await api(`/lineups/${lineupId}`);
        teamName = payload.team.name;
        leagueName = payload.team.leagueName;
        lockText = lockDetail(payload.team.lineupLock);
        name = payload.lineup.name;
        vsLhp = payload.lineup.vsLhp;
        vsRhp = payload.lineup.vsRhp;
        readOnly = !payload.canEditLineups;
      } else {
        const payload = await api(`/teams/${teamId}`);
        teamName = payload.team.name;
        leagueName = payload.team.leagueName;
        lockText = lockDetail(payload.team.lineupLock);
      }
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The lineup could not be loaded.";
    } finally {
      loading = false;
    }
  }

  function update(side, index, patch) {
    const next = (side === "lhp" ? vsLhp : vsRhp).map((slot, slotIndex) => slotIndex === index ? { ...slot, ...patch } : slot);
    if (side === "lhp") vsLhp = next;
    else vsRhp = next;
  }

  async function submit(event) {
    event.preventDefault();
    if (readOnly) return;
    pending = true;
    formError = "";
    const body = JSON.stringify({ name, vsLhp, vsRhp });
    try {
      if (lineupId) await api(`/lineups/${lineupId}`, { method: "PATCH", body });
      else await api(`/teams/${teamId}/lineups`, { method: "POST", body });
      navigate(`/teams/${teamId}`);
    } catch (caught) {
      formError = caught instanceof Error ? caught.message : "The lineup could not be saved.";
      pending = false;
    }
  }

  onMount(load);
</script>

{#if loading}<div class="status" role="status">Loading the lineup…</div>
{:else if error}
  <div class="alert error" role="alert"><strong>Something went wrong</strong><p>{error}</p><button class="secondary" type="button" onclick={load}>Try again</button></div>
{:else}
  <form class="stack" onsubmit={submit}>
    <p class="muted"><a class="link" href="/teams/{teamId}">{teamName}</a> · {leagueName}</p>
    <h1>{readOnly ? name || "Lineup" : lineupId ? "Edit lineup" : "New lineup"}</h1>
    <div class="alert">
      <strong>When this lineup locks</strong>
      <p class="muted">{lockText} This office stores the card. It does not yet hold a change until the next game or week.</p>
    </div>
    {#if formError}<div class="alert error" role="alert"><strong>Could not save</strong><p>{formError}</p></div>{/if}
    <Field id="lineup-name" label="Lineup name" hint="Nine batters. The designated hitter is in the order, and pitchers do not hit. This is the MLB card only.">
      <input id="lineup-name" bind:value={name} disabled={readOnly} />
    </Field>
    <div class="cards">
      {#each [["Versus left-handed pitching", vsLhp, "lhp"], ["Versus right-handed pitching", vsRhp, "rhp"]] as card (card[2])}
        <section class="card stack">
          <h2>{card[0]}</h2>
          {#each card[1] as slot, index (slot.battingOrder)}
            <div class="slot">
              <p class="order">{slot.battingOrder}</p>
              <Field id="{card[2]}-name-{slot.battingOrder}" label="Player name">
                <input id="{card[2]}-name-{slot.battingOrder}" value={slot.playerName} disabled={readOnly} oninput={(event) => update(card[2], index, { playerName: event.currentTarget.value })} />
              </Field>
              <Field id="{card[2]}-position-{slot.battingOrder}" label="Position">
                <select id="{card[2]}-position-{slot.battingOrder}" value={slot.position} disabled={readOnly} onchange={(event) => update(card[2], index, { position: event.currentTarget.value })}>
                  <option value="" disabled>Position</option>
                  {#each POSITIONS as position (position.value)}<option value={position.value}>{position.label}</option>{/each}
                </select>
              </Field>
            </div>
          {/each}
        </section>
      {/each}
    </div>
    {#if readOnly}
      <a class="button secondary" href="/teams/{teamId}">Back to the team</a>
    {:else}
      <div class="spread">
        <button type="submit" disabled={pending}>{pending ? "Saving the lineup…" : "Save lineup"}</button>
        {#if lineupId}
          <Confirm
            label="Delete lineup"
            title={`Delete ${name || "this lineup"}?`}
            description="The two cards are removed. The team stays."
            confirmLabel="Delete lineup"
            onConfirm={async () => { await api(`/lineups/${lineupId}`, { method: "DELETE" }); navigate(`/teams/${teamId}`); }}
          />
        {/if}
      </div>
    {/if}
  </form>
{/if}

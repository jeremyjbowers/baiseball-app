<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { lockDetail, positionLabel } from "../lib/copy.js";
  import { navigate } from "../lib/nav.js";
  import Confirm from "../lib/Confirm.svelte";
  import Field from "../lib/Field.svelte";

  let { teamId } = $props();
  let data = $state(null);
  let loading = $state(true);
  let error = $state("");
  let formError = $state("");
  let name = $state("");
  let divisionId = $state("");
  let homePark = $state("");
  let ownerEmail = $state("");
  let pending = $state(false);

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api(`/teams/${teamId}`);
      data = payload;
      name = payload.team.name;
      divisionId = payload.team.divisionId;
      homePark = payload.team.homePark;
      ownerEmail = payload.team.ownerEmail;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The team could not be loaded.";
    } finally {
      loading = false;
    }
  }

  async function submit(event) {
    event.preventDefault();
    pending = true;
    formError = "";
    try {
      const payload = await api(`/teams/${data.team.id}`, {
        method: "PATCH",
        body: JSON.stringify({
          name,
          divisionId,
          homePark,
          ...(data.viewerIsAdmin ? { ownerEmail } : {}),
        }),
      });
      data = { ...data, team: { ...data.team, ...payload.team } };
      name = payload.team.name;
      divisionId = payload.team.divisionId;
      homePark = payload.team.homePark;
      ownerEmail = payload.team.ownerEmail;
    } catch (caught) {
      formError = caught instanceof Error ? caught.message : "The team could not be saved.";
    } finally {
      pending = false;
    }
  }

  onMount(load);
</script>

{#if loading}<div class="status" role="status">Loading the team…</div>
{:else if error || !data}
  <div class="alert error" role="alert"><strong>Something went wrong</strong><p>{error || "That team is not in the office."}</p><button class="secondary" type="button" onclick={load}>Try again</button></div>
{:else}
  <div class="stack">
    <div class="row">
      <a class="link" href="/teams">Teams you control</a>
      {#if data.viewerIsAdmin}<a class="link" href="/leagues/{data.team.leagueId}">{data.team.leagueName}</a>
      {:else}<span>{data.team.leagueName}</span>{/if}
    </div>
    <h1>{data.team.name}</h1>
    <p class:danger={!data.inBounds}>{data.boundLabel}</p>
    <p class="muted">{lockDetail(data.team.lineupLock)}</p>
    {#if formError}<div class="alert error" role="alert"><strong>Could not save</strong><p>{formError}</p></div>{/if}

    <form class="card stack" onsubmit={submit}>
      <h2>Team</h2>
      <Field id="edit-team-name" label="Team name"><input id="edit-team-name" bind:value={name} /></Field>
      <Field id="edit-division" label="Division">
        <select id="edit-division" bind:value={divisionId}>
          {#each data.divisions as division (division.id)}<option value={division.id}>{division.name}</option>{/each}
        </select>
      </Field>
      <Field id="edit-home-park" label="Home park" hint="The park this team plays in at home."><input id="edit-home-park" bind:value={homePark} /></Field>
      <Field id="edit-owner-email" label="Owner email" hint={data.viewerIsAdmin ? "This owner controls the team and writes its lineups." : "Only the league administrator can change who controls the team."}>
        <input id="edit-owner-email" type="email" bind:value={ownerEmail} disabled={!data.viewerIsAdmin} />
      </Field>
      <button type="submit" disabled={pending}>{pending ? "Saving the team…" : "Save team"}</button>
    </form>

    <section class="stack">
      <div class="spread">
        <div>
          <h2>Lineups</h2>
          <p class="muted">Each lineup is the MLB card only. Store one card versus left-handed pitching and one card versus right-handed pitching.</p>
        </div>
        {#if data.canEditLineups}<a class="button" href="/teams/{data.team.id}/lineups/new">Add a lineup</a>
        {:else}<p class="muted">Lineups are written by the owner who controls this team.</p>{/if}
      </div>
      {#if data.lineups.length === 0}
        <div class="empty stack">
          <h3>No lineup cards yet</h3>
          <p class="muted">Add the MLB lineup versus left-handed pitching and versus right-handed pitching.</p>
          {#if data.canEditLineups}<a class="button" href="/teams/{data.team.id}/lineups/new">Add a lineup</a>{/if}
        </div>
      {/if}
      {#each data.lineups as lineup (lineup.id)}
        <article class="card stack">
          <h3>{lineup.name}</h3>
          <div class="cards">
            {#each [["Versus left-handed pitching", lineup.vsLhp], ["Versus right-handed pitching", lineup.vsRhp]] as card (card[0])}
              <div>
                <h3>{card[0]}</h3>
                <ol>
                  {#each card[1] as slot (slot.battingOrder)}
                    <li>{slot.battingOrder}. {slot.playerName} <span class="muted">· {positionLabel(slot.position)}</span></li>
                  {/each}
                </ol>
              </div>
            {/each}
          </div>
          <a class="button secondary" href="/teams/{data.team.id}/lineups/{lineup.id}">{data.canEditLineups ? "Edit lineup" : "View lineup"}</a>
        </article>
      {/each}
    </section>

    <Confirm
      label="Delete team"
      title={`Delete ${data.team.name}?`}
      description={data.teamCount === 10 ? "A league that has reached 10 teams stays at 10 or more. Change the team, or delete the league to start over." : "This removes the team and its lineups."}
      confirmLabel="Delete team"
      disabled={data.teamCount === 10}
      disabledReason={data.teamCount === 10 ? "Removal is blocked at 10 teams so the league does not fall under the minimum." : ""}
      onConfirm={async () => {
        await api(`/teams/${data.team.id}`, { method: "DELETE" });
        navigate(data.viewerIsAdmin ? `/leagues/${data.team.leagueId}` : "/teams");
      }}
    />
  </div>
{/if}

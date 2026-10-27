<script>
  import { TYPES, countNote, typeDetail } from "./copy.js";
  import Field from "./Field.svelte";

  let {
    name = $bindable(),
    leagueType = $bindable(),
    seasonLength = $bindable(),
    divisionGames = $bindable(),
    outsideGames = $bindable(),
  } = $props();

  let note = $derived(countNote(seasonLength, divisionGames, outsideGames));
</script>

<Field id="league-name" label="League name">
  <input id="league-name" bind:value={name} autocomplete="off" />
</Field>

<fieldset class="stack">
  <legend>League type</legend>
  {#each TYPES as type (type.value)}
    <label class="choice">
      <input type="radio" name="league-type" value={type.value} bind:group={leagueType} />
      <span>
        <strong>{type.label}</strong>
        <span class="muted">{type.detail}</span>
      </span>
    </label>
  {/each}
  {#if leagueType}<p class="hint">{typeDetail(leagueType)}</p>{/if}
</fieldset>

<div class="counts">
  <Field id="season-length" label="Season length" hint="Games each team plays.">
    <input id="season-length" inputmode="numeric" bind:value={seasonLength} />
  </Field>
  <Field id="division-games" label="Inside the division" hint="Games against division opponents.">
    <input id="division-games" inputmode="numeric" bind:value={divisionGames} />
  </Field>
  <Field id="outside-games" label="Outside the division" hint="Games against everyone else.">
    <input id="outside-games" inputmode="numeric" bind:value={outsideGames} />
  </Field>
</div>
<p class:danger={!note.ok}>{note.text}</p>

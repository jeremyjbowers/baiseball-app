<script>
  let { csv = $bindable(""), fileName = $bindable(""), idPrefix = "roster" } = $props();
  let inputEl = $state(null);

  $effect(() => {
    if (!fileName && inputEl) inputEl.value = "";
  });

  async function onChange(event) {
    const file = event.currentTarget.files?.[0];
    fileName = file?.name ?? "";
    csv = file ? await file.text() : "";
  }
</script>

<fieldset class="stack">
  <legend>Roster file</legend>
  <p class="muted">Optional. Download the template, replace the example rows, and upload it. One row creates one team. A division that is not already in the league is created from the division column.</p>
  <p class="hint">managers takes one or more emails separated by semicolons, so a team can have more than one manager. Leave that cell blank and the office holds the team. rivalries is optional and looks like Wharf:4 or Wharf:4;Mesa:2. Those games count toward the inside or outside total. If a park name contains a comma, wrap that cell in quotes. A team name that is already in the league stops the upload, and nothing from that file is saved.</p>
  <a class="button secondary" href="/league-roster-template.csv" download="league-roster-template.csv">Download the template</a>
  <div class="field">
    <label for="{idPrefix}-file">CSV file</label>
    <input id="{idPrefix}-file" type="file" accept=".csv,text/csv" bind:this={inputEl} onchange={onChange} />
  </div>
  {#if fileName}<p class="hint">{fileName} is ready.</p>{/if}
</fieldset>

<script>
  let {
    emails = $bindable(),
    disabled = false,
    idPrefix = "manager",
    hint = "Add every manager. Each one can sign in and write the lineup.",
  } = $props();

  function add() {
    emails = [...emails, ""];
  }

  function remove(index) {
    emails = emails.filter((_, item) => item !== index);
  }

  function set(index, value) {
    emails = emails.map((email, item) => (item === index ? value : email));
  }
</script>

<fieldset class="stack" {disabled}>
  <legend>Managers</legend>
  <p class="hint">{hint}</p>
  {#if emails.length === 0}
    <p class="muted">No manager yet. The office holds the team until you add one.</p>
  {/if}
  {#each emails as email, index (index)}
    <div class="manager-row" class:single={disabled}>
      <div class="field">
        <label for="{idPrefix}-{index}">{emails.length > 1 ? `Manager ${index + 1}` : "Manager email"}</label>
        <input
          id="{idPrefix}-{index}"
          type="email"
          value={email}
          {disabled}
          autocomplete="off"
          oninput={(event) => set(index, event.currentTarget.value)}
        />
      </div>
      {#if !disabled}
        <button class="secondary" type="button" onclick={() => remove(index)}>Remove</button>
      {/if}
    </div>
  {/each}
  {#if !disabled}
    <button class="secondary" type="button" onclick={add}>Add a manager</button>
  {/if}
</fieldset>

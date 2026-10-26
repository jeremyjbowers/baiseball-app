<script>
  let { label, title, description, confirmLabel, onConfirm, disabled = false, disabledReason = "" } = $props();
  let dialog = $state();
  let pending = $state(false);
  let error = $state("");

  async function confirm() {
    pending = true;
    error = "";
    try {
      await onConfirm();
      dialog.close();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The office could not complete that.";
      pending = false;
    }
  }
</script>

<div class="stack">
  {#if disabled && disabledReason}<p class="hint">{disabledReason}</p>{/if}
  <button class="danger" type="button" disabled={disabled} onclick={() => dialog.showModal()}>{label}</button>
  <dialog bind:this={dialog}>
    <div class="stack">
      <h2>{title}</h2>
      <p class="muted">{description}</p>
      {#if error}<div class="alert error" role="alert"><strong>Could not delete</strong><p>{error}</p></div>{/if}
      <div class="row">
        <button class="secondary" type="button" disabled={pending} onclick={() => dialog.close()}>Keep it</button>
        <button class="danger" type="button" disabled={pending} onclick={confirm}>{pending ? "Deleting…" : confirmLabel}</button>
      </div>
    </div>
  </dialog>
</div>

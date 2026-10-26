<script>
  let { id, label, hint = "", children } = $props();
</script>

<div class="field">
  <label for={id}>{label}</label>
  {@render children()}
  {#if hint}<p class="hint">{hint}</p>{/if}
</div>

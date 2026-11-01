<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";

  let email = $state("");
  let isAdmin = $state(false);
  let loading = $state(true);
  let error = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api("/auth/session");
      email = payload.owner.email;
      isAdmin = payload.owner.isAdmin;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The account could not be loaded.";
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

{#if loading}
  <div class="status" role="status">Loading your account…</div>
{:else}
  <div class="stack narrow" style="margin-top:0">
    <div>
      <h1>Your account</h1>
      <p class="muted">{email}{isAdmin ? " · administrator" : ""}</p>
    </div>
    {#if error}<div class="alert error" role="alert"><strong>Could not load</strong><p>{error}</p></div>{/if}
    <p class="muted">Sign-in links go to this email. Text messages are turned off for now.</p>
  </div>
{/if}

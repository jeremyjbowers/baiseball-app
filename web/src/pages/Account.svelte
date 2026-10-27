<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import Field from "../lib/Field.svelte";

  let email = $state("");
  let phone = $state("");
  let isAdmin = $state(false);
  let loading = $state(true);
  let pending = $state(false);
  let error = $state("");
  let saved = $state("");

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api("/auth/session");
      email = payload.owner.email;
      phone = payload.owner.phone ?? "";
      isAdmin = payload.owner.isAdmin;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The account could not be loaded.";
    } finally {
      loading = false;
    }
  }

  async function submit(event) {
    event.preventDefault();
    pending = true;
    error = "";
    saved = "";
    try {
      const payload = await api("/owners/me", {
        method: "PATCH",
        body: JSON.stringify({ phone }),
      });
      phone = payload.owner.phone ?? "";
      saved = phone ? "Phone number saved. You can ask for a text at sign-in." : "Phone number removed.";
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The phone number could not be saved.";
    } finally {
      pending = false;
    }
  }

  onMount(load);
</script>

{#if loading}
  <div class="status" role="status">Loading your account…</div>
{:else}
  <form class="stack narrow" style="margin-top:0" onsubmit={submit}>
    <div>
      <h1>Your account</h1>
      <p class="muted">{email}{isAdmin ? " · administrator" : ""}</p>
    </div>
    {#if error}<div class="alert error" role="alert"><strong>Could not save</strong><p>{error}</p></div>{/if}
    {#if saved}<div class="alert"><strong>Saved</strong><p class="muted">{saved}</p></div>{/if}
    <Field id="account-phone" label="Phone number" hint="Optional. Include the country code, starting with +. Leave it blank to remove it. The office does not invent a number.">
      <input id="account-phone" type="tel" bind:value={phone} autocomplete="tel" />
    </Field>
    <button type="submit" disabled={pending}>{pending ? "Saving…" : "Save phone number"}</button>
  </form>
{/if}

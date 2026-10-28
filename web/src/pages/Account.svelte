<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import Field from "../lib/Field.svelte";
  import SmsConsent from "../lib/SmsConsent.svelte";

  let email = $state("");
  let phone = $state("");
  let savedPhone = $state("");
  let isAdmin = $state(false);
  let loading = $state(true);
  let pending = $state(false);
  let error = $state("");
  let saved = $state("");
  let smsTransactional = $state(false);
  let smsTerms = $state(false);

  const phoneChanged = $derived(phone.trim() !== savedPhone.trim());
  const savingPhone = $derived(phone.trim().length > 0);
  const needsConsent = $derived(savingPhone && phoneChanged);

  async function load() {
    loading = true;
    error = "";
    try {
      const payload = await api("/auth/session");
      email = payload.owner.email;
      phone = payload.owner.phone ?? "";
      savedPhone = payload.owner.phone ?? "";
      isAdmin = payload.owner.isAdmin;
      smsTransactional = false;
      smsTerms = false;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The account could not be loaded.";
    } finally {
      loading = false;
    }
  }

  async function submit(event) {
    event.preventDefault();
    if (needsConsent && (!smsTransactional || !smsTerms)) {
      error = "Confirm the text-message consent and the Terms before saving a phone number.";
      return;
    }
    pending = true;
    error = "";
    saved = "";
    try {
      const body =
        !savingPhone
          ? { phone: "" }
          : needsConsent
            ? { phone, smsConsent: true, termsConsent: true }
            : { phone };
      const payload = await api("/owners/me", {
        method: "PATCH",
        body: JSON.stringify(body),
      });
      phone = payload.owner.phone ?? "";
      savedPhone = payload.owner.phone ?? "";
      smsTransactional = false;
      smsTerms = false;
      saved = phone
        ? "Phone number saved. You opted in to transactional League office texts. You can ask for a text at sign-in. Reply STOP to any text to opt out."
        : "Phone number removed. You will not receive League office texts.";
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
    <Field id="account-phone" label="Mobile phone number" hint="Optional. Include the country code, starting with +. Leave it blank to remove it and stop texts. The office does not invent a number.">
      <input id="account-phone" type="tel" bind:value={phone} autocomplete="tel" placeholder="+1 (555) 000-0000" />
    </Field>
    {#if needsConsent}
      <SmsConsent idPrefix="account" purpose="account" bind:transactional={smsTransactional} bind:terms={smsTerms} />
    {/if}
    <button type="submit" disabled={pending || (needsConsent && (!smsTransactional || !smsTerms))}>
      {pending ? "Saving…" : savingPhone ? "Yes, save my phone number" : "Remove phone number"}
    </button>
    {#if savedPhone}
      <p class="hint">Texts are transactional sign-in links and account alerts only. Reply STOP to opt out, or clear this field and save.</p>
    {/if}
  </form>
{/if}

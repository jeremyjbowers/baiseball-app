<script>
  import { api } from "../lib/api.js";
  import { navigate } from "../lib/nav.js";
  import { setPending } from "../lib/pending.js";
  import Field from "../lib/Field.svelte";

  let { expired = false } = $props();
  let channel = $state("email");
  let pending = $state(false);
  let error = $state("");

  $effect(() => {
    if (expired) error = "That sign-in link is expired or already used. Request another.";
  });

  async function submit(event) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const email = String(data.get("email") ?? "").trim();
    const phone = String(data.get("phone") ?? "").trim();
    pending = true;
    error = "";
    try {
      const issued = await api("/auth/magic-links", {
        method: "POST",
        body: JSON.stringify(channel === "sms" ? { phone } : { email }),
      });
      setPending(issued);
      navigate("/sign-in/sent");
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "The link could not be created.";
      pending = false;
    }
  }
</script>

<div class="narrow stack">
  <div class="brand">
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><path fill="#8d2f2f" d="M2.5 1.5h13v8.4L9 16.5 2.5 9.9V1.5z"/></svg>
    <strong>League office</strong>
  </div>
  <form class="card stack" onsubmit={submit}>
    <div>
      <h1>Sign in</h1>
      <p class="muted">Use the email on your club, or a phone number already saved on your owner record. The office sends a one-time link.</p>
    </div>
    {#if error}<div class="alert error" role="alert"><strong>Could not sign in</strong><p>{error}</p></div>{/if}
    <fieldset class="stack">
      <legend>Send the link by</legend>
      <label class="choice">
        <input type="radio" name="channel" value="email" bind:group={channel} />
        <span><strong>Email</strong><span class="muted">A new address can sign in this way.</span></span>
      </label>
      <label class="choice">
        <input type="radio" name="channel" value="sms" bind:group={channel} />
        <span><strong>Text</strong><span class="muted">Only a phone number already saved on an owner.</span></span>
      </label>
    </fieldset>
    {#if channel === "sms"}
      <Field id="phone" label="Phone number" hint="Include the country code, starting with +.">
        <input id="phone" name="phone" type="tel" autocomplete="tel" required />
      </Field>
    {:else}
      <Field id="email" label="Email address">
        <input id="email" name="email" type="email" autocomplete="email" required />
      </Field>
    {/if}
    <button type="submit" disabled={pending}>{pending ? "Sending the link…" : channel === "sms" ? "Text me a sign-in link" : "Email me a sign-in link"}</button>
    <p class="hint">If Mailgun or Twilio is not configured on this machine, the link is shown on the next screen. That confirmation is for local development only.</p>
  </form>
</div>

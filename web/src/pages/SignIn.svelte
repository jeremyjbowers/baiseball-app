<script>
  import { api } from "../lib/api.js";
  import { navigate } from "../lib/nav.js";
  import { setPending } from "../lib/pending.js";
  import Field from "../lib/Field.svelte";

  let { expired = false } = $props();
  let pending = $state(false);
  let error = $state("");

  $effect(() => {
    if (expired) error = "That sign-in link is expired or already used. Request another.";
  });

  async function submit(event) {
    event.preventDefault();
    const email = String(new FormData(event.currentTarget).get("email") ?? "").trim();
    pending = true;
    error = "";
    try {
      const issued = await api("/auth/magic-links", {
        method: "POST",
        body: JSON.stringify({ email }),
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
      <p class="muted">Use the email address on your club. The office sends a one-time link.</p>
    </div>
    {#if error}<div class="alert error" role="alert"><strong>Could not sign in</strong><p>{error}</p></div>{/if}
    <Field id="email" label="Email address">
      <input id="email" name="email" type="email" autocomplete="email" required />
    </Field>
    <button type="submit" disabled={pending}>{pending ? "Sending the link…" : "Email me a sign-in link"}</button>
    <p class="hint">This machine has no mail provider. The link is printed in the server log and shown on the next screen. That confirmation is for local development only.</p>
  </form>
</div>

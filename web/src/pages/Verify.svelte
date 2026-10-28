<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { navigate } from "../lib/nav.js";
  import { setSession } from "../lib/session.js";

  let { token = "" } = $props();
  let error = $state("");

  onMount(async () => {
    try {
      const issued = await api("/auth/sessions", {
        method: "POST",
        body: JSON.stringify({ token }),
      });
      setSession({
        token: issued.sessionToken,
        email: issued.owner.email,
        ownerId: issued.owner.id,
        expiresAt: issued.expiresAt,
      });
      navigate("/leagues");
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "That sign-in link is expired or already used.";
      navigate("/sign-in?link=expired");
    }
  });
</script>

<div class="narrow">
  <div class="status" role="status">{error || "Signing in…"}</div>
</div>

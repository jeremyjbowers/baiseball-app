<script>
  import { getPending } from "../lib/pending.js";
  import { formatWhen } from "../lib/copy.js";

  const issued = getPending();
</script>

<div class="narrow">
  {#if !issued?.magicLink}
    <div class="empty stack">
      <h1>No sign-in link in this browser</h1>
      <p class="muted">Request another link from the sign-in page. The server log still has the last link that was issued.</p>
      <a class="button" href="/">Back to sign in</a>
    </div>
  {:else}
    <div class="card stack">
      <div>
        <h1>Sign-in link</h1>
        <p class="muted">For {issued.email}</p>
      </div>
      <div class="alert">
        <strong>Local development only</strong>
        <p class="muted">No email was sent. Use the link below, or the copy in the server log.</p>
      </div>
      <div class="stack">
        <p class="slot-label">Magic link</p>
        <p class="mono card">{issued.magicLink}</p>
      </div>
      <a class="button" href={issued.magicLink}>Open the sign-in link</a>
      <p class="hint">The link expires {formatWhen(issued.expiresAt)}. It works once.</p>
      <a class="link" href="/">Use a different email</a>
    </div>
  {/if}
</div>

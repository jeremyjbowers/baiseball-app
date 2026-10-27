<script>
  import { getPending } from "../lib/pending.js";
  import { formatWhen } from "../lib/copy.js";

  const issued = getPending();
  const byText = issued?.channel === "sms";
</script>

<div class="narrow">
  {#if !issued}
    <div class="empty stack">
      <h1>No sign-in link in this browser</h1>
      <p class="muted">Request another link from the sign-in page.</p>
      <a class="button" href="/">Back to sign in</a>
    </div>
  {:else if issued.devOnly && issued.magicLink}
    <div class="card stack">
      <div>
        <h1>Sign-in link</h1>
        <p class="muted">{byText ? `Text for ${issued.phone}` : `Email for ${issued.email}`}</p>
      </div>
      <div class="alert">
        <strong>Local development only</strong>
        <p class="muted">{byText ? "No text was sent. Twilio is not configured." : "No email was sent. Mailgun is not configured."} Use the link below, or the copy in the API log.</p>
      </div>
      <div class="stack">
        <p class="slot-label">Magic link</p>
        <p class="mono card">{issued.magicLink}</p>
      </div>
      <a class="button" href={issued.magicLink}>Open the sign-in link</a>
      <p class="hint">The link expires {formatWhen(issued.expiresAt)}. It works once.</p>
      <a class="link" href="/">Use a different address</a>
    </div>
  {:else}
    <div class="card stack">
      <h1>{byText ? "Check your phone" : "Check your email"}</h1>
      <p class="muted">A sign-in link was sent to {issued.destination}. It works once and expires {formatWhen(issued.expiresAt)}.</p>
      {#if byText}
        <div class="sms-disclosure" role="note">
          <p><strong>Msg &amp; data rates may apply.</strong> Reply <strong>STOP</strong> to opt out of League office texts. Reply <strong>HELP</strong> for help.</p>
        </div>
      {/if}
      <a class="link" href="/">Use a different address</a>
    </div>
  {/if}
</div>

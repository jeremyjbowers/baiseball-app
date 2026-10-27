<script>
  import { PRIVACY_URL, TERMS_URL } from "./site.js";

  let {
    idPrefix = "sms",
    transactional = $bindable(false),
    terms = $bindable(false),
    purpose = "sign-in",
  } = $props();

  const transactionalId = $derived(`${idPrefix}-transactional`);
  const termsId = $derived(`${idPrefix}-terms`);
  const transactionalLabel = $derived(
    purpose === "account"
      ? "Transactional texts from League office"
      : "Transactional sign-in texts from League office",
  );
  const transactionalHint = $derived(
    purpose === "account"
      ? "One-time sign-in links and account alerts for this league office. Frequency depends on when you ask for a text."
      : "A one-time sign-in link for this league office. Frequency depends on when you ask for a text.",
  );
</script>

<div class="stack sms-consent">
  <label class="choice" for={transactionalId}>
    <input id={transactionalId} type="checkbox" bind:checked={transactional} required />
    <span>
      <strong>{transactionalLabel}</strong>
      <span class="muted">{transactionalHint}</span>
    </span>
  </label>
  <label class="choice" for={termsId}>
    <input id={termsId} type="checkbox" bind:checked={terms} required />
    <span>
      <strong>I have read and agree to the <a class="link" href={TERMS_URL} target="_blank" rel="noopener">Terms of Service</a> and <a class="link" href={PRIVACY_URL} target="_blank" rel="noopener">Privacy Policy</a>.</strong>
      <span class="muted">This agreement is separate from text-message consent.</span>
    </span>
  </label>
  <div class="sms-disclosure" role="note">
    <p><strong>Msg &amp; data rates may apply.</strong> Message frequency varies. You may receive up to 8 messages per month. Consent is not a condition of using the league office.</p>
    <p>Reply <strong>STOP</strong> to opt out of texts at any time. Reply <strong>HELP</strong> for help, or email <a class="link" href="mailto:jeremyjbowers@gmail.com">jeremyjbowers@gmail.com</a>.</p>
    <p>For more information, see our <a class="link" href={TERMS_URL} target="_blank" rel="noopener">Terms of Service</a> and <a class="link" href={PRIVACY_URL} target="_blank" rel="noopener">Privacy Policy</a>.</p>
  </div>
</div>

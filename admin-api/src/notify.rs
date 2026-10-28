use std::process::Command;

use serde_json::Value;

use crate::error::ApiError;

pub fn send_email(to: &str, link: &str) -> Result<bool, ApiError> {
    let key = env("MAILGUN_API_KEY");
    let domain = env("MAILGUN_DOMAIN");
    let configured_from = env("MAILGUN_FROM");
    if key.is_empty() && domain.is_empty() && configured_from.is_empty() {
        println!("league-office magic-link email={to} url={link}");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        return Ok(true);
    }
    if key.is_empty() || domain.is_empty() {
        return Err(ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Mailgun is only partly configured.",
        ));
    }
    let from = if configured_from.is_empty() {
        format!("League office <noreply@{domain}>")
    } else {
        configured_from
    };
    let status = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/tmp/league-office-mail.out",
            "-w",
            "%{http_code}",
            "-u",
            &format!("api:{key}"),
            &format!("https://api.mailgun.net/v3/{domain}/messages"),
            "-F",
            &format!("from={from}"),
            "-F",
            &format!("to={to}"),
            "-F",
            "subject=Your league office sign-in link",
            "-F",
            &format!("text=Open this link to sign in. It works once and expires in 15 minutes.\n\n{link}"),
        ])
        .output()
        .map_err(|_| ApiError::internal())?;
    accept_http(&status.stdout, "The sign-in email could not be sent.")
}

pub fn send_sms(to: &str, link: &str) -> Result<bool, ApiError> {
    let sid = first_env(&["TWILIO_SID", "TWILIO_ACCOUNT_SID"]);
    let secret = first_env(&["TWILIO_CLIENT_SECRET", "TWILIO_AUTH_TOKEN"]);
    let configured_from = env("TWILIO_FROM");
    if sid.is_empty() && secret.is_empty() && configured_from.is_empty() {
        println!("league-office magic-link sms phone={to} url={link}");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        return Ok(true);
    }
    if sid.is_empty() || secret.is_empty() {
        return Err(ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Twilio is only partly configured.",
        ));
    }
    let account = twilio_account(&sid, &secret)?;
    let from = if configured_from.is_empty() {
        twilio_from_number(&sid, &secret, &account)?
    } else {
        configured_from
    };
    let status = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/tmp/league-office-sms.out",
            "-w",
            "%{http_code}",
            "-u",
            &format!("{sid}:{secret}"),
            "-X",
            "POST",
            &format!("https://api.twilio.com/2010-04-01/Accounts/{account}/Messages.json"),
            "--data-urlencode",
            &format!("To={to}"),
            "--data-urlencode",
            &format!("From={from}"),
            "--data-urlencode",
            &format!(
                "Body=League office sign-in link. It works once and expires in 15 minutes. {link}"
            ),
        ])
        .output()
        .map_err(|_| ApiError::internal())?;
    accept_http(&status.stdout, "The sign-in text could not be sent.")
}

fn twilio_account(sid: &str, secret: &str) -> Result<String, ApiError> {
    if sid.starts_with("AC") {
        return Ok(sid.to_string());
    }
    let body = curl_body(
        &[
            "-sS",
            "-u",
            &format!("{sid}:{secret}"),
            "-o",
            "/tmp/league-office-twilio-account.out",
            "-w",
            "%{http_code}",
            "https://api.twilio.com/2010-04-01/Accounts.json",
        ],
        "/tmp/league-office-twilio-account.out",
        "Twilio did not return an account.",
    )?;
    account_sid(&body).ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            "Twilio did not return an account.",
        )
    })
}

fn twilio_from_number(sid: &str, secret: &str, account: &str) -> Result<String, ApiError> {
    let body = curl_body(
        &[
            "-sS",
            "-u",
            &format!("{sid}:{secret}"),
            "-o",
            "/tmp/league-office-twilio-numbers.out",
            "-w",
            "%{http_code}",
            &format!(
                "https://api.twilio.com/2010-04-01/Accounts/{account}/IncomingPhoneNumbers.json?PageSize=50"
            ),
        ],
        "/tmp/league-office-twilio-numbers.out",
        "Twilio did not return a phone number.",
    )?;
    sms_number(&body).ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Set TWILIO_FROM. The Twilio account has no SMS number.",
        )
    })
}

fn curl_body(args: &[&str], path: &str, failure: &str) -> Result<String, ApiError> {
    let status = Command::new("curl")
        .args(args)
        .output()
        .map_err(|_| ApiError::internal())?;
    accept_http(&status.stdout, failure)?;
    std::fs::read_to_string(path).map_err(|_| ApiError::internal())
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default().trim().to_string()
}

fn first_env(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| env(name))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn accept_http(stdout: &[u8], failure: &str) -> Result<bool, ApiError> {
    let code = String::from_utf8_lossy(stdout);
    if code.starts_with('2') {
        Ok(false)
    } else {
        eprintln!("delivery status {code}");
        Err(ApiError::new(axum::http::StatusCode::BAD_GATEWAY, failure))
    }
}

fn account_sid(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let accounts = value.get("accounts")?.as_array()?;
    let chosen = accounts
        .iter()
        .find(|account| account.get("status").and_then(Value::as_str) == Some("active"))
        .or_else(|| accounts.first())?;
    let sid = chosen.get("sid")?.as_str()?;
    if sid.starts_with("AC") && sid.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(sid.to_string())
    } else {
        None
    }
}

fn sms_number(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let numbers = value.get("incoming_phone_numbers")?.as_array()?;
    let chosen = numbers
        .iter()
        .find(|number| {
            number
                .get("capabilities")
                .and_then(|caps| caps.get("sms"))
                .and_then(Value::as_bool)
                == Some(true)
        })
        .or_else(|| numbers.first())?;
    let phone = chosen.get("phone_number")?.as_str()?;
    if phone.starts_with('+') {
        Some(phone.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{account_sid, first_env, sms_number};

    #[test]
    fn reads_the_first_configured_name() {
        std::env::remove_var("NOTIFY_TEST_PRIMARY");
        std::env::set_var("NOTIFY_TEST_FALLBACK", "secret");
        assert_eq!(
            first_env(&["NOTIFY_TEST_PRIMARY", "NOTIFY_TEST_FALLBACK"]),
            "secret"
        );
        std::env::remove_var("NOTIFY_TEST_FALLBACK");
    }

    #[test]
    fn uses_the_active_twilio_account() {
        let body = r#"{"accounts":[{"sid":"ACsuspended000000000000000000000","status":"suspended"},{"sid":"ACactive000000000000000000000000","status":"active"}]}"#;
        assert_eq!(
            account_sid(body).as_deref(),
            Some("ACactive000000000000000000000000")
        );
    }

    #[test]
    fn prefers_an_sms_capable_number() {
        let body = r#"{"incoming_phone_numbers":[{"phone_number":"+15550001111","capabilities":{"sms":false}},{"phone_number":"+15550002222","capabilities":{"sms":true}}]}"#;
        assert_eq!(sms_number(body).as_deref(), Some("+15550002222"));
    }
}

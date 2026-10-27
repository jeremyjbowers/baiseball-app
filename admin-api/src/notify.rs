use std::process::Command;

use crate::error::ApiError;

pub fn send_email(to: &str, link: &str) -> Result<bool, ApiError> {
    let key = env("MAILGUN_API_KEY");
    let domain = env("MAILGUN_DOMAIN");
    let from = env("MAILGUN_FROM");
    if key.is_empty() && domain.is_empty() && from.is_empty() {
        println!("league-office magic-link email={to} url={link}");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        return Ok(true);
    }
    if key.is_empty() || domain.is_empty() || from.is_empty() {
        return Err(ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Mailgun is only partly configured.",
        ));
    }
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
    let sid = env("TWILIO_ACCOUNT_SID");
    let token = env("TWILIO_AUTH_TOKEN");
    let from = env("TWILIO_FROM");
    if sid.is_empty() && token.is_empty() && from.is_empty() {
        println!("league-office magic-link sms phone={to} url={link}");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        return Ok(true);
    }
    if sid.is_empty() || token.is_empty() || from.is_empty() {
        return Err(ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Twilio is only partly configured.",
        ));
    }
    let status = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/tmp/league-office-sms.out",
            "-w",
            "%{http_code}",
            "-u",
            &format!("{sid}:{token}"),
            "-X",
            "POST",
            &format!("https://api.twilio.com/2010-04-01/Accounts/{sid}/Messages.json"),
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

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default().trim().to_string()
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

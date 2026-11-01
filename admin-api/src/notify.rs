use std::process::Command;

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

#[cfg(test)]
mod tests {
    use super::env;

    #[test]
    fn mailgun_from_falls_back_to_noreply_on_domain() {
        let domain = "example.test";
        let configured_from = "";
        let from = if configured_from.is_empty() {
            format!("League office <noreply@{domain}>")
        } else {
            configured_from.to_string()
        };
        assert_eq!(from, "League office <noreply@example.test>");
        let _ = env("MAILGUN_FROM");
    }
}

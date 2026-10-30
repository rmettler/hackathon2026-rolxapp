//! Implements the RFC 6749 OAuth 2.0 for native applications/native apps based on the recommendations of RFC 8252 and RFC 9700.

mod authorization_request;
mod token_request;

use rand::CryptoRng;
use std::time::Duration;
use thiserror;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::Instant;

pub use token_request::Response;

/// How long the access token request is retried while the network is unreachable, if `Cfg::retry_while_offline` is
/// set. Google's authorization codes expire after 10 minutes.
const TOKEN_RETRY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const TOKEN_RETRY_INTERVAL: Duration = Duration::from_secs(2);

pub struct Cfg {
    /// URL of the authorization endpoint, authenticating the user and responding with the authorization code
    pub authorization_endpoint: String,
    /// URL of the token endpoint, trading the authorization code to a bearer token
    pub token_endpoint: String,
    /// Identification for the authorization endpoint
    pub client_id: String,
    /// Secret required by some authorization endpoints, even so the key will be leaked by public native apps
    pub client_secret: Option<String>,
    /// The email the user wants to authenticate with. If provided, may simplify authorization in the external user-agent (browser)
    pub user_email: Option<String>,
    /// URL that opens the app, linked from the result page in the browser so the user gets back to the app with a
    /// tap, e.g. a custom URI scheme on mobile
    pub return_url: Option<String>,
    /// Retries the access token request while the network is unreachable, instead of failing at once. Needed on
    /// Android, which blocks the network of background apps until the user is back in the app.
    pub retry_while_offline: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("authorization failed: {0}")]
    Authorization(#[from] authorization_request::Error),

    #[error("exchanging access token failed: {0}")]
    TokenExchange(#[from] token_request::Error),

    #[error("showing success page failed: {0}")]
    SuccessPage(std::io::Error),

    #[error("showing error page failed: {0}")]
    #[allow(clippy::enum_variant_names)]
    ErrorPage(std::io::Error),
}

/// Calls `manual_auth_action` with the authorization request URL if the browser could not be opened.
pub async fn run(
    http_client: &reqwest::Client,
    rng: &mut impl CryptoRng,
    cfg: &Cfg,
    scope: &str,
    manual_auth_action: impl FnOnce(&str),
) -> Result<Response, Error> {
    let req = authorization_request::create_authorization_request(
        rng,
        &cfg.authorization_endpoint,
        &cfg.client_id,
        scope,
        cfg.user_email.as_deref(),
    )
    .await?;

    let mut redirect_stream = authorization_request::open_in_browser_or(
        req.redirect_listener,
        &req.auth_request_url,
        manual_auth_action,
    )
    .await?;

    let auth_result =
        authorization_request::retrieve_auth_code(&mut redirect_stream, &req.state).await;

    // Answers the browser before exchanging the token: on Android, the app is in the background while the user is in
    // the browser, and Android blocks the network of background apps. The exchange succeeds once the user is back.
    match &auth_result {
        Ok(_) => show_success_page(redirect_stream, cfg.return_url.as_deref())
            .await
            .map_err(Error::SuccessPage)?,
        Err(error) => show_error_page(
            redirect_stream,
            &error.to_string(),
            cfg.return_url.as_deref(),
        )
        .await
        .map_err(Error::ErrorPage)?,
    };
    let auth_code = auth_result.map_err(authorization_request::Error::from)?;

    let deadline = Instant::now() + TOKEN_RETRY_TIMEOUT;
    loop {
        match token_request::request_access_token(
            http_client,
            &cfg.token_endpoint,
            &cfg.client_id,
            cfg.client_secret.as_deref(),
            &req.redirect_uri,
            &req.code_verifier,
            &auth_code,
        )
        .await
        {
            // Only connection errors: the request was not sent, so the authorization code is still unused.
            Err(token_request::Error::Http(e))
                if cfg.retry_while_offline && e.is_connect() && Instant::now() < deadline =>
            {
                tracing::debug!(error = %e, "exchanging access token failed, retrying");
                tokio::time::sleep(TOKEN_RETRY_INTERVAL).await;
            }
            result => return result.map_err(Into::into),
        }
    }
}

/// Writes an authentication success page to the given stream, linking `return_url` if given
async fn show_success_page(mut stream: TcpStream, return_url: Option<&str>) -> std::io::Result<()> {
    const HTML: &str = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        include_str!("native_app_flow/success_page.html")
    );
    let html = HTML.replace("{{return_link}}", &return_link(return_url));
    stream.write_all(html.as_bytes()).await?;
    Ok(())
}

/// Writes an error page showing the given error message to the given stream, linking `return_url` if given
async fn show_error_page(
    mut stream: TcpStream,
    error: &str,
    return_url: Option<&str>,
) -> std::io::Result<()> {
    const HTML: &str = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        include_str!("native_app_flow/error_page.html")
    );
    let html = HTML
        .replace("{{return_link}}", &return_link(return_url))
        .replace("{{error}}", &escape_html(error));
    stream.write_all(html.as_bytes()).await?;
    Ok(())
}

/// Returns the "Back to RolX" button for the result pages, or nothing without a `return_url`
fn return_link(return_url: Option<&str>) -> String {
    return_url.map_or_else(String::new, |url| {
        format!(
            r#"<a class="return" href="{}">Back to RolX</a>"#,
            escape_html(url)
        )
    })
}

/// Escapes the characters with special meaning in HTML text content and attribute values
fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

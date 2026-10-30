//! Implements the RFC 6749 OAuth 2.0 for native applications/native apps based on the recommendations of RFC 8252 and RFC 9700.

mod authorization_request;
mod token_request;

use crate::AuthPrompt;
use rand::CryptoRng;
use thiserror;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub use token_request::Response;

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

pub async fn run(
    http_client: &reqwest::Client,
    rng: &mut impl CryptoRng,
    cfg: &Cfg,
    scope: &str,
    prompt: &dyn AuthPrompt,
) -> Result<Response, Error> {
    let req = authorization_request::create_authorization_request(
        rng,
        &cfg.authorization_endpoint,
        &cfg.client_id,
        scope,
        cfg.user_email.as_deref(),
    )
    .await?;

    let mut redirect_stream = authorization_request::open_in_browser_or_ask(
        req.redirect_listener,
        &req.auth_request_url,
        prompt,
    )
    .await?;

    let token_result: Result<Response, Error> = (async move |redirect_stream| {
        let auth_code = authorization_request::retrieve_auth_code(redirect_stream, &req.state)
            .await
            .map_err(Into::<authorization_request::Error>::into)?;
        token_request::request_access_token(
            http_client,
            &cfg.token_endpoint,
            &cfg.client_id,
            cfg.client_secret.as_deref(),
            &req.redirect_uri,
            req.code_verifier,
            &auth_code,
        )
        .await
        .map_err(Into::into)
    })(&mut redirect_stream)
    .await;

    match &token_result {
        Ok(_) => show_success_page(redirect_stream)
            .await
            .map_err(Error::SuccessPage)?,
        Err(error) => show_error_page(redirect_stream, &error.to_string())
            .await
            .map_err(Error::ErrorPage)?,
    };
    token_result
}

/// Writes an authentication success page to the given stream
async fn show_success_page(mut stream: TcpStream) -> std::io::Result<()> {
    const HTML: &[u8] = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        include_str!("native_app_flow/success_page.html")
    )
    .as_bytes();
    stream.write_all(HTML).await?;
    Ok(())
}

/// Writes an error page showing the given error message to the given stream
async fn show_error_page(mut stream: TcpStream, error: &str) -> std::io::Result<()> {
    const HTML: &str = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        include_str!("native_app_flow/error_page.html")
    );
    let html = HTML.replace("{{error}}", &escape_html(error));
    stream.write_all(html.as_bytes()).await?;
    Ok(())
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

use crate::oauth2::pkce;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::CryptoRng;
use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

pub struct AuthRequest {
    pub redirect_uri: String,
    pub redirect_listener: TcpListener,
    pub state: String,
    pub auth_request_url: String,
    pub code_verifier: pkce::CodeVerifier,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    AuthResponse(#[from] AuthResponseError),
}

pub async fn create_authorization_request(
    rng: &mut impl CryptoRng,
    auth_endpoint: &str,
    client_id: &str,
    scope: &str,
    user_email: Option<&str>,
) -> Result<AuthRequest, Error> {
    let response_addrs = [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ]
    .map(|ip| SocketAddr::new(ip, 0));
    let redirect_listener = TcpListener::bind(response_addrs.as_slice()).await?;
    let redirect_uri = format!("http://{}", redirect_listener.local_addr()?);

    let (code_verifier, code_challenge) = pkce::generate_s256::<32>(rng);

    // Sent with auth request and received with auth response to ensure that the response comes from
    // the server the request was sent to.
    let state = create_state(rng);

    let mut params = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri.as_str()),
        ("scope", scope),
        ("code_challenge", code_challenge.as_str()),
        ("code_challenge_method", code_challenge.method_as_str()),
        ("state", state.as_str()),
    ];
    params.extend(user_email.into_iter().map(|email| ("login_hint", email)));

    let query = build_query(&params);
    let auth_request_url = format!("{auth_endpoint}?{query}");
    Ok(AuthRequest {
        redirect_listener,
        redirect_uri,
        state,
        code_verifier,
        auth_request_url,
    })
}

/// Generates random 32 bytes as a base64url encoded string.
fn create_state(rng: &mut impl CryptoRng) -> String {
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Builds an URL query segment from given key/value params. URL encodes the params.
fn build_query(query_params: &[(&str, &str)]) -> String {
    query_params
        .iter()
        .map(|(key, val)| [urlencoding::encode(key), urlencoding::encode(val)].join("="))
        .collect::<Vec<_>>()
        .join("&")
}

/// Opens the default browser to request the user's authorization.
/// If the browser could not be opened, calls the `manual_action` with the authorization request URL.
/// Consumes the TCP listener and returns a TCP stream listening on the `redirect_uri`.
pub async fn open_in_browser_or(
    redirect_listener: TcpListener,
    auth_request_url: &str,
    manual_auth_action: impl FnOnce(&str),
) -> Result<TcpStream, Error> {
    tracing::debug!(
        method = "GET",
        url = auth_request_url,
        "request user authorization"
    );
    match open::that(auth_request_url) {
        Ok(()) => tracing::info!("Opened browser for user authorization…"),
        Err(e) => {
            tracing::warn!(error = %e, "Opening browser failed; open URL manually");
            manual_auth_action(auth_request_url);
        }
    }
    let (redirect_stream, _) = redirect_listener.accept().await?;
    Ok(redirect_stream)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthResponseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parsing failed: {0}")]
    ParsingFailed(#[from] AuthResponseParseError),

    #[error("endpoint responded with error: {0}")]
    EndpointError(AuthErrorResponse),

    #[error("invalid state: expected '{0}', received '{1:?}'")]
    InvalidState(String, Option<String>),

    #[error("authorization code missing")]
    AuthCodeMissing,
}

/// Returns the authorization code retrieved on the passed TCP stream. Errors if there is no state received equal to
/// the passed state.
pub async fn retrieve_auth_code(
    stream: &mut TcpStream,
    state: &str,
) -> Result<String, AuthResponseError> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);

    #[cfg(not(feature = "uncensored_logs"))]
    tracing::debug!(
        addr = stream
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|e| e.to_string()),
        "authorization loopback response"
    );
    #[cfg(feature = "uncensored_logs")]
    tracing::debug!(
        addr = stream.local_addr().map(|a| a.to_string()).unwrap_or_else(|e| e.to_string()),
        response = %response,
        "authorization loopback response"
    );

    let parsed = parse_auth_response(&response)?;
    if Some(state) != parsed.state.as_deref() {
        return Err(AuthResponseError::InvalidState(
            state.to_string(),
            parsed.state,
        ));
    }
    if let Some(e) = parsed.error {
        return Err(AuthResponseError::EndpointError(e));
    }
    parsed.code.ok_or(AuthResponseError::AuthCodeMissing)
}

/// Parsed data, the authorization endpoint sent to the redirection URI.
#[derive(Debug, Default)]
struct AuthResponse {
    code: Option<String>,
    error: Option<AuthErrorResponse>,
    state: Option<String>,
}

/// Error, the authorization endpoint may send with the authorization response to the redirection URI.
#[derive(Debug)]
pub struct AuthErrorResponse {
    /// Expected error codes by RFC 6749 are:
    /// invalid_request, unauthorized_client, access_denied, unsupported_response_type,
    /// invalid_scope, server_error, temporarily_unavailable
    error: String,
    description: Option<String>,
    uri: Option<String>,
}

impl Display for AuthErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error '{}'", self.error)?;
        if let Some(descr) = &self.description {
            write!(f, ", error description '{}'", descr)?;
        }
        if let Some(uri) = &self.uri {
            write!(f, ", error URI '{}'", uri)?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthResponseParseError {
    #[error("no authorization response")]
    NoAuthorizationResponse,

    #[error("missing response uri")]
    MissingResponseUri,

    #[error("missing query")]
    MissingQuery,

    #[error("invalid query")]
    InvalidQuery,

    #[error("invalid encoding of query param: {0}")]
    InvalidQueryParamEnc(String),
}

/// Parses the authorization response. First line should look like:  
/// `GET /?code=XXXX&scope=... HTTP/1.1`
fn parse_auth_response(response: &str) -> Result<AuthResponse, AuthResponseParseError> {
    let first_line = response
        .lines()
        .next()
        .ok_or(AuthResponseParseError::NoAuthorizationResponse)?;

    let response_uri = first_line
        .split_whitespace()
        .nth(1)
        .ok_or(AuthResponseParseError::MissingResponseUri)?;

    let query = response_uri
        .split('?')
        .nth(1)
        .ok_or(AuthResponseParseError::MissingQuery)?;

    let mut resp: AuthResponse = Default::default();
    let mut error = None;
    let mut error_description = None;
    let mut error_uri = None;

    for param in query.split('&') {
        let (key, val) = param
            .split_once('=')
            .ok_or(AuthResponseParseError::InvalidQuery)?;
        let decode_val = || {
            urlencoding::decode(val)
                .map_err(|_| AuthResponseParseError::InvalidQueryParamEnc(val.to_string()))
        };
        match key {
            "state" => resp.state = Some(decode_val()?.into()),
            "code" => resp.code = Some(decode_val()?.into()),
            "error" => error = Some(decode_val()?.into()),
            "error_description" => error_description = Some(decode_val()?.into()),
            "error_uri" => error_uri = Some(decode_val()?.into()),
            _ => (),
        };
    }
    if let Some(err) = error {
        resp.error = Some(AuthErrorResponse {
            error: err,
            description: error_description,
            uri: error_uri,
        });
    }
    Ok(resp)
}

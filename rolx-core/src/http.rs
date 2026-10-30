/// Returns a `reqwest` client builder with the TLS settings every RolX HTTP client must use.
///
/// On Android, `reqwest`'s default `rustls-platform-verifier` panics unless it was initialized via JNI and its
/// Kotlin classes are bundled in the APK, so the Mozilla root certificates are used instead.
pub fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
    );
    builder
}

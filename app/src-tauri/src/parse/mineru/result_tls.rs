//! TLS for MinerU result downloads, with one exception: when the result CDN's
//! certificate has expired and Settings → Parsing allows it (the default), the
//! certificate is re-verified as of its last valid second — chain, signatures
//! and host name are all still checked, only the clock is excused. Any other
//! host or certificate error is refused as usual, and a renewed certificate
//! passes the ordinary check, so the exception lapses on its own.
//!
//! Every handshake with the CDN records what it found (`state`), which is how
//! the UI knows to warn while the exception is in use.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::ring;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use serde::Serialize;

pub const RESULT_HOST: &str = "cdn-mineru.openxlab.org.cn";

/// The agent `download_zip` uses: the verifier below, and stall timeouts
/// rather than a deadline (`client::TRANSFER_STALL`).
pub fn agent(accept_expired: bool) -> &'static ureq::Agent {
    static ACCEPTING: OnceLock<ureq::Agent> = OnceLock::new();
    static STRICT: OnceLock<ureq::Agent> = OnceLock::new();
    let slot = if accept_expired { &ACCEPTING } else { &STRICT };
    slot.get_or_init(|| {
        ureq::AgentBuilder::new()
            .tls_config(Arc::new(config(accept_expired)))
            .timeout_connect(super::client::TRANSFER_STALL)
            .timeout_read(super::client::TRANSFER_STALL)
            .build()
    })
}

/// A handshake with the CDN and nothing more, so `state` is current. Free: no
/// MinerU API call, no quota.
pub fn probe(accept_expired: bool) -> CertState {
    let _ = agent(accept_expired)
        .head(&format!("https://{RESULT_HOST}/"))
        .timeout(Duration::from_secs(10))
        .call();
    state(accept_expired)
}

// ── What the last handshake found ────────────────────────────────────────────

const UNKNOWN: u8 = 0;
const VALID: u8 = 1;
const EXPIRED: u8 = 2;

static SEEN: AtomicU8 = AtomicU8::new(UNKNOWN);
static NOT_AFTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CertState {
    /// `"unknown"` (no handshake yet this session) | `"valid"` | `"expired"`.
    pub certificate: &'static str,
    /// The expiry, as Unix seconds, while `certificate` is `"expired"`.
    pub expired_at: Option<u64>,
    /// Expired and allowed: downloads are going through the exception.
    pub bypassing: bool,
}

pub fn state(accept_expired: bool) -> CertState {
    let seen = SEEN.load(Ordering::Relaxed);
    let expired = seen == EXPIRED;
    CertState {
        certificate: match seen {
            VALID => "valid",
            EXPIRED => "expired",
            _ => "unknown",
        },
        expired_at: expired.then(|| NOT_AFTER.load(Ordering::Relaxed)),
        bypassing: expired && accept_expired,
    }
}

fn record(seen: u8, not_after: u64) {
    NOT_AFTER.store(not_after, Ordering::Relaxed);
    SEEN.store(seen, Ordering::Relaxed);
}

// ── The verifier ─────────────────────────────────────────────────────────────

fn webpki() -> Arc<WebPkiServerVerifier> {
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::new(ring::default_provider()))
        .build()
        .expect("the webpki root set is not empty")
}

fn config(accept_expired: bool) -> rustls::ClientConfig {
    let verifier = ExpiredResultCert {
        inner: webpki(),
        host: RESULT_HOST,
        accept_expired,
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .expect("ring supports TLS 1.2 and 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth()
}

#[derive(Debug)]
struct ExpiredResultCert {
    inner: Arc<WebPkiServerVerifier>,
    host: &'static str,
    accept_expired: bool,
}

impl ExpiredResultCert {
    fn is_ours(&self, server_name: &ServerName<'_>) -> bool {
        matches!(server_name, ServerName::DnsName(name) if name.as_ref() == self.host)
    }
}

/// The `notAfter` of an expired certificate; `None` for every other failure.
fn expired_at(error: &Error) -> Option<UnixTime> {
    match error {
        Error::InvalidCertificate(CertificateError::ExpiredContext { not_after, .. }) => {
            Some(*not_after)
        }
        _ => None,
    }
}

impl ServerCertVerifier for ExpiredResultCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let verdict = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        if !self.is_ours(server_name) {
            return verdict;
        }
        match verdict {
            Ok(verified) => {
                record(VALID, 0);
                Ok(verified)
            }
            Err(error) => match expired_at(&error) {
                Some(not_after) => {
                    record(EXPIRED, not_after.as_secs());
                    if !self.accept_expired {
                        return Err(error);
                    }
                    self.inner.verify_server_cert(
                        end_entity,
                        intermediates,
                        server_name,
                        ocsp_response,
                        not_after,
                    )
                }
                None => Err(error),
            },
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_expiry_is_excused() {
        let not_after = UnixTime::since_unix_epoch(Duration::from_secs(1_790_985_599));
        let expired = Error::InvalidCertificate(CertificateError::ExpiredContext {
            time: UnixTime::since_unix_epoch(Duration::from_secs(1_791_225_603)),
            not_after,
        });
        assert_eq!(expired_at(&expired), Some(not_after));
        for other in [
            CertificateError::NotValidForName,
            CertificateError::UnknownIssuer,
            CertificateError::BadSignature,
            CertificateError::Revoked,
        ] {
            assert_eq!(expired_at(&Error::InvalidCertificate(other)), None);
        }
    }

    #[test]
    fn only_the_result_host_is_ours() {
        let verifier = ExpiredResultCert {
            inner: webpki(),
            host: RESULT_HOST,
            accept_expired: true,
        };
        let name = |host: &str| ServerName::try_from(host.to_string()).unwrap();
        assert!(verifier.is_ours(&name(RESULT_HOST)));
        assert!(!verifier.is_ours(&name("mineru.net")));
        assert!(!verifier.is_ours(&name("evil.openxlab.org.cn")));
    }

    /// Needs the network, and the CDN's certificate still expired.
    #[test]
    #[ignore]
    fn live_cdn_connects_only_when_allowed() {
        let url = format!("https://{RESULT_HOST}/");
        let strict = agent(false).head(&url).call();
        assert!(
            matches!(strict, Err(ureq::Error::Transport(_))),
            "{strict:?}"
        );
        assert_eq!(state(false).certificate, "expired");
        let allowed = agent(true).head(&url).call();
        assert!(
            !matches!(allowed, Err(ureq::Error::Transport(_))),
            "{allowed:?}"
        );
        assert!(state(true).bypassing);
    }
}

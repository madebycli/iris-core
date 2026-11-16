use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use dcap_qvl::{
    collateral::CollateralClient, http::HttpClient, quote::Report, verify::rustcrypto::verify,
};
use k256::PublicKey;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct Attested {
    pub identity: EnclaveIdentity,
    pub key: PublicKey,
    pub key_hex: String,
}

struct CollateralTransport<'a>(&'a Client);
impl HttpClient for CollateralTransport<'_> {
    async fn get(&self, url: &str) -> anyhow::Result<dcap_qvl::http::HttpResponse> {
        // Collateral is public; the host must never attach the Venice credential here.
        let parsed = url::Url::parse(url)?;
        anyhow::ensure!(
            parsed.scheme() == "https"
                && parsed.host_str() == Some("pccs.phala.network")
                && parsed.port_or_known_default() == Some(443)
                && parsed.username().is_empty()
                && parsed.password().is_none(),
            "collateral origin rejected"
        );
        let response = self
            .0
            .http(HttpRequest {
                method: "GET".into(),
                url: url.into(),
                headers: BTreeMap::new(),
                body: vec![],
            })
            .map_err(|_| anyhow::anyhow!("collateral unavailable"))?;
        Ok(dcap_qvl::http::HttpResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
        })
    }
}

impl Client {
    pub(crate) fn attest(&self, model: &str) -> Result<Attested> {
        let nonce = crypto::nonce();
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("model", model)
            .append_pair("nonce", &nonce)
            .finish();
        let evidence = self.get(&format!("tee/attestation?{query}"))?;
        check_envelope(&evidence, model, &nonce)?;
        let key_hex = evidence
            .get("signing_key")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| evidence.get("signing_public_key").and_then(Value::as_str))
            .ok_or(Error::Attestation)?;
        let key = crypto::public_key(key_hex)?;
        let quote_text = evidence
            .get("intel_quote")
            .and_then(Value::as_str)
            .ok_or(Error::Attestation)?;
        if quote_text.len() > 1024 * 1024 {
            return Err(Error::Limit);
        }
        // Venice's documented encoding is base64; the CLI also exposes hex quotes.
        let hex_quote = quote_text.strip_prefix("0x").unwrap_or(quote_text);
        let quote = if hex_quote.len().is_multiple_of(2)
            && hex_quote.bytes().all(|b| b.is_ascii_hexdigit())
        {
            hex::decode(hex_quote).map_err(|_| Error::Attestation)?
        } else {
            STANDARD
                .decode(quote_text)
                .map_err(|_| Error::Attestation)?
        };
        let collateral = futures::executor::block_on(
            CollateralClient::<dcap_qvl::configs::DefaultConfig, _>::new(
                CollateralTransport(self),
                dcap_qvl::PHALA_PCCS_URL,
            )
            .fetch(&quote),
        )
        .map_err(|_| Error::Attestation)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Attestation)?
            .as_secs();
        // Intel production root, certificate/CRL validity, quote signature and QE/TCB policy
        // are checked by dcap-qvl. No test roots or verification bypasses in production.
        let verified = verify(&quote, &collateral, now).map_err(|_| Error::Attestation)?;
        let report = match &verified.report {
            Report::TD10(r) => r,
            Report::TD15(r) => &r.base,
            _ => return Err(Error::Attestation),
        };
        if report.td_attributes[0] & 1 != 0 || verified.status != "UpToDate" {
            return Err(Error::Attestation);
        }
        let address = crypto::address(&key);
        if report.report_data[..20] != address {
            return Err(Error::Attestation);
        }
        check_nonce_binding(&report.report_data, &nonce)?;
        if let Some(reported) = evidence.get("signing_address").and_then(Value::as_str) {
            if reported.trim_start_matches("0x").to_ascii_lowercase() != hex::encode(address) {
                return Err(Error::Attestation);
            }
        }
        let rtmrs = vec![
            hex::encode(report.rt_mr0),
            hex::encode(report.rt_mr1),
            hex::encode(report.rt_mr2),
            hex::encode(report.rt_mr3),
        ];
        let mrtd = hex::encode(report.mr_td);
        let policy = format!(
            "iris-venice-runtime-v1\n{model}\n{mrtd}\n{}",
            rtmrs.join("\n")
        );
        Ok(Attested {
            key,
            key_hex: key_hex.to_owned(),
            identity: EnclaveIdentity {
                model: model.into(),
                fingerprint: hex::encode(Sha256::digest(policy)),
                mrtd,
                rtmrs,
                tcb_status: verified.status,
            },
        })
    }
}

/// Locate the full fresh 32-byte nonce (or SHA-256 representation) in the signed
/// REPORTDATA after the 20-byte key binding. A server's `bound: true` is insufficient.
/// Unknown provider layouts fail closed and require an explicitly reviewed protocol update.
fn check_nonce_binding(report_data: &[u8; 64], nonce: &str) -> Result<()> {
    let nonce_bytes = hex::decode(nonce).map_err(|_| Error::Attestation)?;
    if nonce_bytes.len() != 32 {
        return Err(Error::Attestation);
    }
    let hashed_bytes = Sha256::digest(&nonce_bytes);
    let hashed_hex = Sha256::digest(nonce.as_bytes());
    if !report_data[20..].windows(32).any(|bytes| {
        bytes == nonce_bytes || bytes == hashed_bytes.as_slice() || bytes == hashed_hex.as_slice()
    }) {
        return Err(Error::Attestation);
    }
    Ok(())
}

fn check_envelope(e: &Value, model: &str, nonce: &str) -> Result<()> {
    if e.get("verified").and_then(Value::as_bool) != Some(true)
        || e.get("nonce").and_then(Value::as_str) != Some(nonce)
        || e.get("model").and_then(Value::as_str) != Some(model)
        || e.pointer("/server_verification/nonceBinding/bound")
            .and_then(Value::as_bool)
            != Some(true)
        || e.pointer("/server_verification/tdx/valid")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(Error::Attestation);
    }
    // NVIDIA evidence is additionally verified by Venice. We do NOT label that field as
    // client-side NVIDIA verification; see the documented trust boundary.
    if let Some(payload) = e.get("nvidia_payload").filter(|v| !v.is_null()) {
        let payload = if let Some(text) = payload.as_str() {
            serde_json::from_str::<Value>(text).map_err(|_| Error::Attestation)?
        } else {
            payload.clone()
        };
        if !payload.is_object()
            || e.pointer("/server_verification/nvidia/valid")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return Err(Error::Attestation);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_nonce_claim_cannot_replace_signed_nonce_binding() {
        let nonce = "17".repeat(32);
        let mut data = [0; 64];
        assert!(check_nonce_binding(&data, &nonce).is_err());
        data[32..].copy_from_slice(&hex::decode(&nonce).unwrap());
        assert!(check_nonce_binding(&data, &nonce).is_ok());
        assert!(check_nonce_binding(&data, &"18".repeat(32)).is_err());
        data[32..].copy_from_slice(&Sha256::digest(hex::decode(&nonce).unwrap()));
        assert!(check_nonce_binding(&data, &nonce).is_ok());
    }
    #[test]
    fn rejects_replay_wrong_model_and_unverified_gpu() {
        let mut e = json!({"verified":true,"nonce":"fresh","model":DEFAULT_MODEL,"server_verification":{"nonceBinding":{"bound":true},"tdx":{"valid":true}}});
        assert!(check_envelope(&e, DEFAULT_MODEL, "fresh").is_ok());
        assert!(check_envelope(&e, DEFAULT_MODEL, "old").is_err());
        assert!(check_envelope(&e, "e2ee-other", "fresh").is_err());
        e["nvidia_payload"] = json!({});
        assert!(check_envelope(&e, DEFAULT_MODEL, "fresh").is_err());
    }
}

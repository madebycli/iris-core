//! Reuse platform TLS, cancellation and credential storage. API keys never cross this boundary.
use std::{collections::BTreeMap, sync::Arc};

#[derive(uniffi::Record)]
pub struct VeniceHeader {
    pub name: String,
    pub value: String,
}
#[derive(uniffi::Record)]
pub struct VeniceRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<VeniceHeader>,
    pub body: Vec<u8>,
}
#[derive(uniffi::Record)]
pub struct VeniceResponse {
    pub status: u16,
    pub headers: Vec<VeniceHeader>,
    pub body: Vec<u8>,
}

#[uniffi::export(callback_interface)]
pub trait VeniceHttp: Send + Sync {
    /// Status 0: transport failure; 499: cancelled. Bound reads to 8 MiB, disable redirects,
    /// keep TLS validation, and never attach the Venice key to a collateral request.
    fn execute(&self, request: VeniceRequest) -> VeniceResponse;
}

struct HttpBridge(Arc<dyn VeniceHttp>);
impl iris_cloud::Transport for HttpBridge {
    fn execute(&self, r: iris_cloud::HttpRequest) -> iris_cloud::Result<iris_cloud::HttpResponse> {
        let r = self.0.execute(VeniceRequest {
            method: r.method,
            url: r.url,
            body: r.body,
            headers: r
                .headers
                .into_iter()
                .map(|(name, value)| VeniceHeader { name, value })
                .collect(),
        });
        match r.status {
            0 => return Err(iris_cloud::Error::Network),
            499 => return Err(iris_cloud::Error::Cancelled),
            _ => {}
        }
        Ok(iris_cloud::HttpResponse {
            status: r.status,
            body: r.body,
            headers: r
                .headers
                .into_iter()
                .map(|h| (h.name, h.value))
                .collect::<BTreeMap<_, _>>(),
        })
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum VeniceError {
    #[error("{code}")]
    Failed { code: String },
}
impl From<iris_cloud::Error> for VeniceError {
    fn from(e: iris_cloud::Error) -> Self {
        Self::Failed {
            code: e.to_string(),
        }
    }
}
fn encode<T: serde::Serialize>(value: &T) -> Result<String, VeniceError> {
    serde_json::to_string(value).map_err(|_| VeniceError::from(iris_cloud::Error::Protocol))
}

#[derive(uniffi::Object)]
pub struct VeniceConnection {
    client: iris_cloud::Client,
}
#[uniffi::export]
impl VeniceConnection {
    #[uniffi::constructor]
    pub fn new(http: Box<dyn VeniceHttp>) -> Arc<Self> {
        Arc::new(Self {
            client: iris_cloud::Client::new(Arc::new(HttpBridge(Arc::from(http)))),
        })
    }
    pub fn models(&self) -> Result<String, VeniceError> {
        encode(&self.client.models()?)
    }
    pub fn inspect(&self, model: String) -> Result<String, VeniceError> {
        encode(&self.client.inspect(&model)?)
    }
    pub fn complete(
        &self,
        model: String,
        trusted_fingerprint: String,
        system: String,
        history: String,
        user: String,
    ) -> Result<String, VeniceError> {
        encode(
            &self
                .client
                .complete(&model, &trusted_fingerprint, &system, &history, &user)?,
        )
    }
}

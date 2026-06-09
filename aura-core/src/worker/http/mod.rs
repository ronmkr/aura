pub(crate) mod crawler;
pub(crate) mod metadata;
pub(crate) mod segment;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "captive_portal_tests.rs"]
mod captive_portal_tests;

#[cfg(test)]
#[path = "tests_conditional_pool.rs"]
mod tests_conditional_pool;

#[cfg(test)]
#[path = "tests_governor.rs"]
mod tests_governor;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientKey {
    pub host: String,
    pub port: Option<u16>,
    pub interface: Option<String>,
}

impl ClientKey {
    pub fn from_uri(uri: &str) -> Option<Self> {
        let url = url::Url::parse(uri).ok()?;
        Some(Self {
            host: url.host_str()?.to_string(),
            port: url.port(),
            interface: None, // Simplified for now
        })
    }
}

#[derive(Clone)]
pub struct ClientPool {
    clients: Arc<Mutex<HashMap<ClientKey, Arc<reqwest::Client>>>>,
}

impl ClientPool {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_create<F>(&self, key: &ClientKey, factory: F) -> Arc<reqwest::Client>
    where
        F: FnOnce() -> reqwest::Client,
    {
        let mut clients = self.clients.lock().unwrap();
        if let Some(client) = clients.get(key) {
            client.clone()
        } else {
            let client = Arc::new(factory());
            clients.insert(key.clone(), client.clone());
            client
        }
    }

    pub fn len(&self) -> usize {
        self.clients.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ClientPool {
    fn default() -> Self {
        Self::new()
    }
}

use crate::worker::builder::WorkerOptions;

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    pub(crate) client: Arc<reqwest::Client>,
    pub(crate) http3_client: Arc<Mutex<Option<reqwest::Client>>>,
    pub(crate) options: WorkerOptions,
}

impl HttpWorker {
    pub fn new(options: WorkerOptions) -> Self {
        let client = if let Some(ref pool) = options.client_pool {
            if let Some(key) = ClientKey::from_uri(&options.uri) {
                pool.get_or_create(&key, || {
                    build_client_from_options(&options, false).build().unwrap()
                })
            } else {
                Arc::new(reqwest::Client::new())
            }
        } else {
            Arc::new(reqwest::Client::new())
        };

        Self {
            client,
            http3_client: Arc::new(Mutex::new(None)),
            options,
        }
    }

    pub(crate) async fn check_and_update_hsts(&self, resp: &reqwest::Response) {
        if let Some(ref cache) = self.options.hsts_cache {
            if let Some(hsts_val) = resp.headers().get(reqwest::header::HeaderName::from_static(
                "strict-transport-security",
            )) {
                if let Ok(hsts_str) = hsts_val.to_str() {
                    if let Some(host) = resp.url().host_str() {
                        cache.insert_header(host.to_string(), hsts_str).await;
                    }
                }
            }
        }
    }

    pub(crate) async fn check_and_update_alt_svc(&self, resp: &reqwest::Response) {
        if let Some(ref cache) = self.options.alt_svc_cache {
            let url = resp.url();
            let scheme = url.scheme();
            if scheme == "https" || scheme == "http" {
                if let Some(alt_svc_val) = resp
                    .headers()
                    .get(reqwest::header::HeaderName::from_static("alt-svc"))
                {
                    if let Ok(alt_svc_str) = alt_svc_val.to_str() {
                        if let Some(host) = url.host_str() {
                            cache.insert_policies(host.to_string(), alt_svc_str).await;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn build_http3_client(&self) -> reqwest::Client {
        build_client_from_options(&self.options, true)
            .build()
            .expect("Failed to build HTTP/3 client")
    }

    pub(crate) fn get_http3_client(&self) -> reqwest::Client {
        let mut lock = self.http3_client.lock().unwrap();
        if let Some(ref client) = *lock {
            client.clone()
        } else {
            let client = self.build_http3_client();
            *lock = Some(client.clone());
            client
        }
    }

    pub(crate) async fn send_request(
        &self,
        url_str: &str,
        mut builder_fn: impl FnMut(&reqwest::Client, &str) -> reqwest::RequestBuilder,
    ) -> std::result::Result<reqwest::Response, reqwest::Error> {
        // 1. Try HTTP/3 if cache has a valid policy
        if let Some(ref cache) = self.options.alt_svc_cache {
            if let Ok(url) = url::Url::parse(url_str) {
                if let Some(host) = url.host_str() {
                    if let Some(policy) = cache.get_alt_svc(host).await {
                        if let Some(rewritten_url) =
                            crate::security::alt_svc::rewrite_url_for_alt_svc(url_str, &policy)
                        {
                            tracing::info!(
                                original = url_str,
                                rewritten = %rewritten_url,
                                "Alt-Svc: Attempting connection over HTTP/3"
                            );
                            let h3_client = self.get_http3_client();
                            let req = builder_fn(&h3_client, &rewritten_url);
                            match req.send().await {
                                Ok(resp) => {
                                    self.check_and_update_alt_svc(&resp).await;
                                    self.check_and_update_hsts(&resp).await;
                                    return Ok(resp);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "Alt-Svc: HTTP/3 request failed, falling back to standard client"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Fallback / default path: standard client (HTTP/1.1 / HTTP/2)
        let req = builder_fn(&self.client, url_str);
        let resp = req.send().await;
        if let Ok(ref response) = resp {
            self.check_and_update_alt_svc(response).await;
            self.check_and_update_hsts(response).await;
        }
        resp
    }

    pub fn is_retryable(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    pub(crate) async fn upgrade_url(&self, uri: &str) -> String {
        if !uri.starts_with("http://") {
            return uri.to_string();
        }

        if let Some(ref cache) = self.options.hsts_cache {
            if let Ok(url) = url::Url::parse(uri) {
                if let Some(host) = url.host_str() {
                    if cache.should_upgrade(host).await {
                        let mut upgraded = url.clone();
                        let _ = upgraded.set_scheme("https");
                        return upgraded.to_string();
                    }
                }
            }
        }

        uri.to_string()
    }
}

pub(crate) fn build_client_from_options(
    options: &WorkerOptions,
    http3: bool,
) -> reqwest::ClientBuilder {
    let cookie_jar = if let Some(ref provider) = options.credential_provider {
        provider.cookie_jar()
    } else {
        Arc::new(reqwest::cookie::Jar::default())
    };

    let mut builder = reqwest::Client::builder()
        .user_agent(options.user_agent.as_deref().unwrap_or("Aura/0.1.0"))
        .cookie_provider(cookie_jar)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(
            options.connect_timeout.unwrap_or(30),
        ))
        .tcp_keepalive(std::time::Duration::from_secs(
            options.tcp_keepalive_secs.unwrap_or(60),
        ));

    if http3 {
        builder = builder.http3_prior_knowledge();
    }

    if let Some(addr) = options.local_addr {
        builder = builder.local_address(addr);
    }

    if let Some(ref p) = options.proxy {
        if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
            builder = builder.proxy(proxy_obj);
        }
    }

    if let Some(ref resolver) = options.dns_resolver {
        let wrapped = crate::net_util::ReqwestDnsResolver::from_arc(resolver.clone());
        builder = builder.dns_resolver(Arc::new(wrapped));
    }

    builder
}

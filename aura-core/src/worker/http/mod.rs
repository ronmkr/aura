pub(crate) mod crawler;
pub(crate) mod metadata;
pub(crate) mod segment;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests_governor.rs"]
mod tests_governor;

#[cfg(test)]
#[path = "tests_conditional_pool.rs"]
mod tests_conditional_pool;

use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ClientKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl ClientKey {
    pub fn from_uri(uri: &str) -> Option<Self> {
        let url = url::Url::parse(uri).ok()?;
        let scheme = url.scheme().to_string();
        let host = url.host_str()?.to_string();
        let port = url.port_or_known_default()?;
        Some(Self { scheme, host, port })
    }
}

#[derive(Clone, Default)]
pub struct ClientPool {
    clients: Arc<std::sync::Mutex<std::collections::HashMap<ClientKey, Arc<reqwest::Client>>>>,
}

impl ClientPool {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn get_or_create<F>(&self, key: ClientKey, create_fn: F) -> Arc<reqwest::Client>
    where
        F: FnOnce() -> reqwest::Client,
    {
        let mut lock = self.clients.lock().unwrap();
        lock.retain(|_, client| Arc::strong_count(client) > 1);

        if let Some(client) = lock.get(&key) {
            client.clone()
        } else {
            let client = Arc::new(create_fn());
            lock.insert(key, client.clone());
            client
        }
    }

    pub fn len(&self) -> usize {
        let lock = self.clients.lock().unwrap();
        lock.len()
    }

    pub fn is_empty(&self) -> bool {
        let lock = self.clients.lock().unwrap();
        lock.is_empty()
    }
}

/// Options for creating a new HttpWorker.
pub struct HttpWorkerOptions {
    pub uri: String,
    pub local_addr: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub connect_timeout: Option<u64>,
    pub proxy: Option<String>,
    pub referer: Option<String>,
    pub retry_count: u32,
    pub retry_delay_secs: u64,
    pub credential_provider: Option<Arc<crate::config::credentials::CredentialProvider>>,
    pub dns_resolver: Option<Arc<crate::net_util::TokioResolver>>,
    pub hsts_cache: Option<crate::security::HstsCache>,
    pub alt_svc_cache: Option<crate::security::AltSvcCache>,
    pub resource_governor: Option<Arc<crate::orchestrator::resource_governor::ResourceGovernor>>,
    pub tenant_id: Option<crate::TenantId>,
    pub client_pool: Option<ClientPool>,
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    pub(crate) client: Arc<reqwest::Client>,
    pub(crate) http3_client: std::sync::Mutex<Option<reqwest::Client>>,
    pub(crate) options: HttpWorkerOptions,
}

impl HttpWorker {
    pub(crate) fn is_retryable(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    pub(crate) async fn upgrade_url(&self, url_str: &str) -> String {
        if let Some(ref cache) = self.options.hsts_cache {
            if let Ok(mut url) = url::Url::parse(url_str) {
                if url.scheme() == "http" {
                    if let Some(host) = url.host_str() {
                        if cache.should_upgrade(host).await {
                            let _ = url.set_scheme("https");
                            tracing::info!(
                                from = url_str,
                                to = url.as_str(),
                                "HSTS: Automatically upgraded HTTP request to HTTPS"
                            );
                            return url.to_string();
                        }
                    }
                }
            }
        }
        url_str.to_string()
    }

    pub(crate) async fn check_and_update_hsts(&self, resp: &reqwest::Response) {
        if let Some(ref cache) = self.options.hsts_cache {
            let url = resp.url();
            if url.scheme() == "https" {
                if let Some(hsts_val) = resp
                    .headers()
                    .get(reqwest::header::STRICT_TRANSPORT_SECURITY)
                {
                    if let Ok(hsts_str) = hsts_val.to_str() {
                        if let Some((max_age, include_subdomains)) =
                            crate::security::parse_hsts_header(hsts_str)
                        {
                            if let Some(host) = url.host_str() {
                                cache
                                    .insert_policy(host.to_string(), max_age, include_subdomains)
                                    .await;
                            }
                        }
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
        let cookie_jar = if let Some(ref provider) = self.options.credential_provider {
            provider.cookie_jar()
        } else {
            Arc::new(reqwest::cookie::Jar::default())
        };

        let mut builder = reqwest::Client::builder()
            .user_agent(self.options.user_agent.as_deref().unwrap_or("Aura/0.1.0"))
            .cookie_provider(cookie_jar)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(
                self.options.connect_timeout.unwrap_or(30),
            ))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .http3_prior_knowledge();

        if let Some(addr) = self.options.local_addr {
            builder = builder.local_address(addr);
        }

        if let Some(ref p) = self.options.proxy {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy_obj);
            }
        }

        if let Some(ref resolver) = self.options.dns_resolver {
            let wrapped = crate::net_util::ReqwestDnsResolver::from_arc(resolver.clone());
            builder = builder.dns_resolver(Arc::new(wrapped));
        }

        builder.build().expect("Failed to build HTTP/3 client")
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

    pub fn new(options: HttpWorkerOptions) -> Self {
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
            .tcp_keepalive(std::time::Duration::from_secs(60));

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

        let client = if let (Some(ref pool), Some(key)) =
            (&options.client_pool, ClientKey::from_uri(&options.uri))
        {
            pool.get_or_create(key, || {
                builder.build().expect("Failed to build HTTP client")
            })
        } else {
            Arc::new(builder.build().expect("Failed to build HTTP client"))
        };

        Self {
            client,
            http3_client: std::sync::Mutex::new(None),
            options,
        }
    }
}

pub(crate) mod crawler;
pub(crate) mod metadata;
pub(crate) mod segment;

#[cfg(test)]
mod tests;

use std::sync::Arc;

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
}

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    pub(crate) client: reqwest::Client,
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

        let client = builder.build().expect("Failed to build HTTP client");

        Self { client, options }
    }
}

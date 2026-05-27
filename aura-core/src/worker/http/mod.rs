pub(crate) mod crawler;
pub(crate) mod metadata;
pub(crate) mod segment;

#[cfg(test)]
mod tests;

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    pub(crate) client: reqwest::Client,
    pub(crate) uri: String,
    pub(crate) referer: Option<String>,
    pub(crate) retry_count: u32,
    pub(crate) retry_delay_secs: u64,
    pub(crate) credential_provider:
        Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
    pub(crate) hsts_cache: Option<crate::security::HstsCache>,
}

impl HttpWorker {
    pub(crate) fn is_retryable(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    pub(crate) async fn upgrade_url(&self, url_str: &str) -> String {
        if let Some(ref cache) = self.hsts_cache {
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
        if let Some(ref cache) = self.hsts_cache {
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

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uri: String,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        connect_timeout: Option<u64>,
        proxy: Option<String>,
        referer: Option<String>,
        retry_count: u32,
        retry_delay_secs: u64,
        credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
        dns_resolver: Option<std::sync::Arc<crate::net_util::TokioResolver>>,
        hsts_cache: Option<crate::security::HstsCache>,
    ) -> Self {
        let cookie_jar = if let Some(ref provider) = credential_provider {
            provider.cookie_jar()
        } else {
            std::sync::Arc::new(reqwest::cookie::Jar::default())
        };

        let mut builder = reqwest::Client::builder()
            .user_agent(user_agent.unwrap_or_else(|| "Aura/0.1.0".to_string()))
            .cookie_provider(cookie_jar)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(
                connect_timeout.unwrap_or(30),
            ))
            .tcp_keepalive(std::time::Duration::from_secs(60));

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        if let Some(p) = proxy {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy_obj);
            }
        }

        if let Some(resolver) = dns_resolver {
            let wrapped = crate::net_util::ReqwestDnsResolver::from_arc(resolver);
            builder = builder.dns_resolver(std::sync::Arc::new(wrapped));
        }

        let client = builder.build().expect("Failed to build HTTP client");

        Self {
            client,
            uri,
            referer,
            retry_count,
            retry_delay_secs,
            credential_provider,
            hsts_cache,
        }
    }
}

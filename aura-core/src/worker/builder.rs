use crate::worker::ftp::FtpWorker;
use crate::worker::http::HttpWorker;
use std::net::IpAddr;

/// Builder for protocol workers to ensure idiomatic and robust construction.
pub struct WorkerBuilder {
    uri: String,
    local_addr: Option<IpAddr>,
    user_agent: Option<String>,
    connect_timeout: Option<u64>,
    proxy: Option<String>,
    referer: Option<String>,
    retry_count: u32,
    retry_delay_secs: u64,
    max_redirects: usize,
    happy_eyeballs_stagger_ms: u64,
    http_buffer_capacity: usize,
    http_concurrent_requests: usize,
    credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
    dns_resolver: Option<std::sync::Arc<crate::net_util::TokioResolver>>,
    hsts_cache: Option<crate::security::HstsCache>,
    alt_svc_cache: Option<crate::security::AltSvcCache>,
    resource_governor:
        Option<std::sync::Arc<crate::orchestrator::resource_governor::ResourceGovernor>>,
    tenant_id: Option<crate::TenantId>,
    client_pool: Option<crate::worker::http::ClientPool>,
    if_none_match: Option<String>,
    if_modified_since: Option<String>,
}

impl WorkerBuilder {
    pub fn new(uri: String) -> Self {
        Self {
            uri,
            local_addr: None,
            user_agent: None,
            connect_timeout: None,
            proxy: None,
            referer: None,
            retry_count: 5,
            retry_delay_secs: 2,
            max_redirects: 20,
            happy_eyeballs_stagger_ms: 250,
            http_buffer_capacity: 16384,
            http_concurrent_requests: 32,
            credential_provider: None,
            dns_resolver: None,
            hsts_cache: None,
            alt_svc_cache: None,
            resource_governor: None,
            tenant_id: None,
            client_pool: None,
            if_none_match: None,
            if_modified_since: None,
        }
    }

    pub fn dns_resolver(
        mut self,
        resolver: std::sync::Arc<crate::net_util::TokioResolver>,
    ) -> Self {
        self.dns_resolver = Some(resolver);
        self
    }

    pub fn hsts_cache(mut self, cache: crate::security::HstsCache) -> Self {
        self.hsts_cache = Some(cache);
        self
    }

    pub fn alt_svc_cache(mut self, cache: crate::security::AltSvcCache) -> Self {
        self.alt_svc_cache = Some(cache);
        self
    }

    pub fn credential_provider(
        mut self,
        provider: std::sync::Arc<crate::config::credentials::CredentialProvider>,
    ) -> Self {
        self.credential_provider = Some(provider);
        self
    }

    pub fn local_addr(mut self, addr: Option<IpAddr>) -> Self {
        self.local_addr = addr;
        self
    }

    pub fn user_agent(mut self, ua: Option<String>) -> Self {
        self.user_agent = ua;
        self
    }

    pub fn connect_timeout(mut self, timeout: Option<u64>) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }

    pub fn referer(mut self, referer: Option<String>) -> Self {
        self.referer = referer;
        self
    }

    pub fn retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    pub fn retry_delay_secs(mut self, secs: u64) -> Self {
        self.retry_delay_secs = secs;
        self
    }

    pub fn max_redirects(mut self, count: usize) -> Self {
        self.max_redirects = count;
        self
    }

    pub fn happy_eyeballs_stagger_ms(mut self, ms: u64) -> Self {
        self.happy_eyeballs_stagger_ms = ms;
        self
    }

    pub fn http_buffer_capacity(mut self, cap: usize) -> Self {
        self.http_buffer_capacity = cap;
        self
    }

    pub fn http_concurrent_requests(mut self, count: usize) -> Self {
        self.http_concurrent_requests = count;
        self
    }

    pub fn resource_governor(
        mut self,
        governor: std::sync::Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    ) -> Self {
        self.resource_governor = Some(governor);
        self
    }

    pub fn tenant_id(mut self, tenant_id: Option<crate::TenantId>) -> Self {
        self.tenant_id = tenant_id;
        self
    }

    pub fn client_pool(mut self, pool: crate::worker::http::ClientPool) -> Self {
        self.client_pool = Some(pool);
        self
    }

    pub fn if_none_match(mut self, etag: Option<String>) -> Self {
        self.if_none_match = etag;
        self
    }

    pub fn if_modified_since(mut self, last_modified: Option<String>) -> Self {
        self.if_modified_since = last_modified;
        self
    }

    pub fn build_http(self) -> HttpWorker {
        HttpWorker::new(crate::worker::http::HttpWorkerOptions {
            uri: self.uri,
            local_addr: self.local_addr,
            user_agent: self.user_agent,
            connect_timeout: self.connect_timeout,
            proxy: self.proxy,
            referer: self.referer,
            retry_count: self.retry_count,
            http_retry_delay_secs: self.retry_delay_secs,
            max_redirects: self.max_redirects,
            happy_eyeballs_stagger_ms: self.happy_eyeballs_stagger_ms,
            http_buffer_capacity: self.http_buffer_capacity,
            http_concurrent_requests: self.http_concurrent_requests,
            credential_provider: self.credential_provider,
            dns_resolver: self.dns_resolver,
            hsts_cache: self.hsts_cache,
            alt_svc_cache: self.alt_svc_cache,
            resource_governor: self.resource_governor,
            tenant_id: self.tenant_id,
            client_pool: self.client_pool,
            if_none_match: self.if_none_match,
            if_modified_since: self.if_modified_since,
        })
    }

    pub fn build_ftp(self) -> FtpWorker {
        FtpWorker::new(crate::worker::ftp::FtpWorkerOptions {
            uri: self.uri,
            local_addr: self.local_addr,
            retry_count: self.retry_count,
            http_retry_delay_secs: self.retry_delay_secs,
            happy_eyeballs_stagger_ms: self.happy_eyeballs_stagger_ms,
            http_buffer_capacity: self.http_buffer_capacity,
            credential_provider: self.credential_provider,
            resource_governor: self.resource_governor,
            tenant_id: self.tenant_id,
        })
    }
}

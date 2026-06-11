use std::net::IpAddr;
use std::sync::Arc;

/// Common options for all protocol workers.
#[derive(Clone)]
pub struct WorkerOptions {
    pub uri: String,
    pub local_addr: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub connect_timeout: Option<u64>,
    pub tcp_keepalive_secs: Option<u64>,
    pub proxy: Option<String>,
    pub referer: Option<String>,
    pub retry_count: u32,
    pub retry_delay_secs: u64,
    pub max_redirects: usize,
    pub happy_eyeballs_stagger_ms: u64,
    pub http_buffer_capacity: usize,
    pub http_concurrent_requests: usize,
    pub credential_provider: Option<Arc<crate::config::credentials::CredentialProvider>>,
    pub dns_resolver: Option<Arc<crate::net_util::TokioResolver>>,
    pub hsts_cache: Option<crate::security::HstsCache>,
    pub alt_svc_cache: Option<crate::security::AltSvcCache>,
    pub resource_governor: Option<Arc<crate::orchestrator::resource_governor::ResourceGovernor>>,
    pub tenant_id: Option<crate::TenantId>,
    pub client_pool: Option<crate::worker::http::ClientPool>,
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self {
            uri: String::new(),
            local_addr: None,
            user_agent: None,
            connect_timeout: None,
            tcp_keepalive_secs: None,
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
}

/// Builder for protocol workers to ensure idiomatic and robust construction.
pub struct WorkerBuilder {
    pub options: WorkerOptions,
}

impl WorkerBuilder {
    pub fn new(uri: String) -> Self {
        Self {
            options: WorkerOptions {
                uri,
                ..Default::default()
            },
        }
    }

    pub fn dns_resolver(mut self, resolver: Arc<crate::net_util::TokioResolver>) -> Self {
        self.options.dns_resolver = Some(resolver);
        self
    }

    pub fn hsts_cache(mut self, cache: crate::security::HstsCache) -> Self {
        self.options.hsts_cache = Some(cache);
        self
    }

    pub fn alt_svc_cache(mut self, cache: crate::security::AltSvcCache) -> Self {
        self.options.alt_svc_cache = Some(cache);
        self
    }

    pub fn credential_provider(
        mut self,
        provider: Arc<crate::config::credentials::CredentialProvider>,
    ) -> Self {
        self.options.credential_provider = Some(provider);
        self
    }

    pub fn local_addr(mut self, addr: Option<IpAddr>) -> Self {
        self.options.local_addr = addr;
        self
    }

    pub fn user_agent(mut self, ua: Option<String>) -> Self {
        self.options.user_agent = ua;
        self
    }

    pub fn connect_timeout(mut self, timeout: Option<u64>) -> Self {
        self.options.connect_timeout = timeout;
        self
    }

    pub fn tcp_keepalive_secs(mut self, secs: Option<u64>) -> Self {
        self.options.tcp_keepalive_secs = secs;
        self
    }

    pub fn proxy(mut self, proxy: Option<String>) -> Self {
        self.options.proxy = proxy;
        self
    }

    pub fn referer(mut self, referer: Option<String>) -> Self {
        self.options.referer = referer;
        self
    }

    pub fn retry_count(mut self, count: u32) -> Self {
        self.options.retry_count = count;
        self
    }

    pub fn retry_delay_secs(mut self, secs: u64) -> Self {
        self.options.retry_delay_secs = secs;
        self
    }

    pub fn max_redirects(mut self, count: usize) -> Self {
        self.options.max_redirects = count;
        self
    }

    pub fn happy_eyeballs_stagger_ms(mut self, ms: u64) -> Self {
        self.options.happy_eyeballs_stagger_ms = ms;
        self
    }

    pub fn http_buffer_capacity(mut self, cap: usize) -> Self {
        self.options.http_buffer_capacity = cap;
        self
    }

    pub fn http_concurrent_requests(mut self, count: usize) -> Self {
        self.options.http_concurrent_requests = count;
        self
    }

    pub fn resource_governor(
        mut self,
        governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    ) -> Self {
        self.options.resource_governor = Some(governor);
        self
    }

    pub fn tenant_id(mut self, tenant_id: Option<crate::TenantId>) -> Self {
        self.options.tenant_id = tenant_id;
        self
    }

    pub fn client_pool(mut self, pool: crate::worker::http::ClientPool) -> Self {
        self.options.client_pool = Some(pool);
        self
    }

    pub fn if_none_match(mut self, etag: Option<String>) -> Self {
        self.options.if_none_match = etag;
        self
    }

    pub fn if_modified_since(mut self, last_modified: Option<String>) -> Self {
        self.options.if_modified_since = last_modified;
        self
    }

    pub fn build_http(self) -> crate::worker::http::HttpWorker {
        crate::worker::http::HttpWorker::new(self.options)
    }

    pub fn build_ftp(self) -> crate::worker::ftp::FtpWorker {
        crate::worker::ftp::FtpWorker::new(self.options)
    }

    pub fn build_s3(self) -> crate::worker::s3::S3Worker {
        crate::worker::s3::S3Worker::new(self.options)
    }

    pub fn build_gdrive(self) -> crate::worker::gdrive::GDriveWorker {
        crate::worker::gdrive::GDriveWorker::new(self.options)
    }

    #[cfg(feature = "nntp")]
    pub fn build_nntp(self) -> crate::worker::nntp::NntpWorker {
        crate::worker::nntp::NntpWorker::new(self.options)
    }
}

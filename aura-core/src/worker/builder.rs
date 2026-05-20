use crate::buffer_pool::BufferPool;
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
    pool: Option<BufferPool>,
    retry_count: u32,
    retry_delay_secs: u64,
    credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
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
            pool: None,
            retry_count: 5,
            retry_delay_secs: 2,
            credential_provider: None,
        }
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

    pub fn with_pool(mut self, pool: BufferPool) -> Self {
        self.pool = Some(pool);
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

    pub fn build_http(self) -> HttpWorker {
        HttpWorker::new(
            self.uri,
            self.local_addr,
            self.user_agent,
            self.connect_timeout,
            self.proxy,
            self.referer,
            self.pool,
            self.retry_count,
            self.retry_delay_secs,
            self.credential_provider,
        )
    }

    pub fn build_ftp(self) -> FtpWorker {
        FtpWorker::new(
            self.uri,
            self.local_addr,
            self.pool,
            self.credential_provider,
        )
    }
}

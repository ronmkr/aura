use std::net::IpAddr;
use crate::worker::http::HttpWorker;
use crate::worker::ftp::FtpWorker;

/// Builder for protocol workers to ensure idiomatic and robust construction.
pub struct WorkerBuilder {
    uri: String,
    local_addr: Option<IpAddr>,
    user_agent: Option<String>,
    connect_timeout: Option<u64>,
    proxy: Option<String>,
    referer: Option<String>,
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
        }
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

    pub fn build_http(self) -> HttpWorker {
        HttpWorker::new(
            self.uri,
            self.local_addr,
            self.user_agent,
            self.connect_timeout,
            self.proxy,
            self.referer,
        )
    }

    pub fn build_ftp(self) -> FtpWorker {
        FtpWorker::new(self.uri, self.local_addr)
    }
}

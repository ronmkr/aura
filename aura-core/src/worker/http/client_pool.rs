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

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct File {
    pub length: u64,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2File {
    pub path: Vec<String>,
    pub length: u64,
    pub pieces_root: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: u64,
    #[serde(with = "serde_bytes", skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<u8>>,
    pub length: Option<u64>,
    pub files: Option<Vec<File>>,
    #[serde(rename = "meta version", skip_serializing_if = "Option::is_none")]
    pub meta_version: Option<u64>,
    #[serde(rename = "file tree", skip_serializing_if = "Option::is_none")]
    pub file_tree: Option<serde_bencode::value::Value>,
}

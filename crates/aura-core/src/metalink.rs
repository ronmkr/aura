//! metalink: Support for Metalink (V3/V4) XML parsing and source extraction.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::io::BufReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metalink {
    pub files: Vec<MetalinkFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetalinkFile {
    pub name: String,
    pub size: Option<u64>,
    pub hash: Option<String>,
    pub resources: Vec<MetalinkResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetalinkResource {
    pub uri: String,
    pub priority: u32,
    pub protocol: String,
}

impl Metalink {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut files = Vec::new();
        let reader = BufReader::new(data);

        let mut reader = quick_xml::Reader::from_reader(reader);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut current_file: Option<MetalinkFile> = None;
        let mut current_tag = String::new();
        let mut current_protocol = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if current_tag == "file" {
                        let mut name = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        current_file = Some(MetalinkFile {
                            name,
                            size: None,
                            hash: None,
                            resources: Vec::new(),
                        });
                    } else if current_tag == "url" {
                        current_protocol = "http".to_string(); // Default
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "protocol" {
                                current_protocol = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) => {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    if text.is_empty() {
                        continue;
                    }

                    if let Some(ref mut file) = current_file {
                        match current_tag.as_str() {
                            "size" => file.size = text.parse().ok(),
                            "hash" => file.hash = Some(text),
                            "url" => {
                                let proto = if text.trim().starts_with("ftp://") {
                                    "ftp".to_string()
                                } else {
                                    current_protocol.trim().to_string()
                                };
                                eprintln!("DEBUG: parsed url={} proto={}", text, proto);
                                file.resources.push(MetalinkResource {
                                    uri: text.trim().to_string(),
                                    priority: 0,
                                    protocol: proto,
                                });
                                // Reset tag to avoid double-adding if there's trailing whitespace
                                current_tag = String::new();
                            }
                            _ => {}
                        }
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag == "file" {
                        if let Some(f) = current_file.take() {
                            files.push(f);
                        }
                    }
                    current_tag = String::new();
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(Error::Protocol(format!("Metalink XML error: {}", e))),
                _ => {}
            }
            buf.clear();
        }

        Ok(Metalink { files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metalink() {
        let xml = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <metalink version="3.0" xmlns="http://www.metalinker.org/">
          <files>
            <file name="example.zip">
              <size>12345</size>
              <resources>
                <url protocol="http">http://mirror1.com/example.zip</url>
                <url protocol="ftp">ftp://mirror2.com/example.zip</url>
              </resources>
            </file>
          </files>
        </metalink>
        "#;
        let metalink = Metalink::parse(xml.as_bytes()).expect("Failed to parse Metalink");
        assert_eq!(metalink.files.len(), 1);
        assert_eq!(metalink.files[0].name, "example.zip");
        assert_eq!(metalink.files[0].size, Some(12345));
        assert_eq!(metalink.files[0].resources.len(), 2);
        assert_eq!(metalink.files[0].resources[0].protocol, "http");
        assert_eq!(metalink.files[0].resources[1].protocol, "ftp");
    }
}

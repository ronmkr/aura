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
        let mut current_priority = 0u32;

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
                        current_priority = 0; // Default
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "protocol" {
                                current_protocol = String::from_utf8_lossy(&attr.value).to_string();
                            } else if key == "priority" {
                                if let Ok(parsed) =
                                    String::from_utf8_lossy(&attr.value).parse::<u32>()
                                {
                                    current_priority = parsed;
                                }
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
                                file.resources.push(MetalinkResource {
                                    uri: text.trim().to_string(),
                                    priority: current_priority,
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
                        if let Some(mut f) = current_file.take() {
                            // Sort resources by priority ascending
                            f.resources.sort_by_key(|r| r.priority);
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

    #[test]
    fn test_parse_metalink_priorities() {
        let xml = r#"
    <?xml version="1.0" encoding="utf-8"?>
    <metalink version="3.0" xmlns="http://www.metalinker.org/">
      <files>
        <file name="priority.zip">
          <size>50000</size>
          <resources>
            <url protocol="http" priority="10">http://low-priority.com/priority.zip</url>
            <url protocol="http" priority="2">http://high-priority.com/priority.zip</url>
            <url protocol="http" priority="5">http://med-priority.com/priority.zip</url>
          </resources>
        </file>
      </files>
    </metalink>
    "#;
        let metalink = Metalink::parse(xml.as_bytes()).expect("Failed to parse Metalink");
        assert_eq!(metalink.files.len(), 1);
        let resources = &metalink.files[0].resources;
        assert_eq!(resources.len(), 3);
        // Verify they are sorted by priority ascending
        assert_eq!(resources[0].priority, 2);
        assert_eq!(resources[0].uri, "http://high-priority.com/priority.zip");
        assert_eq!(resources[1].priority, 5);
        assert_eq!(resources[1].uri, "http://med-priority.com/priority.zip");
        assert_eq!(resources[2].priority, 10);
        assert_eq!(resources[2].uri, "http://low-priority.com/priority.zip");
    }
}

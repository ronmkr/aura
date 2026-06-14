use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::BufRead;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeedItem {
    pub title: String,
    pub link: String,
    pub guid: String,
}

fn compute_md5_hex(s: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn parse_feed<R: BufRead>(data: R) -> Result<Vec<FeedItem>, String> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut items = Vec::new();

    #[derive(Debug, PartialEq, Eq)]
    enum State {
        Root,
        Channel,
        Item,
        Feed,
        Entry,
    }

    let mut state = State::Root;
    let mut current_title = String::new();
    let mut current_link = String::new();
    let mut current_guid = String::new();
    let mut current_pubdate = String::new();
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match state {
                    State::Root => {
                        if current_tag == "channel" {
                            state = State::Channel;
                        } else if current_tag == "feed" {
                            state = State::Feed;
                        }
                    }
                    State::Channel => {
                        if current_tag == "item" {
                            state = State::Item;
                            current_title.clear();
                            current_link.clear();
                            current_guid.clear();
                            current_pubdate.clear();
                        }
                    }
                    State::Feed => {
                        if current_tag == "entry" {
                            state = State::Entry;
                            current_title.clear();
                            current_link.clear();
                            current_guid.clear();
                            current_pubdate.clear();
                        }
                    }
                    State::Item | State::Entry => {
                        if current_tag == "link" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"href" {
                                    current_link = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        } else if current_tag == "enclosure" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"url" {
                                    current_link = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if text.is_empty() {
                    continue;
                }
                match state {
                    State::Item => match current_tag.as_str() {
                        "title" => current_title = text,
                        "link" => {
                            if current_link.is_empty() {
                                current_link = text;
                            }
                        }
                        "guid" => current_guid = text,
                        "pubDate" => current_pubdate = text,
                        _ => {}
                    },
                    State::Entry => match current_tag.as_str() {
                        "title" => current_title = text,
                        "id" => current_guid = text,
                        "published" | "updated" => current_pubdate = text,
                        _ => {}
                    },
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match state {
                    State::Item if tag == "item" => {
                        state = State::Channel;
                        if current_guid.is_empty() {
                            let hash_input = if !current_pubdate.is_empty() {
                                format!("{}{}", current_title, current_pubdate)
                            } else {
                                format!("{}{}", current_title, current_link)
                            };
                            current_guid = compute_md5_hex(&hash_input);
                        }
                        if !current_link.is_empty() {
                            items.push(FeedItem {
                                title: current_title.clone(),
                                link: current_link.clone(),
                                guid: current_guid.clone(),
                            });
                        }
                    }
                    State::Entry if tag == "entry" => {
                        state = State::Feed;
                        if current_guid.is_empty() {
                            let hash_input = if !current_pubdate.is_empty() {
                                format!("{}{}", current_title, current_pubdate)
                            } else {
                                format!("{}{}", current_title, current_link)
                            };
                            current_guid = compute_md5_hex(&hash_input);
                        }
                        if !current_link.is_empty() {
                            items.push(FeedItem {
                                title: current_title.clone(),
                                link: current_link.clone(),
                                guid: current_guid.clone(),
                            });
                        }
                    }
                    State::Channel if tag == "channel" => {
                        state = State::Root;
                    }
                    State::Feed if tag == "feed" => {
                        state = State::Root;
                    }
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match state {
                    State::Item | State::Entry => {
                        if tag_name == "link" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"href" {
                                    current_link = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        } else if tag_name == "enclosure" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"url" {
                                    current_link = String::from_utf8_lossy(&attr.value).to_string();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Feed XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}
#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;

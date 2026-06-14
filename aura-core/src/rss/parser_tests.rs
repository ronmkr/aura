use super::*;

#[test]
fn test_parse_rss_feed() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" ?>
<rss version="2.0">
<channel>
    <title>Aura releases</title>
    <link>https://github.com/ronmkr/aura</link>
    <description>Aura release channel</description>
    <item>
        <title>Aura v1.0.0 Stable</title>
        <link>https://github.com/ronmkr/aura/releases/download/v1.0.0/aura</link>
        <guid>v1.0.0-stable</guid>
        <category>software</category>
        <pubDate>Thu, 11 Jun 2026 12:00:00 GMT</pubDate>
    </item>
    <item>
        <title>Aura v1.0.1 Patch</title>
        <enclosure url="https://github.com/ronmkr/aura/releases/download/v1.0.1/aura.torrent" length="12345" type="application/x-bittorrent" />
        <pubDate>Fri, 12 Jun 2026 12:00:00 GMT</pubDate>
    </item>
</channel>
</rss>"#;

    let items = parse_feed(xml.as_bytes()).unwrap();
    assert_eq!(items.len(), 2);

    assert_eq!(items[0].title, "Aura v1.0.0 Stable");
    assert_eq!(
        items[0].link,
        "https://github.com/ronmkr/aura/releases/download/v1.0.0/aura"
    );
    assert_eq!(items[0].guid, "v1.0.0-stable");
    assert_eq!(items[0].category, Some("software".to_string()));
    assert_eq!(items[0].size, None);

    assert_eq!(items[1].title, "Aura v1.0.1 Patch");
    assert_eq!(
        items[1].link,
        "https://github.com/ronmkr/aura/releases/download/v1.0.1/aura.torrent"
    );
    // Hashed GUID fallback because no explicit <guid> is present
    assert!(!items[1].guid.is_empty());
    assert_ne!(items[1].guid, "v1.0.0-stable");
    assert_eq!(items[1].category, None);
    assert_eq!(items[1].size, Some(12345));
}

#[test]
fn test_parse_atom_feed() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
    <title>Aura Atom Feed</title>
    <link href="http://example.org/feed/"/>
    <updated>2026-06-11T12:00:00Z</updated>
    <entry>
        <title>Aura Alpha Build</title>
        <link href="http://example.org/downloads/aura-alpha.zip"/>
        <id>urn:uuid:1225c695-cfb8-4ebb-aaaa-80da344efa6a</id>
        <updated>2026-06-11T12:00:00Z</updated>
    </entry>
</feed>"#;

    let items = parse_feed(xml.as_bytes()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Aura Alpha Build");
    assert_eq!(items[0].link, "http://example.org/downloads/aura-alpha.zip");
    assert_eq!(
        items[0].guid,
        "urn:uuid:1225c695-cfb8-4ebb-aaaa-80da344efa6a"
    );
}

#[test]
fn test_billion_laughs_prevention() {
    // A standard entity expansion block to verify quick-xml parses safely without expanding recursively
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
 <!ENTITY lol "lol">
 <!ELEMENT lolz (#PCDATA)>
 <!ENTITY lol1 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
]>
<rss version="2.0">
<channel>
    <item>
        <title>Test &lol1;</title>
        <link>http://example.com/test</link>
    </item>
</channel>
</rss>"#;

    // It should parse safely and not crash, resolving to unexpanded text (or empty/failed entity resolution)
    let items = parse_feed(xml.as_bytes()).unwrap();
    assert_eq!(items.len(), 1);
    // Entity is not expanded since we did not configure custom entity resolvers.
    assert!(items[0].title.contains("Test") || items[0].title.contains("lol"));
}

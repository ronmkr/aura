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

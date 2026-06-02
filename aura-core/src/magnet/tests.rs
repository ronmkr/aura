use super::*;

#[test]
fn test_parse_magnet() {
    let uri = "magnet:?xt=urn:btih:d2474436908143d52cdeee8d4c96510d3301cdc4&dn=Ubuntu&tr=http://tracker.com/announce";
    let magnet = Magnet::parse(uri).unwrap();

    assert_eq!(magnet.name, Some("Ubuntu".to_string()));
    assert_eq!(magnet.trackers.len(), 1);
    assert_eq!(magnet.trackers[0], "http://tracker.com/announce");
    assert_eq!(magnet.info_hash.to_vec()[0], 0xd2);
}

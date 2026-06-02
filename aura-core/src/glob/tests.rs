use super::*;

#[test]
fn test_expand_set() {
    let urls = expand_url("http://{a,b}.com").unwrap();
    assert_eq!(urls, vec!["http://a.com", "http://b.com"]);
}

#[test]
fn test_expand_numeric_range() {
    let urls = expand_url("img_[1-3].jpg").unwrap();
    assert_eq!(urls, vec!["img_1.jpg", "img_2.jpg", "img_3.jpg"]);
}

#[test]
fn test_expand_numeric_padding() {
    let urls = expand_url("img_[01-03].jpg").unwrap();
    assert_eq!(urls, vec!["img_01.jpg", "img_02.jpg", "img_03.jpg"]);
}

#[test]
fn test_expand_numeric_step() {
    let urls = expand_url("img_[1-5:2].jpg").unwrap();
    assert_eq!(urls, vec!["img_1.jpg", "img_3.jpg", "img_5.jpg"]);
}

#[test]
fn test_expand_alpha_range() {
    let urls = expand_url("part_[a-c].bin").unwrap();
    assert_eq!(urls, vec!["part_a.bin", "part_b.bin", "part_c.bin"]);
}

#[test]
fn test_expand_multiple() {
    let urls = expand_url("http://{s1,s2}.com/file_[1-2].zip").unwrap();
    assert_eq!(
        urls,
        vec![
            "http://s1.com/file_1.zip",
            "http://s1.com/file_2.zip",
            "http://s2.com/file_1.zip",
            "http://s2.com/file_2.zip",
        ]
    );
}

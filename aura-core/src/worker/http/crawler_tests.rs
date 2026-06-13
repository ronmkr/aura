use super::*;

#[test]
fn test_crawler_max_depth() {
    let mut crawler = RecursiveCrawler::new("http://example.com", 1, true).unwrap();
    // At depth 1, enqueue_links should just return and do nothing
    crawler.enqueue_links("http://example.com", r#"<a href="/link1">link</a>"#, 1);
    let next = crawler.next_url(); // should only have the start URL
    assert_eq!(next.unwrap().0, "http://example.com/");
    assert!(crawler.next_url().is_none());
}

#[test]
fn test_crawler_cycles() {
    let mut crawler = RecursiveCrawler::new("http://example.com", 2, true).unwrap();

    // Pop start url
    let (url, depth) = crawler.next_url().unwrap();
    assert_eq!(url, "http://example.com/");

    // Feed a cycle back to root and a new link
    crawler.enqueue_links(
        &url,
        r#"<a href="/">root</a> <a href="/page">page</a>"#,
        depth,
    );

    let (url2, depth2) = crawler.next_url().unwrap();
    assert_eq!(url2, "http://example.com/page"); // Root is skipped because it's already visited

    crawler.enqueue_links(&url2, r#"<a href="/page">self</a>"#, depth2);
    assert!(crawler.next_url().is_none()); // Self is skipped
}

#[test]
fn test_crawler_stay_on_host() {
    let mut crawler = RecursiveCrawler::new("http://example.com", 2, true).unwrap();

    let (url, depth) = crawler.next_url().unwrap();
    crawler.enqueue_links(
        &url,
        r#"<a href="http://external.com/file.zip">Ext</a> <a href="/local">Loc</a>"#,
        depth,
    );

    let (url2, _) = crawler.next_url().unwrap();
    assert_eq!(url2, "http://example.com/local");
    assert!(crawler.next_url().is_none()); // External was skipped
}

#[test]
fn test_crawler_glob_expansion() {
    let mut crawler = RecursiveCrawler::new("http://example.com/page-[1-3].html", 2, true).unwrap();
    let url1 = crawler.next_url().unwrap().0;
    let url2 = crawler.next_url().unwrap().0;
    let url3 = crawler.next_url().unwrap().0;
    let mut urls = [url1, url2, url3];
    urls.sort();
    assert_eq!(urls[0], "http://example.com/page-1.html");
    assert_eq!(urls[1], "http://example.com/page-2.html");
    assert_eq!(urls[2], "http://example.com/page-3.html");
    assert!(crawler.next_url().is_none());
}

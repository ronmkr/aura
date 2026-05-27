use crate::{Error, Result};
use std::collections::{HashSet, VecDeque};
use url::Url;

pub struct RecursiveCrawler {
    visited: HashSet<String>,
    queue: VecDeque<(String, usize)>,
    max_depth: usize,
    stay_on_host: bool,
    base_host: Option<String>,
}

impl RecursiveCrawler {
    pub fn new(start_url: &str, max_depth: usize, stay_on_host: bool) -> Result<Self> {
        let parsed = Url::parse(start_url).map_err(|e| Error::Protocol(e.to_string()))?;
        let base_host = parsed.host_str().map(|s| s.to_string());

        let normalized_url = parsed.to_string();

        let mut queue = VecDeque::new();
        queue.push_back((normalized_url, 0));

        Ok(Self {
            visited: HashSet::new(),
            queue,
            max_depth,
            stay_on_host,
            base_host,
        })
    }

    pub fn next_url(&mut self) -> Option<(String, usize)> {
        while let Some((url, depth)) = self.queue.pop_front() {
            if self.visited.insert(url.clone()) {
                return Some((url, depth));
            }
        }
        None
    }

    pub fn enqueue_links(&mut self, base: &str, html: &str, current_depth: usize) {
        if current_depth >= self.max_depth {
            return;
        }

        let base_url = match Url::parse(base) {
            Ok(u) => u,
            Err(_) => return,
        };

        use lol_html::{element, HtmlRewriter, Settings};

        // Since we are borrowing `links` mutably inside the closure, we wrap it in a RefCell/Rc,
        // but lol-html handlers take `&mut`, wait no, `element!` takes a closure that can capture context.
        // Actually, lol-html closures can be `FnMut`. Let's just use `RefCell`.
        use std::cell::RefCell;
        use std::rc::Rc;

        let links = Rc::new(RefCell::new(Vec::new()));
        let links_clone = Rc::clone(&links);

        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![element!("a[href]", move |el| {
                    if let Some(href) = el.get_attribute("href") {
                        links_clone.borrow_mut().push(href);
                    }
                    Ok(())
                })],
                ..Settings::default()
            },
            |_c: &[u8]| {},
        );

        let _ = rewriter.write(html.as_bytes());
        let _ = rewriter.end();

        let extracted_links = links.take();

        for href in extracted_links {
            if let Ok(resolved) = base_url.join(&href) {
                if self.stay_on_host {
                    if let (Some(h1), Some(h2)) = (resolved.host_str(), self.base_host.as_deref()) {
                        if h1 != h2 {
                            continue; // Skip external links
                        }
                    }
                }

                let url_str = resolved.to_string();
                if !self.visited.contains(&url_str) {
                    self.queue.push_back((url_str, current_depth + 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}

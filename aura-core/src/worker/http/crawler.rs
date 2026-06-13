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
        let expanded_urls = crate::glob::expand_url(start_url)?;

        let mut queue = VecDeque::new();
        let mut base_host = None;

        for url_str in expanded_urls {
            let parsed = Url::parse(&url_str).map_err(|e| Error::Protocol(e.to_string()))?;
            if base_host.is_none() {
                base_host = parsed.host_str().map(|s| s.to_string());
            }
            queue.push_back((parsed.to_string(), 0));
        }

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
            Settings::default().append_element_content_handler(element!("a[href]", move |el| {
                if let Some(href) = el.get_attribute("href") {
                    links_clone.borrow_mut().push(href);
                }
                Ok(())
            })),
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
#[path = "crawler_tests.rs"]
mod tests;

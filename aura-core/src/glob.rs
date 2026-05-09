//! glob: URL globbing and expansion.

use crate::{Error, Result};
use regex::Regex;

/// Expands a single globbed URL into a list of URLs.
/// Supports:
/// - Numeric ranges: [1-10], [01-10], [1-10:2]
/// - Alphabetical ranges: [a-z], [A-Z]
/// - Sets: {a,b,c}
pub fn expand_url(url: &str) -> Result<Vec<String>> {
    let mut expanded = vec![url.to_string()];

    // 1. Expand sets {a,b,c}
    let set_re = Regex::new(r"\{([^}]+)\}").unwrap();
    while expanded.iter().any(|u| set_re.is_match(u)) {
        let mut next_level = Vec::new();
        for base in expanded {
            if let Some(caps) = set_re.captures(&base) {
                let full_match = caps.get(0).unwrap().as_str();
                let items: Vec<&str> = caps.get(1).unwrap().as_str().split(',').collect();
                for item in items {
                    next_level.push(base.replace(full_match, item));
                }
            } else {
                next_level.push(base);
            }
        }
        expanded = next_level;
    }

    // 2. Expand ranges [a-b] or [1-10]
    let range_re = Regex::new(r"\[([^\]]+)\]").unwrap();

    while expanded.iter().any(|u| range_re.is_match(u)) {
        let mut next_level = Vec::new();
        for base in expanded {
            if let Some(caps) = range_re.captures(&base) {
                let full_match = caps.get(0).unwrap().as_str();
                let range_str = caps.get(1).unwrap().as_str();

                let parts: Vec<&str> = range_str.split(':').collect();
                let inner = parts[0];
                let step = parts
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);

                let range_parts: Vec<&str> = inner.split('-').collect();
                if range_parts.len() != 2 {
                    return Err(Error::Protocol(format!(
                        "Invalid range format: [{}]",
                        range_str
                    )));
                }

                let start_str = range_parts[0];
                let end_str = range_parts[1];

                let mut replacements = Vec::new();

                // Numeric range
                if let (Ok(start), Ok(end)) = (start_str.parse::<i64>(), end_str.parse::<i64>()) {
                    let width = if start_str.starts_with('0') {
                        start_str.len()
                    } else {
                        0
                    };
                    let mut current = start;
                    let increment = if start <= end { 1 } else { -1 };

                    while (increment > 0 && current <= end) || (increment < 0 && current >= end) {
                        replacements.push(format!("{:0width$}", current, width = width));
                        current += (step as i64) * increment;
                        if step == 0 {
                            break;
                        }
                    }
                }
                // Alpha range
                else if start_str.len() == 1 && end_str.len() == 1 {
                    let start_char = start_str.chars().next().unwrap();
                    let end_char = end_str.chars().next().unwrap();

                    if start_char.is_ascii_alphabetic() && end_char.is_ascii_alphabetic() {
                        let mut current = start_char as u8;
                        let end = end_char as u8;
                        let increment = if current <= end { 1 } else { -1 };

                        while (increment > 0 && current <= end) || (increment < 0 && current >= end)
                        {
                            replacements.push((current as char).to_string());
                            if (increment > 0 && current > 255 - step as u8)
                                || (increment < 0 && current < step as u8)
                            {
                                break;
                            }
                            current = (current as i16 + (step as i16 * increment as i16)) as u8;
                            if step == 0 {
                                break;
                            }
                        }
                    }
                }

                if replacements.is_empty() {
                    return Err(Error::Protocol(format!(
                        "Unsupported or empty range: [{}]",
                        range_str
                    )));
                }

                for repl in replacements {
                    next_level.push(base.replace(full_match, &repl));
                }
            } else {
                next_level.push(base);
            }
        }
        expanded = next_level;
    }

    Ok(expanded)
}

#[cfg(test)]
mod tests {
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
}

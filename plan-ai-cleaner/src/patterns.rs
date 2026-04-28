use regex::Regex;
use std::sync::OnceLock;

use crate::types::EntityCategory;

pub struct PatternMatch {
    pub text: String,
    pub category: EntityCategory,
}

struct CompiledPattern {
    regex: Regex,
    category: EntityCategory,
}

fn compiled_patterns() -> &'static [CompiledPattern] {
    static PATTERNS: OnceLock<Vec<CompiledPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // Email addresses
            CompiledPattern {
                regex: Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b")
                    .unwrap(),
                category: EntityCategory::Email,
            },
            // US phone numbers
            CompiledPattern {
                regex: Regex::new(
                    r"\b(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
                )
                .unwrap(),
                category: EntityCategory::Phone,
            },
            // International phone numbers
            CompiledPattern {
                regex: Regex::new(r"\+\d{1,3}[-.\s]?\d{4,14}").unwrap(),
                category: EntityCategory::Phone,
            },
            // Social Security Numbers
            CompiledPattern {
                regex: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
                category: EntityCategory::Ssn,
            },
            // Credit card numbers (4 groups of 4 digits)
            CompiledPattern {
                regex: Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap(),
                category: EntityCategory::CreditCard,
            },
            // IPv4 addresses (post-filtered to exclude loopback etc.)
            CompiledPattern {
                regex: Regex::new(
                    r"\b(?:(?:25[0-5]|2[0-4][0-9]|1?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|1?[0-9][0-9]?)\b",
                )
                .unwrap(),
                category: EntityCategory::IpAddress,
            },
            // AWS access key IDs
            CompiledPattern {
                regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                category: EntityCategory::ApiKey,
            },
            // GitHub tokens
            CompiledPattern {
                regex: Regex::new(r"\bgh[ps]_[A-Za-z0-9]{36,}\b").unwrap(),
                category: EntityCategory::ApiKey,
            },
            // Generic API keys / secret tokens (sk-*, pk-*, etc.)
            CompiledPattern {
                regex: Regex::new(
                    r"\b(?:sk|pk|api|token|secret)[-_][A-Za-z0-9]{20,}\b",
                )
                .unwrap(),
                category: EntityCategory::ApiKey,
            },
            // JWTs (three base64url segments separated by dots)
            CompiledPattern {
                regex: Regex::new(
                    r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
                )
                .unwrap(),
                category: EntityCategory::Jwt,
            },
            // IPv6 addresses (common formats: full, compressed with ::)
            // Matches 2+ hex groups separated by colons, with optional :: compression.
            CompiledPattern {
                regex: Regex::new(
                    r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b",
                )
                .unwrap(),
                category: EntityCategory::IpAddress,
            },
            // IPv6 compressed (with ::)
            CompiledPattern {
                regex: Regex::new(
                    r"\b(?:[0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}\b",
                )
                .unwrap(),
                category: EntityCategory::IpAddress,
            },
            // URLs with embedded credentials (https://user:pass@host)
            CompiledPattern {
                regex: Regex::new(
                    r"https?://[^\s@]+:[^\s@]+@[^\s/]+",
                )
                .unwrap(),
                category: EntityCategory::ApiKey,
            },
        ]
    })
}

/// IPs to exclude from detection (loopback, broadcast, link-local prefix).
const EXCLUDED_IPS: &[&str] = &[
    "127.0.0.1",
    "0.0.0.0",
    "255.255.255.255",
    "0000:0000:0000:0000:0000:0000:0000:0001", // ::1 full form
];
const EXCLUDED_IP_PREFIXES: &[&str] = &["169.254.", "fe80:"];

/// Run all compiled regex patterns against the given text and return matches.
/// Deduplicates by exact matched text within the same category.
pub fn scan_text(text: &str) -> Vec<PatternMatch> {
    let patterns = compiled_patterns();
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for pat in patterns {
        for m in pat.regex.find_iter(text) {
            let matched = m.as_str().to_string();

            // Post-filter excluded IPs.
            if pat.category == EntityCategory::IpAddress {
                if EXCLUDED_IPS.contains(&matched.as_str())
                    || EXCLUDED_IP_PREFIXES.iter().any(|p| matched.starts_with(p))
                {
                    continue;
                }
            }

            let key = (matched.clone(), pat.category);
            if seen.insert(key) {
                results.push(PatternMatch {
                    text: matched,
                    category: pat.category,
                });
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_email() {
        let matches = scan_text("Contact john@example.com for details");
        assert!(matches.iter().any(|m| m.text == "john@example.com"));
    }

    #[test]
    fn detect_aws_key() {
        let matches = scan_text("key: AKIAIOSFODNN7EXAMPLE");
        assert!(matches
            .iter()
            .any(|m| m.text == "AKIAIOSFODNN7EXAMPLE" && matches!(m.category, EntityCategory::ApiKey)));
    }

    #[test]
    fn detect_ssn() {
        let matches = scan_text("SSN: 123-45-6789");
        assert!(matches.iter().any(|m| m.text == "123-45-6789"));
    }

    #[test]
    fn detect_credit_card() {
        let matches = scan_text("Card: 4111-1111-1111-1111");
        assert!(matches
            .iter()
            .any(|m| m.text == "4111-1111-1111-1111" && matches!(m.category, EntityCategory::CreditCard)));
    }

    #[test]
    fn detect_github_token() {
        let matches = scan_text("token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij");
        assert!(matches.iter().any(|m| matches!(m.category, EntityCategory::ApiKey)));
    }

    #[test]
    fn skip_loopback_ip() {
        let matches = scan_text("host: 127.0.0.1");
        assert!(!matches.iter().any(|m| m.text == "127.0.0.1"));
    }

    #[test]
    fn detect_ipv6_full() {
        let matches = scan_text("addr: 2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert!(matches
            .iter()
            .any(|m| m.text == "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
                && matches!(m.category, EntityCategory::IpAddress)));
    }

    #[test]
    fn detect_ipv6_compressed() {
        let matches = scan_text("addr: 2001:db8::8a2e:370:7334");
        assert!(matches
            .iter()
            .any(|m| matches!(m.category, EntityCategory::IpAddress)));
    }

    #[test]
    fn detect_credential_url() {
        let matches = scan_text("url: https://admin:s3cret@internal.corp.com/api");
        assert!(matches
            .iter()
            .any(|m| m.text.contains("admin:s3cret@") && matches!(m.category, EntityCategory::ApiKey)));
    }

    #[test]
    fn detect_public_ipv4() {
        let matches = scan_text("server at 203.0.113.42");
        assert!(matches
            .iter()
            .any(|m| m.text == "203.0.113.42" && matches!(m.category, EntityCategory::IpAddress)));
    }
}

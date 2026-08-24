//! Doorstop `ref` field classification per
//! INTEROP-doorstopRefHandling.
//!
//! URL-shaped refs (detected via an explicit prefix whitelist)
//! become a URL artifact + a `cites` link; anything else is
//! preserved verbatim in `legacy.ref`. The whitelist is
//! deliberately narrow — a scheme-plus-colon catch-all would
//! misclassify bibliographic citations like `author:1994:title`
//! as URLs.

/// Case-insensitive prefixes that count as URL-shaped.
///
/// Extending this list is deliberate: any future addition is a
/// policy decision that should come with a test update.
const URL_PREFIXES: &[&str] = &[
    "https://",
    "http://",
    "ftp://",
    "ftps://",
    "doi:",
    "urn:isbn:",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefClass {
    /// The ref is empty / absent — no import report row.
    None,
    /// A URL-shaped value. The plan builder emits a URL
    /// artifact + `cites` link for each Url ref.
    Url(String),
    /// Any non-empty non-URL value. Preserved verbatim in
    /// `legacy.ref`.
    NonUrl(String),
}

pub fn classify_ref(raw: Option<&str>) -> RefClass {
    let Some(raw) = raw else {
        return RefClass::None;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return RefClass::None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    for prefix in URL_PREFIXES {
        if lowered.starts_with(prefix) {
            // Preserve original casing in the output — only
            // the detection is case-insensitive.
            return RefClass::Url(trimmed.to_owned());
        }
    }
    RefClass::NonUrl(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_missing_refs_are_none() {
        assert_eq!(classify_ref(None), RefClass::None);
        assert_eq!(classify_ref(Some("")), RefClass::None);
        assert_eq!(classify_ref(Some("   ")), RefClass::None);
    }

    #[test]
    fn http_and_https_are_urls() {
        assert_eq!(
            classify_ref(Some("https://example.com/x")),
            RefClass::Url("https://example.com/x".to_owned())
        );
        assert_eq!(
            classify_ref(Some("http://example.com/x")),
            RefClass::Url("http://example.com/x".to_owned())
        );
    }

    #[test]
    fn prefixes_match_case_insensitively_but_preserve_original_casing() {
        assert_eq!(
            classify_ref(Some("HTTPS://Example.com")),
            RefClass::Url("HTTPS://Example.com".to_owned())
        );
    }

    #[test]
    fn doi_and_urn_isbn_are_urls() {
        assert_eq!(
            classify_ref(Some("doi:10.1000/xyz")),
            RefClass::Url("doi:10.1000/xyz".to_owned())
        );
        assert_eq!(
            classify_ref(Some("urn:isbn:0-306-40615-2")),
            RefClass::Url("urn:isbn:0-306-40615-2".to_owned())
        );
    }

    #[test]
    fn non_url_refs_pass_through_to_nonurl() {
        assert_eq!(
            classify_ref(Some("src/main.rs")),
            RefClass::NonUrl("src/main.rs".to_owned())
        );
        assert_eq!(
            classify_ref(Some("author:1994:title")),
            RefClass::NonUrl("author:1994:title".to_owned())
        );
        assert_eq!(
            classify_ref(Some("Smith 1994, chapter 4")),
            RefClass::NonUrl("Smith 1994, chapter 4".to_owned())
        );
    }

    #[test]
    fn trimming_strips_surrounding_whitespace() {
        assert_eq!(
            classify_ref(Some("  https://example.com  ")),
            RefClass::Url("https://example.com".to_owned())
        );
    }
}

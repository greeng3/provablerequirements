//! The set of blob extensions ReqForge accepts, per
//! `ART-uploadSupport`.
//!
//! Kept deliberately small for Phase 5 — the loader is responsible
//! for checking any blob sidecar against this list and flagging
//! unsupported entries as a `LoadDiagnostic` rather than silently
//! indexing a binary ReqForge can't render. Extensions are matched
//! case-insensitively so `DES-SPEC.PDF` works alongside
//! `des-spec.pdf`.

/// Extensions accepted for blob artifacts. `ART-uploadSupport` calls
/// out office documents, PDFs, and common image types as the
/// starting set; additional formats land through the thumbnailer
/// extensibility path in a later phase.
pub const ALLOWED_BLOB_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "xlsx", "pptx", "png", "jpg", "jpeg", "gif", "svg",
];

/// Is `ext` in the allowlist? Comparison is case-insensitive.
pub fn is_allowed_blob_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    ALLOWED_BLOB_EXTENSIONS
        .iter()
        .any(|allowed| *allowed == lower)
}

/// Best-effort guess at a media type from the extension. Used for
/// `Content-Type` headers and for the thumbnailer registry in
/// later sub-phases. Returns `application/octet-stream` for
/// unrecognised extensions so downstream code still has *something*
/// to serve.
pub fn media_type_for_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_is_case_insensitive() {
        assert!(is_allowed_blob_extension("pdf"));
        assert!(is_allowed_blob_extension("PDF"));
        assert!(is_allowed_blob_extension("Docx"));
    }

    #[test]
    fn unknown_extensions_are_rejected() {
        assert!(!is_allowed_blob_extension("exe"));
        assert!(!is_allowed_blob_extension(""));
        assert!(!is_allowed_blob_extension("md"));
    }

    #[test]
    fn media_type_covers_every_allowlisted_extension() {
        for ext in ALLOWED_BLOB_EXTENSIONS {
            assert_ne!(
                media_type_for_extension(ext),
                "application/octet-stream",
                "no media type mapped for allowlisted extension {ext}",
            );
        }
    }

    #[test]
    fn media_type_falls_back_for_unknown() {
        assert_eq!(media_type_for_extension("exe"), "application/octet-stream",);
    }
}

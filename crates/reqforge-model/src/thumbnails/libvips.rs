//! libvips-backed image provider (Phase 5c).
//!
//! Accepts the natively-renderable raster formats plus PDF
//! (libvips renders the first page transparently) and SVG. Shells
//! out to the `vips` CLI because the Rust bindings for libvips are
//! thin and bring in C headers we'd rather not vendor. If `vips`
//! isn't on `$PATH`, [`LibvipsProvider::probe`] returns `None` and
//! the registry simply doesn't install the provider — callers then
//! fall through to the "no thumbnailer" tier.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use super::{
    THUMBNAIL_LONGEST_EDGE_PX, ThumbnailError, ThumbnailProvider, binary_on_path,
    run_tool_with_timeout,
};

const VIPS_TIMEOUT: Duration = Duration::from_secs(30);

/// Formats libvips claims first-class support for. docx/xlsx/pptx
/// are *not* here — those fall to the LibreOffice provider which
/// converts to PDF then hands off to us via the cache.
pub const LIBVIPS_ACCEPTS: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/svg+xml",
    "application/pdf",
];

pub struct LibvipsProvider {
    pub(super) _private: (),
}

impl LibvipsProvider {
    /// Returns `Some(provider)` iff `vips --version` succeeds on
    /// `$PATH`. Dev machines without libvips installed get a
    /// `None` and the provider list simply doesn't grow.
    pub fn probe() -> Option<Self> {
        if binary_on_path("vips") {
            Some(Self { _private: () })
        } else {
            None
        }
    }
}

impl ThumbnailProvider for LibvipsProvider {
    fn name(&self) -> &'static str {
        "libvips"
    }

    fn accepts(&self, media_type: &str) -> bool {
        LIBVIPS_ACCEPTS.contains(&media_type)
    }

    fn generate(&self, input: &Path, output: &Path) -> Result<(), ThumbnailError> {
        // `vips thumbnail <in> <out> <width>` scales so the longest
        // edge is `width`; the `[page=0]` suffix renders page 1 for
        // PDFs (`vips` ignores the suffix for other formats, so no
        // special-casing is needed beyond the media-type check).
        let mut input_arg = OsString::from(input.as_os_str());
        if input
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            input_arg.push("[page=0]");
        }
        let width_arg = OsString::from(THUMBNAIL_LONGEST_EDGE_PX.to_string());

        let args: Vec<&std::ffi::OsStr> = vec![
            "thumbnail".as_ref(),
            input_arg.as_os_str(),
            output.as_os_str(),
            width_arg.as_os_str(),
        ];
        run_tool_with_timeout("vips", &args, VIPS_TIMEOUT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_none_when_binary_missing() {
        // Dev envs run this test as gospel — `vips` is not
        // installed, so the probe must refuse to register.
        if !binary_on_path("vips") {
            assert!(LibvipsProvider::probe().is_none());
        }
    }

    #[test]
    fn accepts_the_documented_media_types_and_rejects_others() {
        let provider = LibvipsProvider { _private: () };
        assert!(provider.accepts("image/png"));
        assert!(provider.accepts("image/jpeg"));
        assert!(provider.accepts("application/pdf"));
        assert!(provider.accepts("image/svg+xml"));
        assert!(
            !provider
                .accepts("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert!(!provider.accepts("text/plain"));
    }
}

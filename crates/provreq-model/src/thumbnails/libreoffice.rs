//! LibreOffice-headless provider (Phase 5c).
//!
//! Handles the Office family (docx / xlsx / pptx) by invoking
//! `soffice --headless --convert-to pdf` into a temp directory and
//! returning the PDF path. The [`ThumbnailRegistry`] re-feeds the
//! PDF through the libvips provider so the Office path shares the
//! same "PDF page 1 → 512 px PNG" tail with native PDF uploads.
//!
//! Soft-fails at probe time: if `soffice --version` doesn't
//! succeed the provider refuses to register. The runtime Dockerfile
//! (added in Phase 5d) will make the binary present; `cargo run`
//! on a dev machine without it simply falls through to the "no
//! thumbnailer" tier.

use std::path::Path;
use std::time::Duration;

use super::{ThumbnailError, ThumbnailProvider, binary_on_path, libvips, run_tool_with_timeout};

/// 30 seconds matches the locked Phase 5 decision. LibreOffice
/// startup alone can eat ~5 seconds cold, so a tighter budget
/// would false-positive on first-conversion.
const SOFFICE_TIMEOUT: Duration = Duration::from_secs(30);

pub const LIBREOFFICE_ACCEPTS: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

pub struct LibreofficeProvider {
    /// Keep libvips alongside so Office→PDF→PNG doesn't have to
    /// re-probe the binary on every call. `None` when libvips
    /// isn't installed: the provider refuses to register even if
    /// `soffice` is present, because a PDF we can't rasterise is
    /// strictly worse than "no thumbnailer for format".
    libvips: libvips::LibvipsProvider,
}

impl LibreofficeProvider {
    pub fn probe() -> Option<Self> {
        if !binary_on_path("soffice") {
            return None;
        }
        let libvips = libvips::LibvipsProvider::probe()?;
        Some(Self { libvips })
    }
}

impl ThumbnailProvider for LibreofficeProvider {
    fn name(&self) -> &'static str {
        "libreoffice"
    }

    fn accepts(&self, media_type: &str) -> bool {
        LIBREOFFICE_ACCEPTS.contains(&media_type)
    }

    fn generate(&self, input: &Path, output: &Path) -> Result<(), ThumbnailError> {
        let workdir = tempfile::tempdir().map_err(|source| ThumbnailError::Io {
            path: std::path::PathBuf::from("<tempdir>"),
            source,
        })?;

        // `soffice --headless --convert-to pdf --outdir <dir> <in>`
        // writes `<dir>/<stem>.pdf`. We don't pass `-env:...` here
        // because the Docker image ships a user-profile dir that
        // avoids clobber-on-first-run.
        let outdir = workdir.path();
        let args: Vec<&std::ffi::OsStr> = vec![
            "--headless".as_ref(),
            "--convert-to".as_ref(),
            "pdf".as_ref(),
            "--outdir".as_ref(),
            outdir.as_os_str(),
            input.as_os_str(),
        ];
        run_tool_with_timeout("soffice", &args, SOFFICE_TIMEOUT)?;

        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let intermediate_pdf = outdir.join(format!("{stem}.pdf"));
        if !intermediate_pdf.is_file() {
            return Err(ThumbnailError::Generator(format!(
                "soffice did not produce expected PDF at {}",
                intermediate_pdf.display()
            )));
        }

        // Hand off to libvips for the PDF→PNG rasterisation — same
        // path native PDF uploads take. The intermediate PDF drops
        // with `workdir` when this function returns.
        self.libvips.generate(&intermediate_pdf, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_none_when_soffice_missing() {
        if !binary_on_path("soffice") {
            assert!(LibreofficeProvider::probe().is_none());
        }
    }

    #[test]
    fn accepts_the_documented_office_types_and_rejects_others() {
        // Construct without probing so this test works on a dev
        // machine that lacks the binary — the accepts map is pure.
        let libvips = libvips::LibvipsProvider { _private: () };
        let provider = LibreofficeProvider { libvips };
        assert!(
            provider
                .accepts("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert!(
            provider.accepts("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        assert!(
            provider.accepts(
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            )
        );
        assert!(!provider.accepts("application/pdf"));
        assert!(!provider.accepts("image/png"));
    }
}

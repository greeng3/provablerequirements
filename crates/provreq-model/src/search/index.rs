//! Tantivy index construction. Each `AppState::publish` produces
//! a fresh in-memory index from the World snapshot; the old
//! index is dropped when the snapshot rotates.

use std::sync::Arc;

use tantivy::{
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyError,
    directory::RamDirectory,
    doc,
    schema::{FAST, Field, INDEXED, STORED, STRING, Schema, TEXT},
};

use crate::mount::MountState;
use crate::reviews::{ReviewState, derive_review_state};
use crate::schema::ArtifactShape;

/// Field handles carried with the schema so lookups in the
/// query layer are zero-cost.
pub struct SearchFields {
    pub uuid: Field,
    pub project_slug: Field,
    pub collection_prefix: Field,
    pub artifact_name: Field,
    pub title: Field,
    pub body: Field,
    pub description: Field,
    pub tags: Field,
    pub shape: Field,
    pub review_state: Field,
    pub active: Field,
    pub has_links: Field,
}

/// Pre-built Tantivy index owned by a World snapshot. `Arc`-
/// shared so handlers can clone cheaply without holding a
/// writer lock.
#[derive(Clone)]
pub struct SearchIndex {
    pub index: Index,
    pub reader: IndexReader,
    pub fields: Arc<SearchFields>,
}

impl std::fmt::Debug for SearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchIndex")
            .field("schema_fields", &"<12>")
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchIndexError {
    #[error("tantivy error: {0}")]
    Tantivy(#[from] TantivyError),
}

impl SearchIndex {
    /// Build a fresh index from the World's mount tree. Called
    /// inside `run_discovery` so the returned index shares the
    /// lifetime of its World.
    pub fn build(mounts: &[crate::mount::MountInfo]) -> Result<Self, SearchIndexError> {
        let (schema, fields) = build_schema();
        let directory = RamDirectory::create();
        let index = Index::create(directory, schema.clone(), Default::default())?;

        // 15 MB write buffer is comfortably over the largest
        // expected single-publish corpus; Tantivy rejects
        // buffers under 3 MB with InvalidArgument.
        let mut writer: IndexWriter = index.writer(15_000_000)?;
        for mount in mounts {
            let MountState::Project(project) = &mount.state else {
                continue;
            };
            for collection in &project.collections {
                for artifact in &collection.artifacts {
                    let review = derive_review_state(&artifact.metadata.review_log);
                    let review_tag = review_state_tag(&review.state);
                    let body_text = artifact.body.clone().unwrap_or_default();
                    let description_text =
                        artifact.metadata.description.clone().unwrap_or_default();
                    let tags_text = artifact
                        .metadata
                        .tags
                        .as_ref()
                        .map(|t| t.join(" "))
                        .unwrap_or_default();
                    let shape_text = shape_tag(artifact.metadata.shape);
                    let active_i = if artifact.metadata.is_active() { 1 } else { 0 };
                    let has_links_i = if artifact.metadata.links.is_empty() {
                        0
                    } else {
                        1
                    };
                    writer.add_document(doc!(
                        fields.uuid => artifact.metadata.uuid.to_string(),
                        fields.project_slug => project.config.slug.clone(),
                        fields.collection_prefix => collection.config.prefix.clone(),
                        fields.artifact_name => artifact.name.clone(),
                        fields.title => artifact.metadata.title.clone(),
                        fields.body => body_text,
                        fields.description => description_text,
                        fields.tags => tags_text,
                        fields.shape => shape_text,
                        fields.review_state => review_tag.to_string(),
                        fields.active => active_i as i64,
                        fields.has_links => has_links_i as i64,
                    ))?;
                }
            }
        }
        writer.commit()?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(SearchIndex {
            index,
            reader,
            fields: Arc::new(fields),
        })
    }
}

/// Convenience for tests + the pre-publish "no world yet"
/// fallback. Schema-valid empty index.
pub fn empty_index() -> Arc<SearchIndex> {
    Arc::new(SearchIndex::build(&[]).expect("empty SearchIndex should always build successfully"))
}

fn build_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let uuid = builder.add_text_field("uuid", STORED);
    let project_slug = builder.add_text_field("project_slug", STRING | STORED | FAST);
    let collection_prefix = builder.add_text_field("collection_prefix", STRING | STORED | FAST);
    let artifact_name = builder.add_text_field("artifact_name", TEXT | STORED);
    let title = builder.add_text_field("title", TEXT | STORED);
    let body = builder.add_text_field("body", TEXT | STORED);
    let description = builder.add_text_field("description", TEXT | STORED);
    let tags = builder.add_text_field("tags", TEXT | STORED);
    let shape = builder.add_text_field("shape", STRING | STORED | FAST);
    let review_state = builder.add_text_field("review_state", STRING | STORED | FAST);
    let active = builder.add_i64_field("active", INDEXED | FAST | STORED);
    let has_links = builder.add_i64_field("has_links", INDEXED | FAST | STORED);
    let schema = builder.build();
    let fields = SearchFields {
        uuid,
        project_slug,
        collection_prefix,
        artifact_name,
        title,
        body,
        description,
        tags,
        shape,
        review_state,
        active,
        has_links,
    };
    (schema, fields)
}

/// Stable kebab-case tag for the `review_state` filter field.
/// Matches the frontend's `MatrixReviewStateTag` on the
/// wire so operators can share filter values between views.
pub fn review_state_tag(state: &ReviewState) -> &'static str {
    match state {
        ReviewState::NeverReviewed => "never-reviewed",
        ReviewState::Approved => "approved",
        ReviewState::Rejected => "rejected",
        ReviewState::ReRequested => "re-requested",
    }
}

pub fn shape_tag(shape: ArtifactShape) -> &'static str {
    match shape {
        ArtifactShape::Content => "content",
        ArtifactShape::Blob => "blob",
        ArtifactShape::Url => "url",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #348 — the absorbed index builds against tantivy =0.24: an empty mount tree still produces a
    // schema-valid, reader-open index. The whole tantivy construction path (schema, RAM directory,
    // writer commit, reader) runs, so a version or feature drift in the pinned tantivy fails here
    // rather than only when the first real corpus is indexed.
    #[test]
    fn an_empty_index_builds_and_opens() {
        let index = SearchIndex::build(&[]).expect("empty index builds");
        // The searcher opens over the committed (empty) segment set — proof the whole
        // schema/writer/commit/reader path ran, not just that `build` returned.
        assert_eq!(index.reader.searcher().num_docs(), 0);
        // The `empty_index` convenience shares the same path.
        let _ = empty_index();
    }
}

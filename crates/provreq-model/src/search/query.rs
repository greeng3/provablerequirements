//! Query planning + execution against a `SearchIndex`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tantivy::{
    TantivyDocument, Term,
    collector::{Count, TopDocs},
    query::{AllQuery, BooleanQuery, Occur, Query, QueryParser, TermQuery},
    schema::{IndexRecordOption, Value},
    snippet::SnippetGenerator,
};

use super::index::SearchIndex;

/// Default response size + hard ceiling per the locked
/// decision — snippet generation is the per-hit cost driver,
/// and 200 hits × a few hundred bytes each keeps one request
/// under the 50 ms budget on mid-range hardware.
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    /// Raw Tantivy query string. An empty / whitespace-only
    /// query runs a match-all so pure-filter searches work
    /// ("every never-reviewed content artifact in REQ").
    #[serde(default)]
    pub q: Option<String>,
    /// `scope=system|project:<slug>|collection:<slug>/<prefix>`
    /// mirroring the Phase 6a reports shape.
    #[serde(default)]
    pub scope: Option<String>,
    /// CSV of shape tags (`content`, `blob`, `url`).
    #[serde(default)]
    pub shape: Option<String>,
    /// CSV of review-state kebab-case tags.
    #[serde(default)]
    pub review_state: Option<String>,
    /// Three-way: `true` → has ≥ 1 outgoing link, `false` →
    /// zero outgoing links, omitted → any.
    #[serde(default)]
    pub has_links: Option<bool>,
    #[serde(default)]
    pub include_inactive: Option<bool>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub uuid: String,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: String,
    pub review_state: String,
    pub active: bool,
    pub score: f32,
    /// HTML-escaped body excerpt with `<mark>...</mark>`
    /// markers around matching terms. `None` when the body
    /// didn't contribute to the hit (pure title / name /
    /// tag matches). The frontend splits on the literal
    /// `<mark>` tokens and renders spans — never
    /// `dangerouslySetInnerHTML`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub total_hits: usize,
    pub limit: usize,
    pub offset: usize,
    pub truncated: bool,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("malformed query: {0}")]
    BadQuery(String),
    #[error("unknown review state(s): {0}")]
    UnknownReviewStates(String),
    #[error("unknown shape(s): {0}")]
    UnknownShapes(String),
    #[error(transparent)]
    Tantivy(#[from] tantivy::TantivyError),
}

/// Resolved scope filter. The handler parses raw text into
/// `reports::Scope`; we translate down to index-level filter
/// terms here.
#[derive(Debug, Clone, Default)]
pub struct ScopeFilter {
    pub project_slug: Option<String>,
    pub collection_prefix: Option<String>,
}

impl ScopeFilter {
    pub fn from_reports_scope(scope: &crate::reports::Scope) -> Self {
        match scope {
            crate::reports::Scope::System => Self::default(),
            crate::reports::Scope::Project(slug) => Self {
                project_slug: Some(slug.clone()),
                collection_prefix: None,
            },
            crate::reports::Scope::Collection { slug, prefix } => Self {
                project_slug: Some(slug.clone()),
                collection_prefix: Some(prefix.clone()),
            },
        }
    }
}

/// Run a query against the search index. Returns a response
/// with hits + paging metadata.
pub fn run(
    index: &SearchIndex,
    scope: ScopeFilter,
    query: &SearchQuery,
) -> Result<SearchResponse, SearchError> {
    let searcher = index.reader.searcher();
    let fields = &index.fields;

    // Default query fields: unqualified terms hit every
    // indexed-text column per UX-search.
    let parser = QueryParser::for_index(
        &index.index,
        vec![
            fields.title,
            fields.artifact_name,
            fields.body,
            fields.description,
            fields.tags,
        ],
    );

    let text_query: Box<dyn Query> = match query.q.as_deref().map(str::trim) {
        Some(q) if !q.is_empty() => parser
            .parse_query(q)
            .map_err(|e| SearchError::BadQuery(e.to_string()))?,
        _ => Box::new(AllQuery),
    };

    // Structured filters AND onto the text query.
    let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, text_query)];

    if let Some(slug) = &scope.project_slug {
        clauses.push((Occur::Must, term_query(fields.project_slug, slug)));
    }
    if let Some(prefix) = &scope.collection_prefix {
        clauses.push((Occur::Must, term_query(fields.collection_prefix, prefix)));
    }

    if let Some(list) = csv_field(query.shape.as_deref()) {
        let parsed = parse_shape_list(&list)?;
        clauses.push((Occur::Must, any_of_terms(fields.shape, &parsed)));
    }

    if let Some(list) = csv_field(query.review_state.as_deref()) {
        let parsed = parse_review_state_list(&list)?;
        clauses.push((Occur::Must, any_of_terms(fields.review_state, &parsed)));
    }

    if let Some(has_links) = query.has_links {
        clauses.push((
            Occur::Must,
            i64_term_query(fields.has_links, if has_links { 1 } else { 0 }),
        ));
    }

    // Active filter: default excludes inactive. `includeInactive=true`
    // skips the clause altogether; `false` (default) forces active=1.
    if !query.include_inactive.unwrap_or(false) {
        clauses.push((Occur::Must, i64_term_query(fields.active, 1)));
    }

    let combined: Box<dyn Query> = Box::new(BooleanQuery::new(clauses));

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);

    let total_hits = searcher.search(&*combined, &Count)?;
    let top_collector = TopDocs::with_limit(limit).and_offset(offset);
    let top = searcher.search(&*combined, &top_collector)?;

    // Snippet generator operates on the body field only; we
    // skip it when the body is empty or wasn't indexed (blob /
    // url artifacts don't carry a body).
    let snippet_gen = SnippetGenerator::create(&searcher, &*combined, fields.body)
        .ok()
        .map(|mut g| {
            // 180 chars of context is enough for one line at
            // typical font-size.
            g.set_max_num_chars(180);
            g
        });

    let mut hits: Vec<SearchHit> = Vec::with_capacity(top.len());
    for (score, doc_address) in top {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        let snippet_text = snippet_gen.as_ref().and_then(|g| {
            let snip = g.snippet_from_doc(&doc);
            if snip.fragment().is_empty() {
                None
            } else {
                // Tantivy's Snippet::to_html() wraps matches in
                // `<b>` tags; the frontend splits on a literal
                // `<mark>` token, so rewrite the wrapper here.
                // The fragment text and the tags are the only
                // HTML content the snippet emits — a straight
                // string swap is safe.
                let html = snip.to_html();
                Some(html.replace("<b>", "<mark>").replace("</b>", "</mark>"))
            }
        });
        hits.push(SearchHit {
            uuid: field_str(&doc, fields.uuid).unwrap_or_default(),
            project_slug: field_str(&doc, fields.project_slug).unwrap_or_default(),
            collection_prefix: field_str(&doc, fields.collection_prefix).unwrap_or_default(),
            artifact_name: field_str(&doc, fields.artifact_name).unwrap_or_default(),
            title: field_str(&doc, fields.title).unwrap_or_default(),
            shape: field_str(&doc, fields.shape).unwrap_or_default(),
            review_state: field_str(&doc, fields.review_state).unwrap_or_default(),
            active: field_i64(&doc, fields.active).unwrap_or(1) != 0,
            score,
            snippet: snippet_text,
        });
    }

    let returned = hits.len();
    Ok(SearchResponse {
        total_hits,
        limit,
        offset,
        truncated: offset + returned < total_hits,
        hits,
    })
}

fn term_query(field: tantivy::schema::Field, value: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_text(field, value),
        IndexRecordOption::Basic,
    ))
}

fn i64_term_query(field: tantivy::schema::Field, value: i64) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_i64(field, value),
        IndexRecordOption::Basic,
    ))
}

fn any_of_terms(field: tantivy::schema::Field, values: &[String]) -> Box<dyn Query> {
    let sub: Vec<(Occur, Box<dyn Query>)> = values
        .iter()
        .map(|v| (Occur::Should, term_query(field, v)))
        .collect();
    Box::new(BooleanQuery::new(sub))
}

fn field_str(doc: &TantivyDocument, field: tantivy::schema::Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
}

fn field_i64(doc: &TantivyDocument, field: tantivy::schema::Field) -> Option<i64> {
    doc.get_first(field).and_then(|v| v.as_i64())
}

fn csv_field(raw: Option<&str>) -> Option<Vec<String>> {
    let raw = raw?;
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.iter().any(|e| e == trimmed) {
            out.push(trimmed.to_owned());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn parse_shape_list(list: &[String]) -> Result<Vec<String>, SearchError> {
    let allowed = ["content", "blob", "url"];
    let mut out: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for entry in list {
        if allowed.contains(&entry.as_str()) {
            out.push(entry.clone());
        } else {
            unknown.push(entry.clone());
        }
    }
    if !unknown.is_empty() {
        return Err(SearchError::UnknownShapes(unknown.join(", ")));
    }
    Ok(out)
}

fn parse_review_state_list(list: &[String]) -> Result<Vec<String>, SearchError> {
    let allowed: HashSet<&str> = ["approved", "rejected", "re-requested", "never-reviewed"]
        .into_iter()
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for entry in list {
        if allowed.contains(entry.as_str()) {
            out.push(entry.clone());
        } else {
            unknown.push(entry.clone());
        }
    }
    if !unknown.is_empty() {
        return Err(SearchError::UnknownReviewStates(unknown.join(", ")));
    }
    Ok(out)
}

/// Tests are colocated so the query layer can reach into
/// the index fields without publishing them externally.
#[cfg(test)]
mod tests {
    use super::super::index::{SearchIndex, shape_tag};
    use super::*;
    use crate::reports::compute::test_support::{hint, link, make_artifact, make_world};
    use crate::schema::ArtifactShape;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn uuid(seed: u8) -> Uuid {
        let mut b = [0u8; 16];
        b[15] = seed;
        b[6] = 0x70 | seed;
        Uuid::from_bytes(b)
    }

    fn sample_index() -> SearchIndex {
        let mut a = make_artifact(
            "REQ-core",
            uuid(1),
            "Core requirement",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "satisfies",
                hint("sample", "DES", "DES-impl"),
            )],
            None,
        );
        a.metadata.description = Some("root-level safety goal".into());
        a.metadata.tags = Some(vec!["core".into(), "safety".into()]);
        a.body = Some("The reactor vessel shall satisfy the pressure envelope.".into());

        let mut b = make_artifact(
            "REQ-legacy",
            uuid(3),
            "Legacy interlock",
            ArtifactShape::Content,
            vec![],
            Some(false),
        );
        b.body = Some("Legacy paragraph about interlocks and wiring.".into());

        let d = make_artifact(
            "DES-impl",
            uuid(2),
            "Implementation design",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("requirements".into(), "REQ".into(), vec![a, b]),
                ("designs".into(), "DES".into(), vec![d]),
            ],
        );
        SearchIndex::build(&world.mounts).unwrap()
    }

    #[test]
    fn default_field_search_hits_title_and_body() {
        let index = sample_index();
        // "reactor" is only in REQ-core's body.
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                q: Some("reactor".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resp.total_hits, 1);
        assert_eq!(resp.hits[0].artifact_name, "REQ-core");
        // Body contributed → snippet present.
        assert!(resp.hits[0].snippet.is_some());
    }

    #[test]
    fn empty_query_runs_match_all() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                q: Some("   ".into()),
                ..Default::default()
            },
        )
        .unwrap();
        // Default excludes inactive: REQ-core + DES-impl only.
        assert_eq!(resp.total_hits, 2);
    }

    #[test]
    fn field_scoped_query_only_hits_named_field() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                // "interlock" is in REQ-legacy's body AND its title
                // ("Legacy interlock"). Scoping to title hits the
                // title only; REQ-legacy is inactive so we need the
                // flag.
                q: Some("title:interlock".into()),
                include_inactive: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resp.total_hits, 1);
        assert_eq!(resp.hits[0].artifact_name, "REQ-legacy");
    }

    #[test]
    fn boolean_required_and_excluded_terms_narrow_results() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                // Must match "requirement" but must not match
                // "Implementation". REQ-core has "requirement" in
                // its title and nothing about implementation;
                // DES-impl has "Implementation" so it's excluded.
                // The `+term -term` form is Tantivy's native
                // required/excluded syntax.
                q: Some("+requirement -Implementation".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resp.total_hits, 1);
        assert_eq!(resp.hits[0].artifact_name, "REQ-core");
    }

    #[test]
    fn shape_filter_narrows_to_requested_shapes() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                shape: Some("content".into()),
                ..Default::default()
            },
        )
        .unwrap();
        for h in &resp.hits {
            assert_eq!(h.shape, "content");
        }
    }

    #[test]
    fn has_links_filter_keeps_only_that_partition() {
        let index = sample_index();
        let with_links = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                has_links: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(with_links.total_hits, 1);
        assert_eq!(with_links.hits[0].artifact_name, "REQ-core");

        let no_links = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                has_links: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        // Default excludes inactive so REQ-legacy is gone; DES-impl
        // has no outgoing links and is active → one hit.
        assert_eq!(no_links.total_hits, 1);
        assert_eq!(no_links.hits[0].artifact_name, "DES-impl");
    }

    #[test]
    fn scope_filter_narrows_project_and_collection() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter {
                project_slug: Some("sample".into()),
                collection_prefix: Some("DES".into()),
            },
            &SearchQuery::default(),
        )
        .unwrap();
        assert_eq!(resp.total_hits, 1);
        assert_eq!(resp.hits[0].artifact_name, "DES-impl");
    }

    #[test]
    fn unknown_shape_and_review_state_surface_typed_errors() {
        let index = sample_index();
        let bad_shape = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                shape: Some("content,bogus".into()),
                ..Default::default()
            },
        );
        assert!(matches!(bad_shape, Err(SearchError::UnknownShapes(_))));

        let bad_rs = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                review_state: Some("approved,bogus".into()),
                ..Default::default()
            },
        );
        assert!(matches!(bad_rs, Err(SearchError::UnknownReviewStates(_))));
    }

    #[test]
    fn malformed_query_returns_bad_query_error() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                // Unmatched quote — Tantivy's query parser rejects.
                q: Some("\"reactor".into()),
                ..Default::default()
            },
        );
        assert!(matches!(resp, Err(SearchError::BadQuery(_))));
    }

    #[test]
    fn pagination_offsets_and_truncates() {
        let index = sample_index();
        // Match-all with a tiny limit so pagination kicks in.
        let page1 = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                limit: Some(1),
                offset: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page1.hits.len(), 1);
        assert_eq!(page1.total_hits, 2);
        assert!(page1.truncated);

        let page2 = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                limit: Some(1),
                offset: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page2.hits.len(), 1);
        assert_eq!(page2.total_hits, 2);
        assert!(!page2.truncated);
    }

    #[test]
    fn snippet_uses_mark_tag_and_shows_up_when_body_matches() {
        let index = sample_index();
        let resp = run(
            &index,
            ScopeFilter::default(),
            &SearchQuery {
                q: Some("reactor".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let snippet = resp.hits[0].snippet.clone().unwrap();
        assert!(snippet.contains("<mark>"));
    }

    /// Shape tag keeps the same wire values as the rest of the
    /// codebase (content / blob / url), so a regression on the
    /// ArtifactShape enum would be caught here before it rides
    /// into index docs.
    #[test]
    fn shape_tags_match_schema_values() {
        assert_eq!(shape_tag(ArtifactShape::Content), "content");
        assert_eq!(shape_tag(ArtifactShape::Blob), "blob");
        assert_eq!(shape_tag(ArtifactShape::Url), "url");
    }
}

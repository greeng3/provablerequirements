// TypeScript mirrors of the JSON DTOs defined in
// backend/reqforge-server/src/http/dto.rs. When the backend wire
// format changes, both sides need to move in lockstep.

export type ArtifactShape = "content" | "blob" | "url";

export interface HealthResponse {
  status: string;
}

export interface ReadinessResponse {
  ready: boolean;
}

export interface ApiError {
  error: string;
}

export interface ProjectSummary {
  slug: string;
  name: string;
  description: string | null;
  collectionCount: number;
  artifactCount: number;
}

export interface CollectionSummary {
  prefix: string;
  name: string;
  description: string | null;
  artifactCount: number;
  expectsCodeTrace: boolean;
}

export interface ProjectDetail {
  slug: string;
  name: string;
  description: string | null;
  artifactsPath: string;
  collections: CollectionSummary[];
  /** Phase 11a: per-project schema diagnostics for files whose
   * on-disk schemaVersion is newer than this build knows. Omitted
   * from the wire when empty, so v1-clean projects keep the
   * prior shape. The frontend shows a banner on the project
   * when any diagnostic is present. */
  schemaDiagnostics?: SchemaDiagnostic[];
}

export type SchemaFileType = "artifact" | "collection" | "project" | "system";

export interface SchemaDiagnostic {
  path: string;
  fileType: SchemaFileType;
  foundVersion: number;
  currentVersion: number;
}

export interface MigrateSchemaRequest {
  force?: boolean;
}

export interface MigrationOutcome {
  fileType: SchemaFileType;
  fromVersion: number;
  toVersion: number;
  migrated: boolean;
}

export interface FileRewrite {
  path: string;
  outcome: MigrationOutcome;
}

export interface FileFailure {
  path: string;
  fileType: SchemaFileType;
  error: string;
}

export interface BulkMigrateResult {
  filesScanned: number;
  filesRewritten: number;
  filesUpToDate: number;
  failures: FileFailure[];
  rewritten: FileRewrite[];
}

export interface MigrateSchemaResponse {
  projectSlug: string;
  result: BulkMigrateResult;
}

// --- Phase 11b: sample-content onboarding --------------------------------

export interface SampleContentCollectionSummary {
  prefix: string;
  directoryName: string;
  artifactCount: number;
  artifactNames: string[];
}

export interface SampleContentResponse {
  projectSlug: string;
  collectionsCreated: number;
  artifactsCreated: number;
  collections: SampleContentCollectionSummary[];
}

export interface ArtifactListing {
  name: string;
  uuid: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
  reviewState: ReviewState;
}

export interface LinkHint {
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
}

export type LinkTypeSource = "builtin" | "system";

export interface LinkType {
  name: string;
  inverseName: string;
  directed: boolean;
  acyclic: boolean;
  source: LinkTypeSource;
}

export type LinkResolution = "resolved" | "unresolved" | "unknownType";

export interface LinkTargetSummary {
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
}

/// Server-resolved view of one outgoing link; replaces the raw
/// on-disk `Link` shape in ArtifactDetail.links from Phase 3a on.
export interface LinkView {
  targetUuid: string;
  type: string;
  hint: LinkHint;
  resolution: LinkResolution;
  typeMetadata?: LinkType;
  targetSummary?: LinkTargetSummary;
}

/// Wire-side input shape accepted by PUT /api/artifacts/:uuid
/// when replacing the `links` array.
export interface LinkWriteRequest {
  targetUuid: string;
  type: string;
  hint?: LinkHint;
}

export interface ArtifactSearchResult {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  active: boolean;
}

export interface ReviewLogEntry {
  timestamp: string;
  reviewer: string;
  outcome: string;
  explanation?: string;
  addedTodos?: Array<{ id: string; text: string }>;
  resolvedTodos?: string[];
}

export type ReviewState =
  | "neverReviewed"
  | "approved"
  | "rejected"
  | "reRequested";

export interface OpenTodo {
  id: string;
  text: string;
  addedAt: string;
  addedBy: string;
}

export interface DerivedReviewState {
  state: ReviewState;
  lastApprovalAt?: string;
  lastEventAt?: string;
  lastReviewer?: string;
  blockingTodos: OpenTodo[];
}

export interface ReviewerIdentityOptions {
  gitDefault?: string;
  persisted: string[];
  session: string[];
}

export interface ReviewQueueEntry {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  state: ReviewState;
  lastEventAt?: string;
  modifiedAt: string;
  blockingTodoCount: number;
  tags: string[];
  lastReviewer?: string;
}

export interface ReviewQueueResponse {
  awaitingReview: ReviewQueueEntry[];
  blockingTodos: ReviewQueueEntry[];
}

export interface ReviewQueueFilters {
  projectSlug?: string;
  collectionPrefix?: string;
  shape?: ArtifactShape;
  tag?: string;
  reviewer?: string;
  order?: "oldest-first" | "newest-first";
}

export type CreateReviewRequest =
  | { reviewer: string; action: "approve"; explanation?: string }
  | {
      reviewer: string;
      action: "reject-with-todo";
      todo: { id?: string; text: string };
      explanation?: string;
    }
  | {
      reviewer: string;
      action: "add-todo";
      todo: { id?: string; text: string };
      explanation?: string;
    }
  | {
      reviewer: string;
      action: "resolve-todo";
      todoId: string;
      explanation?: string;
    }
  | { reviewer: string; action: "re-request-review"; explanation?: string };

export interface LastApprovalSnapshot {
  approvedAt: string;
  body: string;
  metadata: Record<string, unknown>;
}

/// Phase 5c: shape-specific payload surfaced alongside core
/// artifact fields on `ArtifactDetail`. The server always fills
/// `blob` for blob shapes and `url/checkedAt/checkStatus` for URL
/// shapes; all five fields are `undefined` for content-shape
/// artifacts.
export interface BlobDetail {
  byteSize: number;
  contentHash: string;
  mediaType: string;
  downloadUrl: string;
  thumbnailUrl: string;
}

/// Stable checkStatus strings returned by the URL-check endpoint.
/// Kept as a union rather than a string enum so unknown values
/// arriving from a newer backend don't crash the narrow-chain.
export type UrlCheckStatus =
  | "ok"
  | "redirect"
  | "not-found"
  | "forbidden"
  | "unauthorized"
  | "server-error"
  | "timeout"
  | "dns-error"
  | "tls-error"
  | "other";

export interface ArtifactDetail {
  name: string;
  projectSlug: string;
  collectionPrefix: string;
  uuid: string;
  title: string;
  shape: ArtifactShape;
  description: string | null;
  active: boolean;
  derived: boolean;
  createdAt: string;
  modifiedAt: string;
  tags: string[];
  outlineLevel?: string;
  links: LinkView[];
  reviewLog: ReviewLogEntry[];
  reviewState: DerivedReviewState;
  body: string | null;
  blob?: BlobDetail;
  url?: string;
  checkedAt?: string;
  checkStatus?: UrlCheckStatus;
}

export interface CreateArtifactRequest {
  name: string;
  title: string;
  description?: string | null;
  body?: string;
  tags?: string[];
  active?: boolean;
  derived?: boolean;
  outlineLevel?: string;
}

export interface UpdateArtifactRequest {
  title?: string;
  description?: string | null;
  body?: string;
  active?: boolean | null;
  derived?: boolean | null;
  tags?: string[] | null;
  outlineLevel?: string | null;
  /// Full-array replacement: omit to leave links unchanged, pass []
  /// to clear. Each entry's hint is optional — the server fills it
  /// from the UUID index when the target is mounted.
  links?: LinkWriteRequest[];
}

export interface UpdateArtifactUrlRequest {
  url: string;
}

export interface CreateUrlArtifactRequest {
  name: string;
  title: string;
  url: string;
  description?: string | null;
  tags?: string[];
  active?: boolean;
  derived?: boolean;
  outlineLevel?: string;
}

export interface CheckUrlResponse {
  uuid: string;
  checkedAt: string;
  checkStatus: UrlCheckStatus;
}

export interface BulkCheckUrlsResponse {
  checked: CheckUrlResponse[];
}

export interface RenameArtifactRequest {
  name: string;
}

export interface CreateCollectionRequest {
  dirName: string;
  prefix: string;
  name: string;
  description?: string | null;
  expectsCodeTrace?: boolean;
}

export interface IncomingLinkEntry {
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  sourceUuid: string;
  linkType: string;
}

/// Phase 5d: one commit that touched the artifact's tracked file.
/// Serves the history dropdown for the standalone diff route.
export interface CommitInfo {
  oid: string;
  shortOid: string;
  committedAt: string;
  author: string;
  summary: string;
}

/// Response for `GET /api/artifacts/:uuid/history`. When git
/// history is unavailable the server returns an empty `commits`
/// array plus a `fallbackReason` string; the UI banners the
/// reason rather than showing an empty list silently.
export interface ArtifactHistoryResponse {
  commits: CommitInfo[];
  fallbackReason?: string;
}

/// Shape-tagged union mirroring `crate::diff::ShapeDiff`. The UI
/// dispatches on `shape` before rendering so type narrowing does
/// the heavy lifting.
export type ShapeDiff =
  | { shape: "content"; lines: DiffLine[] }
  | { shape: "blob"; before?: BlobDiffSide; after?: BlobDiffSide }
  | { shape: "url"; before?: string; after?: string; note: string };

export interface DiffLine {
  kind: "same" | "added" | "removed";
  text: string;
}

export interface BlobDiffSide {
  byteSize: number;
  contentHash: string;
  mediaType: string;
  downloadUrl: string;
}

/// Response for `GET /api/artifacts/:uuid/diff`. `diff` is the
/// shape-tagged union above; `fallbackReason` is only present
/// when the server couldn't resolve `from`/`to` against git
/// history and fell through to an empty shape-appropriate diff
/// (per the Phase 5 locked fallback decision).
export interface ArtifactDiffResponse {
  shape: ArtifactShape;
  fromLabel: string;
  toLabel: string;
  diff: ShapeDiff;
  fallbackReason?: string;
}

// ---- Phase 6a: report catalog ----

/// Stable wire identifiers for every report kind. URLs use the
/// kebab-case form; the frontend mirrors the backend enum so
/// typos surface at compile time.
export type ReportKind =
  | "unresolved-links"
  | "link-orphans"
  | "cycles"
  | "conflicts"
  | "coverage-matrix"
  | "impact-analysis"
  | "review-status"
  | "filesystem-orphans"
  | "code-traceability";

/// `?scope=` query-string forms. `system` is the default.
export type ReportScopeParam =
  | "system"
  | `project:${string}`
  | `collection:${string}/${string}`;

/// Scope as the server echoes it back inside report bodies —
/// the frontend renders "for collection REQ" headers from this
/// without re-parsing the query string.
export type ReportScopeDto =
  | { kind: "system" }
  | { kind: "project"; slug: string }
  | { kind: "collection"; slug: string; prefix: string };

/// Tagged union response from `GET /api/reports/:kind`. All eight
/// report kinds are live as of Phase 6a.4.
export type ReportResponse =
  | UnresolvedLinksReport
  | LinkOrphansReport
  | CyclesReportPayload
  | ConflictsReportPayload
  | CoverageMatrixReportPayload
  | ImpactAnalysisReportPayload
  | ReviewStatusReportPayload
  | FilesystemOrphansReportPayload
  | CodeTraceabilityReportPayload;

export interface UnresolvedLinksReport {
  kind: "unresolved-links";
  scope: ReportScopeDto;
  totalUnresolved: number;
  entries: UnresolvedLinkReportEntry[];
}

/// Stable reason strings from `src/reports/compute.rs`. An
/// unknown value lands in the catch-all `string` so the frontend
/// doesn't crash on a backend version skew.
export type UnresolvedLinkReason = "target-missing" | "mount-missing";

export interface UnresolvedLinkReportEntry {
  sourceUuid: string;
  sourceProjectSlug: string;
  sourceCollectionPrefix: string;
  sourceArtifactName: string;
  sourceTitle: string;
  sourceShape: ArtifactShape;
  linkType: string;
  targetUuid: string;
  targetHintProjectSlug: string;
  targetHintCollectionPrefix: string;
  targetHintArtifactName: string;
  reason: UnresolvedLinkReason | string;
}

export interface LinkOrphansReport {
  kind: "link-orphans";
  scope: ReportScopeDto;
  totalOrphans: number;
  entries: LinkOrphanReportEntry[];
}

export interface LinkOrphanReportEntry {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
  derived: boolean;
}

/// One node in a cycle or conflict pair. Mirrors `CycleNode` on
/// the backend — shared DTO for both reports so the frontend
/// renders artifact breadcrumbs identically across the two.
export interface ReportGraphNode {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
}

export interface CyclesReportPayload {
  kind: "cycles";
  scope: ReportScopeDto;
  linkTypesChecked: string[];
  totalCycles: number;
  truncated: boolean;
  cycles: CycleReportEntry[];
}

export interface CycleReportEntry {
  linkType: string;
  nodes: ReportGraphNode[];
}

export interface ConflictsReportPayload {
  kind: "conflicts";
  scope: ReportScopeDto;
  totalPairs: number;
  pairs: ConflictPairEntry[];
}

export interface ConflictPairEntry {
  first: ReportGraphNode;
  second: ReportGraphNode;
  bidirectional: boolean;
}

export interface CoverageMatrixReportPayload {
  kind: "coverage-matrix";
  scope: ReportScopeDto;
  coveringLinkTypes: string[];
  unknownRequestedTypes: string[];
  totalParents: number;
  gapCount: number;
  parents: CoverageParentEntry[];
}

export interface CoverageParentEntry {
  parent: ReportGraphNode;
  hasGap: boolean;
  coveringChildren: CoverageChildEntry[];
  /// Phase 9b: in-code tags whose verb matches the effective
  /// covering link-type set. Field is absent (not empty) on
  /// responses where no code evidence covers the parent — the
  /// backend skip_serializes empty vectors — so renderers
  /// guard with `?? []`.
  coveringCodeEvidence?: CoverageCodeEntry[];
}

export interface CoverageChildEntry {
  child: ReportGraphNode;
  linkType: string;
}

export interface CoverageCodeEntry {
  file: string;
  line: number;
  verb: string;
}

export type ImpactDirection = "dependents" | "dependencies";

export interface ImpactAnalysisReportPayload {
  kind: "impact-analysis";
  scope: ReportScopeDto;
  seed?: ReportGraphNode;
  direction: ImpactDirection | string;
  totalImpacted: number;
  impacted: ImpactedArtifactEntry[];
  missingSeedReason?: string;
}

export interface ImpactedArtifactEntry {
  node: ReportGraphNode;
  depth: number;
  linkTypes: string[];
}

export interface ReviewStatusCounts {
  approved: number;
  rejected: number;
  reRequested: number;
  neverReviewed: number;
}

export interface ReviewStatusReportPayload {
  kind: "review-status";
  scope: ReportScopeDto;
  totals: ReviewStatusCounts;
  byProject: Array<{ projectSlug: string; counts: ReviewStatusCounts }>;
  byCollection: Array<{
    projectSlug: string;
    collectionPrefix: string;
    counts: ReviewStatusCounts;
  }>;
  byShape: {
    content: ReviewStatusCounts;
    blob: ReviewStatusCounts;
    url: ReviewStatusCounts;
  };
}

export interface FilesystemOrphansReportPayload {
  kind: "filesystem-orphans";
  scope: ReportScopeDto;
  missingSidecar: OrphanBinaryEntry[];
  missingBinary: OrphanSidecarEntry[];
}

export interface OrphanBinaryEntry {
  projectSlug: string;
  collectionPrefix: string;
  filename: string;
  binaryRelativePath: string;
  byteSize: number;
  mediaType: string;
}

export interface OrphanSidecarEntry {
  projectSlug: string;
  collectionPrefix: string;
  sidecarFilename: string;
  declaredBlobPath: string;
}

/// Phase 9b: code-traceability report payload. Per-artifact
/// listing of in-code tag locations grouped by Phase 9a
/// canonical verb, plus orphan tags (in-code references that
/// didn't resolve to a mounted artifact).
export interface CodeTraceabilityReportPayload {
  kind: "code-traceability";
  scope: ReportScopeDto;
  totalArtifacts: number;
  uncoveredCount: number;
  orphanTagCount: number;
  entries: CodeTraceabilityEntry[];
  orphanTags: CodeTraceabilityOrphan[];
}

export interface CodeTraceabilityEntry {
  artifact: ReportGraphNode;
  expectsCodeTrace: boolean;
  hasGap: boolean;
  /// Verb → list of source locations. Verbs are the Phase 9a
  /// canonical forms (`"Satisfies"`, `"Verifies"`, ...).
  locationsByVerb: Record<string, CodeTraceabilityLocation[]>;
}

export interface CodeTraceabilityLocation {
  file: string;
  line: number;
}

export interface CodeTraceabilityOrphan {
  file: string;
  line: number;
  verb: string;
  rawId: string;
}

/// Request body for the Adopt-as-artifact wizard.
export interface AdoptOrphanBlobRequest {
  name: string;
  title: string;
  binaryRelativePath: string;
  description?: string | null;
  tags?: string[];
  active?: boolean;
  derived?: boolean;
  outlineLevel?: string;
}

/// Opaque JSON blob the server persists for each report kind.
/// The shape is owned by the frontend — the backend just
/// round-trips it.
export interface SavedReportConfig {
  scope?: ReportScopeParam;
  includeInactive?: boolean;
  /// Per-kind options (coverage-matrix covering link types, etc.)
  /// ride through here in later sub-phases. Kept as a dictionary
  /// so adding options doesn't need a schema change.
  options?: Record<string, unknown>;
}

// ---- Phase 7a: graph-canvas DTOs ----

/// Soft cap the backend enforces on canvas-visible nodes. Kept
/// in sync with `GRAPH_NODE_CAP` in
/// `backend/reqforge-server/src/graph/mod.rs`; the frontend uses
/// it for the truncation banner's "500 of N" copy.
export const GRAPH_NODE_CAP = 500;

export interface GraphNodeDto {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
  derived: boolean;
  tags: string[];
}

export interface GraphEdgeDto {
  sourceUuid: string;
  targetUuid: string;
  linkType: string;
  acyclic: boolean;
  directed: boolean;
}

export interface GraphLinkTypeDto {
  name: string;
  inverseName: string;
  directed: boolean;
  acyclic: boolean;
}

export interface GraphResponse {
  scope: ReportScopeDto;
  totalNodes: number;
  truncated: boolean;
  hintAllEdgesAcyclic: boolean;
  referencedLinkTypes: GraphLinkTypeDto[];
  nodes: GraphNodeDto[];
  edges: GraphEdgeDto[];
}

/// Query-string parameters the frontend sends on `GET /api/graph`.
export interface GraphQueryParams {
  scope?: ReportScopeParam;
  includeInactive?: boolean;
  /// Explicit whitelist of link-type names; `undefined` or empty
  /// means "all link types".
  linkTypes?: string[];
  /// Explicit whitelist of tag strings; `undefined` or empty
  /// means "all tags".
  tags?: string[];
}

// ---- Phase 7b: matrix-link-view DTOs ----

/// Soft cap the backend enforces per axis. Kept in sync with
/// `MATRIX_AXIS_CAP` in
/// `backend/reqforge-server/src/matrix/mod.rs`; the frontend
/// uses it for the truncation banner's "N items" copy.
export const MATRIX_AXIS_CAP = 500;

export type MatrixReviewStateTag =
  | "never-reviewed"
  | "approved"
  | "rejected"
  | "re-requested";

export const MATRIX_REVIEW_STATE_TAGS: MatrixReviewStateTag[] = [
  "approved",
  "rejected",
  "re-requested",
  "never-reviewed",
];

export interface MatrixNodeDto {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
  derived: boolean;
  tags: string[];
  reviewState: MatrixReviewStateTag;
}

export interface MatrixEdgeDto {
  rowUuid: string;
  columnUuid: string;
}

export interface MatrixResponse {
  rowScope: ReportScopeDto;
  columnScope: ReportScopeDto;
  linkType: GraphLinkTypeDto;
  totalRows: number;
  rowsTruncated: boolean;
  totalColumns: number;
  columnsTruncated: boolean;
  rows: MatrixNodeDto[];
  columns: MatrixNodeDto[];
  edges: MatrixEdgeDto[];
}

/// Query-string parameters the frontend sends on
/// `GET /api/matrix`. `linkType` is required on the wire but
/// modelled optional here so the initial-render state can
/// skip the fetch until a link type is picked.
export interface MatrixQueryParams {
  rowScope?: ReportScopeParam;
  columnScope?: ReportScopeParam;
  linkType?: string;
  includeInactive?: boolean;
  rowTags?: string[];
  columnTags?: string[];
  rowReviewStates?: MatrixReviewStateTag[];
  columnReviewStates?: MatrixReviewStateTag[];
}

// ---- Phase 7c: full-text search DTOs ----

/// Default response size + hard ceiling kept in sync with
/// `DEFAULT_LIMIT` / `MAX_LIMIT` in
/// `backend/reqforge-server/src/search/query.rs`.
export const SEARCH_DEFAULT_LIMIT = 50;
export const SEARCH_MAX_LIMIT = 200;

export type SearchShapeTag = "content" | "blob" | "url";

export interface SearchHit {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: SearchShapeTag;
  reviewState: MatrixReviewStateTag;
  active: boolean;
  score: number;
  /// HTML-escaped body excerpt with `<mark>...</mark>`
  /// markers. The frontend splits on the literal token and
  /// renders spans — never `dangerouslySetInnerHTML`.
  snippet?: string;
}

export interface SearchResponse {
  totalHits: number;
  limit: number;
  offset: number;
  truncated: boolean;
  hits: SearchHit[];
}

/// Three-way has-links toggle surfaced to the UI.
export type SearchHasLinksFilter = "any" | "true" | "false";

export interface SearchQueryParams {
  q?: string;
  scope?: ReportScopeParam;
  shape?: SearchShapeTag[];
  reviewState?: MatrixReviewStateTag[];
  /// Absent means "any"; `true` means ≥ 1 outgoing link;
  /// `false` means zero outgoing links.
  hasLinks?: boolean;
  includeInactive?: boolean;
  limit?: number;
  offset?: number;
}

// ---- Phase 8: doorstop import DTOs ----

export interface DoorstopImportRequest {
  source: string;
}

export interface DoorstopPlanLinkHint {
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
}

export interface DoorstopPlanArtifactLink {
  targetUuid: string;
  linkType: string;
  hint: DoorstopPlanLinkHint;
  unresolved: boolean;
}

export type DoorstopPlanRefDisposition =
  | { kind: "none" }
  | {
      kind: "urlArtifact";
      url: string;
      urlArtifactUuid: string;
      urlArtifactName: string;
    }
  | { kind: "legacy"; value: string };

export interface DoorstopPlanSyntheticReview {
  outcome: string;
  reviewer: string;
  timestamp: string;
  explanation: string;
}

export interface DoorstopPlanArtifact {
  uuid: string;
  name: string;
  originalUid: string;
  title: string;
  body: string;
  active?: boolean | null;
  derived?: boolean | null;
  outlineLevel?: string | null;
  links: DoorstopPlanArtifactLink[];
  tags: string[];
  refDisposition: DoorstopPlanRefDisposition;
  syntheticReview?: DoorstopPlanSyntheticReview | null;
  legacyExtensions: Record<string, unknown>;
  sourcePath: string;
}

export interface DoorstopPlanCollection {
  prefix: string;
  name: string;
  directoryName: string;
  sourceMarkerPath: string;
  importNotes: Record<string, unknown>;
  artifacts: DoorstopPlanArtifact[];
  emptyWarning?: string | null;
}

export interface DoorstopPrefixCollision {
  prefix: string;
  existingCollectionDirectory: string;
  doorstopMarkerPath: string;
}

export interface DoorstopUnresolvedLink {
  sourceUid: string;
  targetUid: string;
  sourceMarkerPath: string;
}

export interface DoorstopImportPlan {
  importRunAt: string;
  collections: DoorstopPlanCollection[];
  prefixCollisions: DoorstopPrefixCollision[];
  unresolvedLinks: DoorstopUnresolvedLink[];
  warnings: string[];
}

export interface DoorstopReportCollection {
  prefix: string;
  name: string;
  directoryName: string;
  artifactCount: number;
  syntheticReviewCount: number;
  legacyPreservedCount: number;
  derivesFromLinkCount: number;
  urlArtifactCount: number;
  sourceMarkerPath: string;
}

export interface DoorstopReportTotals {
  collectionsCreated: number;
  artifactsImported: number;
  derivesFromLinks: number;
  urlArtifacts: number;
  citesLinks: number;
  legacyRefs: number;
  syntheticReviewEntries: number;
  legacyPreservedFields: number;
  unresolvedLinkCount: number;
}

export type DoorstopReportRefDisposition =
  | {
      kind: "urlArtifact";
      sourceUid: string;
      url: string;
      urlArtifactName: string;
    }
  | { kind: "legacy"; sourceUid: string; value: string };

export interface DoorstopImportReport {
  projectSlug: string;
  source: string;
  importRunAt: string;
  collections: DoorstopReportCollection[];
  totals: DoorstopReportTotals;
  refDispositions: DoorstopReportRefDisposition[];
  unresolvedLinks: DoorstopUnresolvedLink[];
  prefixCollisions: DoorstopPrefixCollision[];
  warnings: string[];
}

/// Shape of the 409 body when import refuses on prefix
/// collision — kept as a structured type so the dialog can
/// surface the collisions rather than swallowing the error
/// as an opaque string.
export interface DoorstopImportConflictBody {
  error: string;
  collisions: DoorstopPrefixCollision[];
}

// ---- Phase 7d: browse-by-type DTOs ----

export interface BrowseArtifact {
  uuid: string;
  projectSlug: string;
  collectionPrefix: string;
  artifactName: string;
  title: string;
  shape: ArtifactShape;
  active: boolean;
  reviewState: MatrixReviewStateTag;
  tags: string[];
}

export interface BrowsePane {
  prefix: string;
  name: string;
  /// Present only when the same prefix appears under ≥ 2
  /// distinct Collection names across mounted projects — the
  /// UI surfaces the drift as a warning pill.
  nameVariants?: string[];
  totalArtifacts: number;
  artifacts: BrowseArtifact[];
}

export interface BrowseResponse {
  scope: ReportScopeDto;
  totalPanes: number;
  totalArtifacts: number;
  panes: BrowsePane[];
}

export interface BrowseQueryParams {
  scope?: ReportScopeParam;
  tags?: string[];
  reviewState?: MatrixReviewStateTag[];
  includeInactive?: boolean;
}

// --- Phase 10a + 10b: LLM adapter layer ------------------------------------

export type LlmHealthState =
  | { kind: "healthy" }
  | { kind: "transient-degraded"; retryAfterSecs: number }
  | { kind: "hard-disabled" };

export interface LlmProviderEntry {
  index: number;
  provider: string;
  model: string;
  endpoint: string;
  isLocal: boolean;
  requiresPrivacyAck: boolean;
  apiKeyAvailable: boolean;
  /// Phase 13: `true` iff the slot is enabled in the System
  /// config. Disabled slots are skipped by the fallback chain.
  enabled: boolean;
  health: LlmHealthState;
}

/// Phase 13: body for POST/PUT /api/llm/providers.
export interface ProviderCrudRequest {
  provider: string;
  model: string;
  endpoint?: string;
  apiKey?: string;
  enabled?: boolean;
  /// POST-only: insert at this index instead of appending.
  position?: number;
}

/// Phase 13: body for PATCH /api/llm/providers/{index}.
export interface ProviderPatchRequest {
  enabled?: boolean;
  position?: number;
}

export interface LlmProvidersResponse {
  providers: LlmProviderEntry[];
}

export interface LlmRetestResponse {
  ok: boolean;
  error?: string;
  health: LlmHealthState;
}

export interface LlmAcknowledgePrivacyResponse {
  acknowledged: boolean;
}

export interface RenameSuggestion {
  name: string;
  rationale: string;
}

export type RenameSuggestionsResponse =
  | {
      kind: "ok";
      suggestions: RenameSuggestion[];
      servedByIndex: number;
      servedBy: string;
    }
  | { kind: "privacyAckRequired"; indices: number[] }
  | { kind: "noProviders" };

export type BulkRenameSuggestionEntry =
  | {
      kind: "ok";
      uuid: string;
      suggestions: RenameSuggestion[];
      servedByIndex: number;
      servedBy: string;
    }
  | { kind: "error"; uuid: string; error: string }
  | { kind: "privacyAckRequired"; uuid: string; indices: number[] }
  | { kind: "notFound"; uuid: string };

export interface BulkRenameSuggestionsResponse {
  results: BulkRenameSuggestionEntry[];
}

// ---------------------------------------------------------------------------
// Phase 12a: LLM-assisted link suggestion.

export interface LinkSuggestion {
  id: string;
  from: string;
  to: string;
  linkType: string;
  confidence: number;
  rationale: string;
}

export interface LinkSuggestionDeclineRecord extends LinkSuggestion {
  declinedAt: string;
}

export type AnalyzeLinkSuggestionsResponse =
  | {
      kind: "ok";
      suggestions: LinkSuggestion[];
      servedByIndex: number;
      servedBy: string;
    }
  | { kind: "privacyAckRequired"; indices: number[] }
  | { kind: "noProviders" };

export interface ListLinkSuggestionsResponse {
  suggestions: LinkSuggestion[];
}

export interface ListDeclinedLinkSuggestionsResponse {
  declined: LinkSuggestionDeclineRecord[];
}

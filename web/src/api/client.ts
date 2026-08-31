// Typed fetch wrappers over the ReqForge read-only HTTP API.
//
// Base URL resolution: if VITE_REQFORGE_API_BASE is set at build time,
// it prefixes every request; otherwise requests use relative paths so
// the Vite dev proxy (or the production container's same-origin
// serving) handles routing.

import type {
  AdoptOrphanBlobRequest,
  AnalyzeLinkSuggestionsResponse,
  ProviderCrudRequest,
  ProviderPatchRequest,
  BrowseQueryParams,
  BrowseResponse,
  BulkRenameSuggestionsResponse,
  DoorstopImportPlan,
  DoorstopImportReport,
  DoorstopImportRequest,
  ArtifactDetail,
  ListDeclinedLinkSuggestionsResponse,
  ListLinkSuggestionsResponse,
  ArtifactDiffResponse,
  ArtifactHistoryResponse,
  ArtifactSearchResult,
  BulkCheckUrlsResponse,
  CheckUrlResponse,
  CollectionSummary,
  ArtifactListing,
  CreateArtifactRequest,
  CreateCollectionRequest,
  CreateReviewRequest,
  CreateUrlArtifactRequest,
  GraphQueryParams,
  GraphResponse,
  HealthResponse,
  IncomingLinkEntry,
  LastApprovalSnapshot,
  LinkType,
  LlmAcknowledgePrivacyResponse,
  LlmProvidersResponse,
  LlmRetestResponse,
  MigrateSchemaRequest,
  MigrateSchemaResponse,
  SampleContentResponse,
  MatrixQueryParams,
  MatrixResponse,
  ProjectDetail,
  ProjectSummary,
  ReadinessResponse,
  RenameArtifactRequest,
  RenameSuggestionsResponse,
  ReportKind,
  ReportResponse,
  ReportScopeParam,
  ReviewQueueFilters,
  ReviewQueueResponse,
  ReviewerIdentityOptions,
  SavedReportConfig,
  SearchQueryParams,
  SearchResponse,
  UpdateArtifactRequest,
} from "./types";

const BASE =
  (import.meta.env.VITE_REQFORGE_API_BASE as string | undefined) ?? "";

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly url: string,
    /// Parsed JSON body when the server sent one on the
    /// error response. Phase 8's prefix-collision 409 carries
    /// a structured `{error, collisions}` payload the wizard
    /// renders — callers that need structured error data
    /// inspect this field rather than parsing the string
    /// message a second time.
    public readonly body?: unknown,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string): Promise<T> {
  return send<T>(path, "GET");
}

async function send<T>(
  path: string,
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE",
  body?: unknown,
): Promise<T> {
  const url = `${BASE}${path}`;
  const init: RequestInit = {
    method,
    headers: { Accept: "application/json" },
  };
  if (body instanceof FormData) {
    // Browsers set the multipart boundary + Content-Type header
    // automatically when `body` is FormData; setting it manually
    // breaks the framing, so we deliberately skip the JSON header.
    init.body = body;
  } else if (body !== undefined) {
    (init.headers as Record<string, string>)["Content-Type"] =
      "application/json";
    init.body = JSON.stringify(body);
  }
  const response = await fetch(url, init);
  if (!response.ok) {
    let detail = "";
    let parsedBody: unknown = undefined;
    try {
      parsedBody = await response.json();
      const errBody = parsedBody as { error?: string };
      detail = errBody.error ? `: ${errBody.error}` : "";
    } catch {
      // Non-JSON error body — keep detail empty.
    }
    throw new ApiError(
      `${response.status} ${response.statusText}${detail}`,
      response.status,
      url,
      parsedBody,
    );
  }
  // 204 No Content responses still need to satisfy the Promise<T>
  // signature; cast `null` through undefined-as-T for void returns.
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const api = {
  health: () => request<HealthResponse>("/healthz"),
  readiness: () => request<ReadinessResponse>("/readyz"),
  projects: () => request<ProjectSummary[]>("/api/projects"),
  project: (slug: string) =>
    request<ProjectDetail>(`/api/projects/${encodeURIComponent(slug)}`),
  collections: (slug: string) =>
    request<CollectionSummary[]>(
      `/api/projects/${encodeURIComponent(slug)}/collections`,
    ),
  collection: (slug: string, prefix: string) =>
    request<CollectionSummary>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}`,
    ),
  artifacts: (slug: string, prefix: string) =>
    request<ArtifactListing[]>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/artifacts`,
    ),
  artifact: (uuid: string) =>
    request<ArtifactDetail>(`/api/artifacts/${encodeURIComponent(uuid)}`),
  incomingLinks: (uuid: string) =>
    request<IncomingLinkEntry[]>(
      `/api/artifacts/${encodeURIComponent(uuid)}/incoming-links`,
    ),

  createArtifact: (slug: string, prefix: string, req: CreateArtifactRequest) =>
    send<ArtifactDetail>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/artifacts`,
      "POST",
      req,
    ),
  updateArtifact: (uuid: string, req: UpdateArtifactRequest) =>
    send<ArtifactDetail>(
      `/api/artifacts/${encodeURIComponent(uuid)}`,
      "PUT",
      req,
    ),
  renameArtifact: (uuid: string, req: RenameArtifactRequest) =>
    send<ArtifactDetail>(
      `/api/artifacts/${encodeURIComponent(uuid)}`,
      "PATCH",
      req,
    ),
  deleteArtifact: (uuid: string) =>
    send<void>(`/api/artifacts/${encodeURIComponent(uuid)}`, "DELETE"),

  createCollection: (slug: string, req: CreateCollectionRequest) =>
    send<CollectionSummary>(
      `/api/projects/${encodeURIComponent(slug)}/collections`,
      "POST",
      req,
    ),
  deleteCollection: (slug: string, prefix: string) =>
    send<void>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}`,
      "DELETE",
    ),
  wipeProjectArtifacts: (slug: string, opts: { deinit?: boolean } = {}) => {
    const qs = opts.deinit ? "?deinit=true" : "";
    return send<void>(
      `/api/projects/${encodeURIComponent(slug)}/artifacts${qs}`,
      "DELETE",
    );
  },

  linkTypes: () => request<LinkType[]>("/api/link-types"),
  searchArtifacts: (q: string, exclude?: string, limit = 25) => {
    const params = new URLSearchParams({ q, limit: String(limit) });
    if (exclude) {
      params.set("exclude", exclude);
    }
    return request<ArtifactSearchResult[]>(
      `/api/artifacts/search?${params.toString()}`,
    );
  },

  reviewers: (projectSlug?: string) => {
    const qs = projectSlug
      ? `?projectSlug=${encodeURIComponent(projectSlug)}`
      : "";
    return request<ReviewerIdentityOptions>(`/api/reviewers${qs}`);
  },
  createReview: (uuid: string, req: CreateReviewRequest) =>
    send<ArtifactDetail>(
      `/api/artifacts/${encodeURIComponent(uuid)}/reviews`,
      "POST",
      req,
    ),
  reviewQueue: (filters: ReviewQueueFilters = {}) => {
    const params = new URLSearchParams();
    if (filters.projectSlug) params.set("projectSlug", filters.projectSlug);
    if (filters.collectionPrefix)
      params.set("collectionPrefix", filters.collectionPrefix);
    if (filters.shape) params.set("shape", filters.shape);
    if (filters.tag) params.set("tag", filters.tag);
    if (filters.reviewer) params.set("reviewer", filters.reviewer);
    if (filters.order) params.set("order", filters.order);
    const qs = params.toString();
    return request<ReviewQueueResponse>(
      `/api/reviews/queue${qs ? `?${qs}` : ""}`,
    );
  },
  lastApprovalSnapshot: (uuid: string) =>
    request<LastApprovalSnapshot>(
      `/api/artifacts/${encodeURIComponent(uuid)}/reviews/last-approval-snapshot`,
    ),

  // Phase 5c: non-content artifact shapes.
  createBlobArtifact: (
    slug: string,
    prefix: string,
    form: FormData,
  ): Promise<ArtifactDetail> =>
    send<ArtifactDetail>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/artifacts/blob`,
      "POST",
      form,
    ),
  replaceBlob: (uuid: string, form: FormData): Promise<ArtifactDetail> =>
    send<ArtifactDetail>(
      `/api/artifacts/${encodeURIComponent(uuid)}/blob`,
      "PUT",
      form,
    ),
  createUrlArtifact: (
    slug: string,
    prefix: string,
    req: CreateUrlArtifactRequest,
  ) =>
    send<ArtifactDetail>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/artifacts/url`,
      "POST",
      req,
    ),
  checkUrl: (uuid: string) =>
    send<CheckUrlResponse>(
      `/api/artifacts/${encodeURIComponent(uuid)}/check-url`,
      "POST",
    ),
  bulkCheckUrls: (slug: string, prefix: string, uuids?: string[]) =>
    send<BulkCheckUrlsResponse>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/check-urls`,
      "POST",
      uuids ? { uuids } : null,
    ),

  // Phase 5d: history + shape-aware diff.
  artifactHistory: (uuid: string) =>
    request<ArtifactHistoryResponse>(
      `/api/artifacts/${encodeURIComponent(uuid)}/history`,
    ),
  artifactDiff: (uuid: string, from: string, to?: string) => {
    const params = new URLSearchParams({ from });
    if (to) params.set("to", to);
    return request<ArtifactDiffResponse>(
      `/api/artifacts/${encodeURIComponent(uuid)}/diff?${params.toString()}`,
    );
  },

  // Phase 6a: reports + saved-config.
  report: (
    kind: ReportKind,
    scope?: ReportScopeParam,
    includeInactive?: boolean,
    /// Per-kind options the unified endpoint accepts as extra
    /// query params. 6a.3 uses `coveringLinkTypes` (coverage
    /// matrix) + `seed`/`direction` (impact analysis).
    extra?: Record<string, string | undefined>,
  ) => {
    const params = new URLSearchParams();
    if (scope && scope !== "system") params.set("scope", scope);
    if (includeInactive) params.set("includeInactive", "true");
    if (extra) {
      for (const [key, value] of Object.entries(extra)) {
        if (value !== undefined && value !== "") params.set(key, value);
      }
    }
    const qs = params.toString();
    return request<ReportResponse>(
      `/api/reports/${encodeURIComponent(kind)}${qs ? `?${qs}` : ""}`,
    );
  },
  readReportConfig: (kind: ReportKind) =>
    request<SavedReportConfig>(
      `/api/reports/${encodeURIComponent(kind)}/config`,
    ),
  writeReportConfig: (kind: ReportKind, config: SavedReportConfig) =>
    send<void>(
      `/api/reports/${encodeURIComponent(kind)}/config`,
      "PUT",
      config,
    ),
  clearReportConfig: (kind: ReportKind) =>
    send<void>(`/api/reports/${encodeURIComponent(kind)}/config`, "DELETE"),
  adoptOrphanBlob: (
    slug: string,
    prefix: string,
    req: AdoptOrphanBlobRequest,
  ) =>
    send<ArtifactDetail>(
      `/api/projects/${encodeURIComponent(slug)}/collections/${encodeURIComponent(prefix)}/artifacts/blob/adopt`,
      "POST",
      req,
    ),

  // Phase 7a: graph canvas.
  graph: (params: GraphQueryParams) => {
    const qs = new URLSearchParams();
    if (params.scope && params.scope !== "system")
      qs.set("scope", params.scope);
    if (params.includeInactive) qs.set("includeInactive", "true");
    if (params.linkTypes && params.linkTypes.length > 0)
      qs.set("linkTypes", params.linkTypes.join(","));
    if (params.tags && params.tags.length > 0)
      qs.set("tags", params.tags.join(","));
    const suffix = qs.toString();
    return request<GraphResponse>(`/api/graph${suffix ? `?${suffix}` : ""}`);
  },

  // Phase 7b: matrix link view.
  matrix: (params: MatrixQueryParams) => {
    const qs = new URLSearchParams();
    if (params.rowScope && params.rowScope !== "system")
      qs.set("rowScope", params.rowScope);
    if (params.columnScope && params.columnScope !== "system")
      qs.set("columnScope", params.columnScope);
    if (params.linkType) qs.set("linkType", params.linkType);
    if (params.includeInactive) qs.set("includeInactive", "true");
    if (params.rowTags && params.rowTags.length > 0)
      qs.set("rowTags", params.rowTags.join(","));
    if (params.columnTags && params.columnTags.length > 0)
      qs.set("columnTags", params.columnTags.join(","));
    if (params.rowReviewStates && params.rowReviewStates.length > 0)
      qs.set("rowReviewStates", params.rowReviewStates.join(","));
    if (params.columnReviewStates && params.columnReviewStates.length > 0)
      qs.set("columnReviewStates", params.columnReviewStates.join(","));
    const suffix = qs.toString();
    return request<MatrixResponse>(`/api/matrix${suffix ? `?${suffix}` : ""}`);
  },

  // Phase 7c: full-text search.
  search: (params: SearchQueryParams) => {
    const qs = new URLSearchParams();
    if (params.q && params.q.trim()) qs.set("q", params.q);
    if (params.scope && params.scope !== "system")
      qs.set("scope", params.scope);
    if (params.shape && params.shape.length > 0)
      qs.set("shape", params.shape.join(","));
    if (params.reviewState && params.reviewState.length > 0)
      qs.set("reviewState", params.reviewState.join(","));
    if (params.hasLinks !== undefined)
      qs.set("hasLinks", params.hasLinks ? "true" : "false");
    if (params.includeInactive) qs.set("includeInactive", "true");
    if (params.limit !== undefined) qs.set("limit", String(params.limit));
    if (params.offset !== undefined) qs.set("offset", String(params.offset));
    const suffix = qs.toString();
    return request<SearchResponse>(`/api/search${suffix ? `?${suffix}` : ""}`);
  },

  // Phase 8: doorstop import.
  doorstopPreview: (slug: string, req: DoorstopImportRequest) =>
    send<DoorstopImportPlan>(
      `/api/projects/${encodeURIComponent(slug)}/doorstop/preview`,
      "POST",
      req,
    ),
  doorstopImport: (slug: string, req: DoorstopImportRequest) =>
    send<DoorstopImportReport>(
      `/api/projects/${encodeURIComponent(slug)}/doorstop/import`,
      "POST",
      req,
    ),
  doorstopReport: (slug: string) =>
    request<DoorstopImportReport>(
      `/api/projects/${encodeURIComponent(slug)}/doorstop/report`,
    ),

  // Phase 7d: browse-by-type.
  browse: (params: BrowseQueryParams) => {
    const qs = new URLSearchParams();
    if (params.scope && params.scope !== "system")
      qs.set("scope", params.scope);
    if (params.tags && params.tags.length > 0)
      qs.set("tags", params.tags.join(","));
    if (params.reviewState && params.reviewState.length > 0)
      qs.set("reviewState", params.reviewState.join(","));
    if (params.includeInactive) qs.set("includeInactive", "true");
    const suffix = qs.toString();
    return request<BrowseResponse>(`/api/browse${suffix ? `?${suffix}` : ""}`);
  },

  // Phase 10a + 10b: LLM adapter layer.
  llmProviders: () => request<LlmProvidersResponse>("/api/llm/providers"),
  // Phase 13: in-app LLM provider CRUD.
  addLlmProvider: (req: ProviderCrudRequest) =>
    send<void>("/api/llm/providers", "POST", req),
  replaceLlmProvider: (index: number, req: ProviderCrudRequest) =>
    send<void>(`/api/llm/providers/${index}`, "PUT", req),
  deleteLlmProvider: (index: number) =>
    send<void>(`/api/llm/providers/${index}`, "DELETE"),
  patchLlmProvider: (index: number, req: ProviderPatchRequest) =>
    send<void>(`/api/llm/providers/${index}`, "PATCH", req),
  retestLlmProvider: (index: number) =>
    send<LlmRetestResponse>(
      `/api/llm/providers/${encodeURIComponent(index)}/retest`,
      "POST",
    ),
  acknowledgeLlmPrivacy: (index: number) =>
    send<LlmAcknowledgePrivacyResponse>(
      `/api/llm/providers/${encodeURIComponent(index)}/acknowledge-privacy`,
      "POST",
    ),
  renameSuggestions: (uuid: string) =>
    send<RenameSuggestionsResponse>(
      `/api/artifacts/${encodeURIComponent(uuid)}/rename-suggestions`,
      "POST",
      {},
    ),
  bulkRenameSuggestions: (slug: string, uuids: string[]) =>
    send<BulkRenameSuggestionsResponse>(
      `/api/projects/${encodeURIComponent(slug)}/rename-suggestions/bulk`,
      "POST",
      { uuids },
    ),

  // Phase 11a: per-project schema migration.
  migrateProjectSchema: (slug: string, req: MigrateSchemaRequest) =>
    send<MigrateSchemaResponse>(
      `/api/projects/${encodeURIComponent(slug)}/migrate-schema`,
      "POST",
      req,
    ),

  // Phase 11b: sample-content onboarding seed.
  createSampleContent: (slug: string) =>
    send<SampleContentResponse>(
      `/api/projects/${encodeURIComponent(slug)}/sample-content`,
      "POST",
      {},
    ),

  // Phase 12a: LLM-assisted link suggestion.
  analyzeLinkSuggestions: (slug: string) =>
    send<AnalyzeLinkSuggestionsResponse>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links/analyze`,
      "POST",
      {},
    ),
  listPendingLinkSuggestions: (slug: string) =>
    request<ListLinkSuggestionsResponse>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links`,
    ),
  listDeclinedLinkSuggestions: (slug: string) =>
    request<ListDeclinedLinkSuggestionsResponse>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links/declined`,
    ),
  acceptLinkSuggestion: (slug: string, id: string) =>
    send<void>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links/${encodeURIComponent(id)}/accept`,
      "POST",
      {},
    ),
  rejectLinkSuggestion: (slug: string, id: string) =>
    send<void>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links/${encodeURIComponent(id)}/reject`,
      "POST",
      {},
    ),
  reinstateLinkSuggestion: (slug: string, id: string) =>
    send<void>(
      `/api/projects/${encodeURIComponent(slug)}/suggestions/links/${encodeURIComponent(id)}/reinstate`,
      "POST",
      {},
    ),
};

// React Query hooks wrapping the typed fetch client. One hook per
// endpoint keeps call-sites concise and the cache keys consistent.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import type {
  AdoptOrphanBlobRequest,
  BrowseQueryParams,
  CreateArtifactRequest,
  DoorstopImportRequest,
  CreateCollectionRequest,
  CreateReviewRequest,
  CreateUrlArtifactRequest,
  GraphQueryParams,
  MatrixQueryParams,
  ProviderCrudRequest,
  ProviderPatchRequest,
  RenameArtifactRequest,
  ReportKind,
  ReportScopeParam,
  ReviewQueueFilters,
  SavedReportConfig,
  SearchQueryParams,
  UpdateArtifactRequest,
} from "./types";

// Phase 5d cache keys for history + diff. Kept within the
// general artifact namespace so the SSE invalidateAll drop on
// write propagates correctly.
export const queryKeys = {
  artifactHistory: (uuid: string) => ["artifacts", uuid, "history"] as const,
  artifactDiff: (uuid: string, from: string, to: string | undefined) =>
    ["artifacts", uuid, "diff", from, to ?? "current"] as const,
  report: (
    kind: ReportKind,
    scope: ReportScopeParam | undefined,
    includeInactive: boolean,
  ) => ["reports", kind, scope ?? "system", includeInactive] as const,
  reportConfig: (kind: ReportKind) => ["reports", kind, "config"] as const,
  graph: (params: GraphQueryParams) =>
    [
      "graph",
      params.scope ?? "system",
      params.includeInactive ?? false,
      (params.linkTypes ?? []).slice().sort().join(","),
      (params.tags ?? []).slice().sort().join(","),
    ] as const,
  matrix: (params: MatrixQueryParams) =>
    [
      "matrix",
      params.rowScope ?? "system",
      params.columnScope ?? "system",
      params.linkType ?? "",
      params.includeInactive ?? false,
      (params.rowTags ?? []).slice().sort().join(","),
      (params.columnTags ?? []).slice().sort().join(","),
      (params.rowReviewStates ?? []).slice().sort().join(","),
      (params.columnReviewStates ?? []).slice().sort().join(","),
    ] as const,
  search: (params: SearchQueryParams) =>
    [
      "search",
      params.q ?? "",
      params.scope ?? "system",
      (params.shape ?? []).slice().sort().join(","),
      (params.reviewState ?? []).slice().sort().join(","),
      params.hasLinks ?? null,
      params.includeInactive ?? false,
      params.limit ?? null,
      params.offset ?? 0,
    ] as const,
  browse: (params: BrowseQueryParams) =>
    [
      "browse",
      params.scope ?? "system",
      (params.tags ?? []).slice().sort().join(","),
      (params.reviewState ?? []).slice().sort().join(","),
      params.includeInactive ?? false,
    ] as const,
  health: ["health"] as const,
  readiness: ["readiness"] as const,
  projects: ["projects"] as const,
  project: (slug: string) => ["projects", slug] as const,
  collections: (slug: string) => ["projects", slug, "collections"] as const,
  collection: (slug: string, prefix: string) =>
    ["projects", slug, "collections", prefix] as const,
  artifacts: (slug: string, prefix: string) =>
    ["projects", slug, "collections", prefix, "artifacts"] as const,
  artifact: (uuid: string) => ["artifacts", uuid] as const,
  linkTypes: ["link-types"] as const,
  artifactSearch: (q: string, exclude?: string) =>
    ["artifact-search", q, exclude ?? null] as const,
  reviewers: (projectSlug?: string) =>
    ["reviewers", projectSlug ?? null] as const,
  reviewQueue: (filters: ReviewQueueFilters) =>
    ["reviews", "queue", filters] as const,
  lastApprovalSnapshot: (uuid: string) =>
    ["artifacts", uuid, "last-approval-snapshot"] as const,
  pendingLinkSuggestions: (slug: string) =>
    ["projects", slug, "suggestions", "links", "pending"] as const,
  declinedLinkSuggestions: (slug: string) =>
    ["projects", slug, "suggestions", "links", "declined"] as const,
};

export function useHealth() {
  return useQuery({
    queryKey: queryKeys.health,
    queryFn: api.health,
  });
}

export function useReadiness() {
  return useQuery({
    queryKey: queryKeys.readiness,
    queryFn: api.readiness,
    refetchInterval: (query) => (query.state.data?.ready ? false : 1000),
  });
}

export function useProjects() {
  return useQuery({
    queryKey: queryKeys.projects,
    queryFn: api.projects,
  });
}

export function useProject(slug: string | undefined) {
  return useQuery({
    queryKey: slug ? queryKeys.project(slug) : queryKeys.projects,
    queryFn: () => api.project(slug!),
    enabled: Boolean(slug),
  });
}

export function useCollections(slug: string | undefined) {
  return useQuery({
    queryKey: slug ? queryKeys.collections(slug) : queryKeys.projects,
    queryFn: () => api.collections(slug!),
    enabled: Boolean(slug),
  });
}

export function useCollection(
  slug: string | undefined,
  prefix: string | undefined,
) {
  return useQuery({
    queryKey:
      slug && prefix ? queryKeys.collection(slug, prefix) : queryKeys.projects,
    queryFn: () => api.collection(slug!, prefix!),
    enabled: Boolean(slug && prefix),
  });
}

export function useArtifacts(
  slug: string | undefined,
  prefix: string | undefined,
) {
  return useQuery({
    queryKey:
      slug && prefix ? queryKeys.artifacts(slug, prefix) : queryKeys.projects,
    queryFn: () => api.artifacts(slug!, prefix!),
    enabled: Boolean(slug && prefix),
  });
}

export function useArtifact(uuid: string | undefined) {
  return useQuery({
    queryKey: uuid ? queryKeys.artifact(uuid) : queryKeys.projects,
    queryFn: () => api.artifact(uuid!),
    enabled: Boolean(uuid),
  });
}

export function useIncomingLinks(uuid: string | undefined) {
  return useQuery({
    queryKey: uuid ? ["artifacts", uuid, "incoming-links"] : queryKeys.projects,
    queryFn: () => api.incomingLinks(uuid!),
    enabled: Boolean(uuid),
  });
}

/// The effective link-type catalog is cached aggressively because
/// it only changes when the System config or the backend version
/// does; SSE change events invalidate via invalidateAll.
export function useLinkTypes() {
  return useQuery({
    queryKey: queryKeys.linkTypes,
    queryFn: api.linkTypes,
    staleTime: 60_000,
  });
}

/// Back-end for the picker's target field. `q` should already be
/// debounced by the caller — the hook just disables itself on an
/// empty query so the cache doesn't fill with noise.
export function useArtifactSearch(q: string, exclude?: string) {
  return useQuery({
    queryKey: queryKeys.artifactSearch(q, exclude),
    queryFn: () => api.searchArtifacts(q, exclude),
    enabled: q.length > 0,
  });
}

// ---- Mutations ----

/// After any write, we invalidate everything — the SSE client
/// (commit 14) does the same on incoming `change` events so the
/// two mechanisms converge cleanly.
function invalidateAll(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries();
}

export function useCreateArtifact(slug: string, prefix: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateArtifactRequest) =>
      api.createArtifact(slug, prefix, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useUpdateArtifact(uuid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: UpdateArtifactRequest) => api.updateArtifact(uuid, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useRenameArtifact(uuid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: RenameArtifactRequest) => api.renameArtifact(uuid, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useDeleteArtifact() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (uuid: string) => api.deleteArtifact(uuid),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useCreateCollection(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateCollectionRequest) =>
      api.createCollection(slug, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useDeleteCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { slug: string; prefix: string }) =>
      api.deleteCollection(args.slug, args.prefix),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useWipeProjectArtifacts(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (opts: { deinit?: boolean } = {}) =>
      api.wipeProjectArtifacts(slug, opts),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useReviewers(projectSlug?: string) {
  return useQuery({
    queryKey: queryKeys.reviewers(projectSlug),
    queryFn: () => api.reviewers(projectSlug),
  });
}

export function useReviewQueue(filters: ReviewQueueFilters = {}) {
  return useQuery({
    queryKey: queryKeys.reviewQueue(filters),
    queryFn: () => api.reviewQueue(filters),
  });
}

export function useSubmitReview(uuid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateReviewRequest) => api.createReview(uuid, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useLastApprovalSnapshot(uuid: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.lastApprovalSnapshot(uuid),
    queryFn: () => api.lastApprovalSnapshot(uuid),
    enabled,
    retry: false,
  });
}

// ---- Phase 5c: blob + URL shape mutations ----

export function useCreateBlobArtifact(slug: string, prefix: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (form: FormData) => api.createBlobArtifact(slug, prefix, form),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useReplaceBlob(uuid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (form: FormData) => api.replaceBlob(uuid, form),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useCreateUrlArtifact(slug: string, prefix: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateUrlArtifactRequest) =>
      api.createUrlArtifact(slug, prefix, req),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useCheckUrl(uuid: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.checkUrl(uuid),
    onSuccess: () => invalidateAll(qc),
  });
}

export function useBulkCheckUrls(slug: string, prefix: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (uuids?: string[]) => api.bulkCheckUrls(slug, prefix, uuids),
    onSuccess: () => invalidateAll(qc),
  });
}

// Phase 5d: history + diff queries.
export function useArtifactHistory(uuid: string | undefined) {
  return useQuery({
    queryKey: uuid ? queryKeys.artifactHistory(uuid) : queryKeys.projects,
    queryFn: () => api.artifactHistory(uuid!),
    enabled: Boolean(uuid),
  });
}

export function useArtifactDiff(
  uuid: string | undefined,
  from: string | undefined,
  to: string | undefined,
) {
  return useQuery({
    queryKey:
      uuid && from
        ? queryKeys.artifactDiff(uuid, from, to)
        : queryKeys.projects,
    queryFn: () => api.artifactDiff(uuid!, from!, to),
    enabled: Boolean(uuid && from),
  });
}

// Phase 6a: reports.
export function useReport(
  kind: ReportKind,
  scope: ReportScopeParam | undefined,
  includeInactive: boolean,
  /// Optional per-kind extra query params. The key is stable
  /// through the cache key so flipping, e.g., impact-analysis
  /// direction re-fetches.
  extra?: Record<string, string | undefined>,
) {
  return useQuery({
    queryKey: [
      ...queryKeys.report(kind, scope, includeInactive),
      extra ?? null,
    ] as const,
    queryFn: () => api.report(kind, scope, includeInactive, extra),
  });
}

export function useReportConfig(kind: ReportKind) {
  return useQuery({
    queryKey: queryKeys.reportConfig(kind),
    queryFn: () => api.readReportConfig(kind),
    staleTime: 5_000,
  });
}

export function useSaveReportConfig(kind: ReportKind) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (config: SavedReportConfig) =>
      api.writeReportConfig(kind, config),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: queryKeys.reportConfig(kind) }),
  });
}

export function useClearReportConfig(kind: ReportKind) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.clearReportConfig(kind),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: queryKeys.reportConfig(kind) }),
  });
}

// Phase 7a: graph canvas.
export function useGraph(params: GraphQueryParams) {
  return useQuery({
    queryKey: queryKeys.graph(params),
    queryFn: () => api.graph(params),
  });
}

// Phase 7b: matrix link view. The backend treats `linkType` as
// required, so we gate the query on it rather than firing a 400
// on initial render before the user has picked one.
export function useMatrix(params: MatrixQueryParams) {
  return useQuery({
    queryKey: queryKeys.matrix(params),
    queryFn: () => api.matrix(params),
    enabled: Boolean(params.linkType),
  });
}

// Phase 7c: full-text search. Empty `q` runs a match-all on
// the backend so pure-filter searches work — always enabled.
export function useSearch(params: SearchQueryParams) {
  return useQuery({
    queryKey: queryKeys.search(params),
    queryFn: () => api.search(params),
  });
}

// Phase 8: doorstop import. Preview + import both mutate from
// the frontend's perspective (the import writes files; the
// preview doesn't but runs a fresh walk each time, so treating
// it as a mutation keeps cache semantics straightforward).
// After a successful import we invalidate all caches so the
// newly-imported artifacts surface immediately in every view.
export function useDoorstopPreview(slug: string) {
  return useMutation({
    mutationFn: (req: DoorstopImportRequest) => api.doorstopPreview(slug, req),
  });
}

export function useDoorstopImport(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: DoorstopImportRequest) => api.doorstopImport(slug, req),
    onSuccess: () => invalidateAll(qc),
  });
}

// Phase 7d: browse-by-type.
export function useBrowse(params: BrowseQueryParams) {
  return useQuery({
    queryKey: queryKeys.browse(params),
    queryFn: () => api.browse(params),
  });
}

export function useAdoptOrphanBlob(slug: string, prefix: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AdoptOrphanBlobRequest) =>
      api.adoptOrphanBlob(slug, prefix, req),
    onSuccess: () => invalidateAll(qc),
  });
}

// --- Phase 10a + 10b: LLM adapter layer + rename suggestions -------------

const LLM_PROVIDERS_KEY = ["llm", "providers"] as const;

export function useAddLlmProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ProviderCrudRequest) => api.addLlmProvider(req),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function useReplaceLlmProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { index: number; req: ProviderCrudRequest }) =>
      api.replaceLlmProvider(args.index, args.req),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function useDeleteLlmProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (index: number) => api.deleteLlmProvider(index),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function usePatchLlmProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { index: number; req: ProviderPatchRequest }) =>
      api.patchLlmProvider(args.index, args.req),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function useLlmProviders() {
  return useQuery({
    queryKey: LLM_PROVIDERS_KEY,
    queryFn: () => api.llmProviders(),
    // The providers list is cheap to fetch and rarely
    // changes, but retest + ack invalidate it, so keep the
    // default staleTime so invalidations surface promptly.
  });
}

export function useRetestLlmProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (index: number) => api.retestLlmProvider(index),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function useAcknowledgeLlmPrivacy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (index: number) => api.acknowledgeLlmPrivacy(index),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LLM_PROVIDERS_KEY });
    },
  });
}

export function useRenameSuggestions() {
  return useMutation({
    mutationFn: (uuid: string) => api.renameSuggestions(uuid),
  });
}

export function useBulkRenameSuggestions() {
  return useMutation({
    mutationFn: (args: { slug: string; uuids: string[] }) =>
      api.bulkRenameSuggestions(args.slug, args.uuids),
  });
}

// --- Phase 11a: schema migration -----------------------------------------

export function useMigrateProjectSchema(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { force?: boolean } = {}) =>
      api.migrateProjectSchema(slug, req),
    // Any migration run — even a no-op — should refresh the
    // project caches because a successful rewrite bumped file
    // contents; SchemaTooNew diagnostics may also have cleared.
    onSuccess: () => invalidateAll(qc),
  });
}

// --- Phase 11b: sample-content onboarding --------------------------------

export function useCreateSampleContent(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.createSampleContent(slug),
    // Seeding writes a full project worth of collections +
    // artifacts; every cached listing is stale afterwards.
    onSuccess: () => invalidateAll(qc),
  });
}

// --- Phase 12a: LLM-assisted link suggestion ----------------------------

export function usePendingLinkSuggestions(slug: string) {
  return useQuery({
    queryKey: queryKeys.pendingLinkSuggestions(slug),
    queryFn: () => api.listPendingLinkSuggestions(slug),
  });
}

export function useDeclinedLinkSuggestions(slug: string) {
  return useQuery({
    queryKey: queryKeys.declinedLinkSuggestions(slug),
    queryFn: () => api.listDeclinedLinkSuggestions(slug),
  });
}

export function useAnalyzeLinkSuggestions(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.analyzeLinkSuggestions(slug),
    // A successful run rewrites pending.json; refresh that
    // query so the inbox reflects the new proposals.
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: queryKeys.pendingLinkSuggestions(slug),
      });
    },
  });
}

export function useAcceptLinkSuggestion(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.acceptLinkSuggestion(slug, id),
    // Accept writes a real link AND drops from pending — every
    // cached project view is potentially stale.
    onSuccess: () => invalidateAll(qc),
  });
}

export function useRejectLinkSuggestion(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.rejectLinkSuggestion(slug, id),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: queryKeys.pendingLinkSuggestions(slug),
      });
      qc.invalidateQueries({
        queryKey: queryKeys.declinedLinkSuggestions(slug),
      });
    },
  });
}

export function useReinstateLinkSuggestion(slug: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.reinstateLinkSuggestion(slug, id),
    // Reinstate writes a real link AND drops from declined.
    onSuccess: () => invalidateAll(qc),
  });
}

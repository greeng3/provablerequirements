// Mirrors the `GET /api/requirements` payload from `src/server.rs` (Backlog) — the
// serde field names (snake_case coverage counts, kebab-case enum values) are the contract.

export type Classification =
  | "formalizable-now"
  | "falsifiable-only"
  | "stays-prose";

export type Formalization = "none" | "drafting" | "admitted";

// Mirrors `triage::Origin` from src/triage.rs (#172): what produced a classification. A bucket a
// classifier judged and one seeded because nothing could are different facts — and "unrecorded" is
// deliberately neither, since an entry written before the field existed may have been either and
// guessing is the thing the enum exists to stop.
export type Origin = "classified" | "seeded" | "operator" | "unrecorded";

export interface Coverage {
  discovered: number;
  untriaged: number;
  formalizable_now: number;
  falsifiable_only: number;
  stays_prose: number;
  drafting: number;
  formalized: number;
  verified: number;
  stale: number;
}

// Mirrors `verdict_store::VerdictView` from src/verdict_store.rs — a stored verdict paired with
// whether it still holds against the current world (REQ039, the living loop). `fresh` is false when
// any provenance axis drifted; `stale_reasons` names each drift the operator must re-verify.
export interface VerdictView {
  status: string;
  basis: string | null;
  reason: string | null;
  fresh: boolean;
  stale_reasons: string[];
  // Where this verdict was proved (REQ049/REQ050), or null when it predates environment recording.
  // Distinct from `fresh` on purpose: an unchanged recorded environment is a checked claim, an
  // absent one is not, and both leave `fresh` true.
  environment: string | null;
}

// Mirrors `EngineReport` from `src/server.rs` (REQ051). `state` is the tag the UI tones by;
// `detail` is the same line the CLI prints. "unusable" is deliberately its own state: the engine
// is installed but cannot start, so it is neither available nor something installing would fix.
export type EngineState =
  | "available"
  | "missing"
  | "unusable"
  | "incompatible"
  | "not-wired";

export interface EngineReport {
  category: string;
  name: string;
  state: EngineState;
  /** The same human line the CLI prints. */
  detail: string;
  /** The bare fault text, for a state that has one to explain — null otherwise. */
  reason: string | null;
}

export interface ItemState {
  id: string;
  title: string | null;
  text: string;
  classification: Classification | null;
  /** What produced that classification (#172); null when the item is untriaged. */
  classified_by: Origin | null;
  formalization: Formalization;
  verdict: VerdictView | null;
}

export interface Backlog {
  coverage: Coverage;
  items: ItemState[];
}

export type Fidelity = "definitional" | "observed" | "probed";

export interface Binding {
  symbol: string;
  category: string;
  observable: string;
  fidelity: Fidelity;
}

// Mirrors `draft::GateStatus` (serde tag "status", snake_case): the mechanical-gate outcome.
export type GateStatus =
  | { status: "ungated" }
  | { status: "passed"; warnings: string[] }
  | { status: "failed"; errors: string[] };

export interface AdmissionInfo {
  review: "mandatory" | "optional";
  by: string;
}

export interface BindingResolution {
  symbol: string;
  observable: string;
  category: string;
  resolved: boolean;
  summary: string;
}

export interface GroundingReport {
  grounded: boolean;
  bindings: BindingResolution[];
}

export interface Detail {
  id: string;
  title: string | null;
  text: string;
  revision: string;
  stale: boolean;
  classification: Classification | null;
  /** What produced that classification (#172); null when the item is untriaged. */
  classified_by: Origin | null;
  formalization: Formalization;
  admission: AdmissionInfo | null;
  candidate: string | null;
  gate: GateStatus | null;
  readback: string | null;
  bindings: Binding[];
  grounding: GroundingReport | null;
  verdict: VerdictView | null;
}

// Mirrors `verdict::report` from `src/verdict.rs` — the `POST /:id/verify` verdict wire shape.
// Polarity/basis/reason carry their human labels (the same strings the CLI prints), so the UI
// renders no enum internals. `status` is "holds" | "fails" | "unknown".

export interface ProvenanceReport {
  requirement_revision: string;
  subject_commit: string | null;
  tool_version: string;
}

export interface EvidenceReport {
  engine: string;
  status: string;
  basis: string | null;
  witness: string | null;
  detail: string[];
}

export interface VerdictReport {
  id: string;
  status: string;
  basis: string | null;
  reason: string | null;
  witness: string | null;
  detail: string[];
  evidence: EvidenceReport[];
  provenance: ProvenanceReport;
}

// Mirrors `verify_payload` from `src/server.rs` — the `state` tag discriminates a real verdict
// from each honest not-yet-verifiable state (nothing fabricated when there is nothing to run).
export type VerifyResponse =
  | { state: "no-draft" }
  | { state: "not-admitted" }
  | { state: "no-candidate" }
  | { state: "gate-failed"; errors: string[] }
  | { state: "verdict"; stale: boolean; verdict: VerdictReport };

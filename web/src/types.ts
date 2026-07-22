// Mirrors the `GET /api/requirements` payload from `src/server.rs` (Backlog) — the
// serde field names (snake_case coverage counts, kebab-case enum values) are the contract.

export type Classification =
  | "formalizable-now"
  | "falsifiable-only"
  | "stays-prose";

export type Formalization = "none" | "drafting" | "admitted";

export interface Coverage {
  discovered: number;
  untriaged: number;
  formalizable_now: number;
  falsifiable_only: number;
  stays_prose: number;
  drafting: number;
  formalized: number;
  verified: number;
}

export interface ItemState {
  id: string;
  title: string | null;
  text: string;
  classification: Classification | null;
  formalization: Formalization;
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
  formalization: Formalization;
  admission: AdmissionInfo | null;
  candidate: string | null;
  gate: GateStatus | null;
  readback: string | null;
  bindings: Binding[];
  grounding: GroundingReport | null;
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

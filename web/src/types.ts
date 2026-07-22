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

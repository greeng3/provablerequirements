import type { Classification, Formalization } from "./types";

export type Tone = "accent" | "warn" | "muted" | "ok" | "info";

/** Operator-facing label + semantic tone for a triage classification (null = untriaged). */
export function triage(c: Classification | null): { label: string; tone: Tone } {
  switch (c) {
    case "formalizable-now":
      return { label: "formalizable now", tone: "info" };
    case "falsifiable-only":
      return { label: "falsifiable only", tone: "warn" };
    case "stays-prose":
      return { label: "stays prose", tone: "muted" };
    case null:
      return { label: "untriaged", tone: "muted" };
  }
}

/** Operator-facing label + semantic tone for a formalization state. */
export function formalization(f: Formalization): { label: string; tone: Tone } {
  switch (f) {
    case "admitted":
      return { label: "admitted", tone: "ok" };
    case "drafting":
      return { label: "drafting", tone: "accent" };
    case "none":
      return { label: "—", tone: "muted" };
  }
}

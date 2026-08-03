import type { Classification, Formalization, Origin } from "./types";

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

/**
 * How a classification's origin reads beside it — mirroring `Origin::note()` in src/triage.rs, so
 * the browser and the CLI annotate the same buckets with the same words.
 *
 * Empty for a real classification and for an untriaged item: only the origins carrying less than
 * they appear to are annotated. Annotating a judgement too would make the note decoration, and the
 * whole point is that its presence means something (#180).
 */
export function origin(o: Origin | null): string {
  switch (o) {
    case "seeded":
      return "seeded — no classifier ran";
    case "operator":
      return "set by the operator";
    case "unrecorded":
      return "origin not recorded";
    case "classified":
    case null:
      return "";
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

import { Fragment } from "react";

interface Props {
  readonly snippet: string;
}

/// Render the backend's HTML-escaped + `<mark>`-marked snippet
/// as spans without `dangerouslySetInnerHTML`. The backend
/// escapes everything except the `<mark>` / `</mark>` tags; we
/// split on the literal token so a future injection attempt
/// surfaces as visible text rather than executing.
export function SearchSnippet({ snippet }: Props) {
  const segments = parseMarkSegments(snippet);
  return (
    <p className="text-xs text-slate-600 dark:text-slate-400">
      {segments.map((seg, idx) => (
        <Fragment key={idx}>
          {seg.kind === "match" ? (
            <mark
              data-testid="search-snippet-mark"
              className="rounded bg-amber-200 px-0.5 text-amber-950 dark:bg-amber-400 dark:text-amber-950"
            >
              {seg.text}
            </mark>
          ) : (
            <span>{seg.text}</span>
          )}
        </Fragment>
      ))}
    </p>
  );
}

type Segment = { kind: "plain" | "match"; text: string };

export function parseMarkSegments(html: string): Segment[] {
  // Split on the literal opening tag first; each piece after
  // the first opener either contains a closing tag (match +
  // rest) or none (the "plain" suffix).
  const out: Segment[] = [];
  const parts = html.split("<mark>");
  const leading = parts.shift();
  if (leading) out.push({ kind: "plain", text: decodeEntities(leading) });
  for (const p of parts) {
    const closeIdx = p.indexOf("</mark>");
    if (closeIdx < 0) {
      // Missing close — treat the whole rest as plain to stay
      // safe.
      out.push({ kind: "plain", text: decodeEntities(p) });
      continue;
    }
    const match = p.slice(0, closeIdx);
    const rest = p.slice(closeIdx + "</mark>".length);
    out.push({ kind: "match", text: decodeEntities(match) });
    if (rest) out.push({ kind: "plain", text: decodeEntities(rest) });
  }
  return out;
}

/// Tantivy's `Snippet::to_html()` HTML-escapes ampersands,
/// angle brackets, and quotes before emitting the `<b>` tags
/// we rewrite server-side. Decode the five XML entities so the
/// rendered text reads naturally; no other entity is emitted.
function decodeEntities(text: string): string {
  return text
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#x27;/g, "'")
    .replace(/&amp;/g, "&");
}

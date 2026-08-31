import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";

/// Renders the artifact's Markdown body using remark-gfm (tables,
/// task lists, strikethrough, autolinks) and rehype-highlight for
/// syntax highlighting. Tailwind Typography gives a comfortable
/// reading width without any per-element tweaks.
export function ArtifactMarkdown({ body }: { body: string | null }) {
  if (!body) {
    return <p className="text-sm text-slate-500">This artifact has no body.</p>;
  }
  return (
    <article className="prose prose-slate dark:prose-invert max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
      >
        {body}
      </ReactMarkdown>
    </article>
  );
}

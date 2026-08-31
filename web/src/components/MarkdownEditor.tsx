// Side-by-side Markdown editor per UX-markdownEditor: CodeMirror 6
// on the left, react-markdown preview on the right. One-way data
// flow — the text pane is the source of truth; the preview
// re-renders from it on every keystroke.

import { autocompletion } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown as markdownLanguage } from "@codemirror/lang-markdown";
import {
  bracketMatching,
  foldGutter,
  indentOnInput,
} from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import {
  EditorView,
  drawSelection,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";

/// Editor theme: make the caret visible regardless of Tailwind's
/// preflight, and give the selection a usable contrast. Without
/// this, the default CodeMirror caret `border-left-color` falls
/// through to `currentColor` which, combined with Tailwind's
/// body colour rules, can render invisible on some backgrounds.
const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
  },
  ".cm-content": {
    caretColor: "currentColor",
  },
  "&.cm-focused .cm-cursor": {
    borderLeftColor: "currentColor",
    borderLeftWidth: "2px",
  },
  ".cm-scroller": {
    fontFamily: "inherit",
  },
});

export interface MarkdownEditorProps {
  /// Current Markdown text. The editor's document is synced from
  /// this on every change — if a parent resets `value` (say, on
  /// "discard"), the editor catches up.
  readonly value: string;
  /// Called with the full document text on every edit. Keep the
  /// handler stable (useCallback or component-state reducer) so
  /// the editor doesn't rebuild on every keystroke.
  readonly onChange: (next: string) => void;
  readonly ariaLabel?: string;
}

export function MarkdownEditor({
  value,
  onChange,
  ariaLabel = "Markdown source",
}: MarkdownEditorProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Latch the latest onChange so the updateListener reads the
  // current callback — avoids stale closures when the parent
  // re-renders.
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  // Mount the editor once per parent-component lifetime.
  useEffect(() => {
    if (!containerRef.current) return;
    const state = EditorState.create({
      doc: value,
      extensions: [
        keymap.of([...defaultKeymap, ...historyKeymap]),
        history(),
        drawSelection(),
        lineNumbers(),
        foldGutter(),
        highlightActiveLineGutter(),
        bracketMatching(),
        indentOnInput(),
        autocompletion(),
        markdownLanguage(),
        editorTheme,
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
        EditorView.contentAttributes.of({ "aria-label": ariaLabel }),
      ],
    });
    const view = new EditorView({ state, parent: containerRef.current });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Mount-only. Subsequent `value` / `onChange` changes are
    // propagated via the sibling effects below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync external `value` resets into the editor without
  // clobbering the cursor mid-type: only replace the document when
  // the current text differs from the prop.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div className="grid h-[60vh] grid-cols-1 gap-4 lg:grid-cols-2">
      <div
        ref={containerRef}
        className="overflow-auto rounded-lg border border-slate-200 bg-white font-mono text-sm dark:border-slate-800 dark:bg-slate-900"
      />
      <article
        className="prose prose-slate dark:prose-invert max-w-none overflow-auto rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900"
        aria-label="Rendered preview"
      >
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeHighlight]}
        >
          {value}
        </ReactMarkdown>
      </article>
    </div>
  );
}

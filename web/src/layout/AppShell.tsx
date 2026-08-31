import { useCallback, useEffect, useState } from "react";
import { Outlet } from "react-router-dom";

import { Breadcrumbs } from "./Breadcrumbs";
import { Header } from "./Header";
import { KeyboardShortcutsOverlay } from "./KeyboardShortcutsOverlay";
import { Sidebar } from "./Sidebar";

/// Top-level layout: fixed header, sidebar + main content, with
/// breadcrumbs above the scrolling content region.
///
/// Phase 11c adds:
/// - A skip-to-main link as the first focusable element, so
///   keyboard users can bypass the sidebar + header.
/// - A global `?` hotkey that opens the keyboard-shortcuts
///   overlay. The hotkey is suppressed while a text input or
///   contenteditable is focused so it doesn't steal literal `?`
///   keystrokes from the Markdown editor.
export function AppShell() {
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const openShortcuts = useCallback(() => setShortcutsOpen(true), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "?") return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (isEditableTarget(e.target)) return;
      e.preventDefault();
      setShortcutsOpen(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex min-h-screen flex-col">
      <a
        href="#main"
        data-testid="skip-to-main"
        className="sr-only z-30 bg-slate-900 px-3 py-1 text-sm font-medium text-white focus:not-sr-only focus:fixed focus:left-2 focus:top-2"
      >
        Skip to main content
      </a>
      <Header onOpenShortcuts={openShortcuts} />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <div className="flex flex-1 flex-col overflow-hidden">
          <Breadcrumbs />
          <main id="main" className="flex-1 overflow-auto px-6 py-6">
            <Outlet />
          </main>
        </div>
      </div>
      <KeyboardShortcutsOverlay
        open={shortcutsOpen}
        onClose={() => setShortcutsOpen(false)}
      />
    </div>
  );
}

/// Treat text inputs, textareas, and any contenteditable region
/// as "do not steal keystrokes from me". Covers the CodeMirror
/// case because its host element carries
/// `contenteditable="true"` on the inner content region.
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

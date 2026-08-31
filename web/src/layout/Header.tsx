import { Link } from "react-router-dom";

interface Props {
  readonly onOpenShortcuts: () => void;
}

export function Header({ onOpenShortcuts }: Props) {
  return (
    <header className="border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-3">
        <Link
          to="/"
          className="flex items-center gap-2 text-lg font-semibold tracking-tight text-slate-900 dark:text-slate-100"
        >
          <img
            src="/ReqForge_192x105.png"
            alt=""
            aria-hidden="true"
            className="h-8 w-auto"
          />
          <span>ReqForge</span>
        </Link>
        <div className="flex items-center gap-2">
          {import.meta.env.DEV ? (
            <span
              className="rounded bg-slate-100 px-2 py-0.5 font-mono text-xs text-slate-500 dark:bg-slate-800 dark:text-slate-400"
              aria-label="development mode port"
            >
              dev · :5173
            </span>
          ) : null}
          <button
            type="button"
            onClick={onOpenShortcuts}
            title="Keyboard shortcuts (press ?)"
            aria-label="Keyboard shortcuts"
            data-testid="header-shortcuts-button"
            className="rounded border border-slate-300 px-2 py-0.5 font-mono text-xs text-slate-600 hover:bg-slate-100 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            ?
          </button>
        </div>
      </div>
    </header>
  );
}

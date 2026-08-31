import { useEffect, useId, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";

import {
  useCreateArtifact,
  useCreateBlobArtifact,
  useCreateUrlArtifact,
} from "../../api/queries";
import type { ArtifactDetail } from "../../api/types";

interface Props {
  readonly projectSlug: string;
  readonly collectionPrefix: string;
  readonly onClose: () => void;
}

type Tab = "markdown" | "upload" | "url";

export function NewArtifactDialog({
  projectSlug,
  collectionPrefix,
  onClose,
}: Props) {
  const [tab, setTab] = useState<Tab>("markdown");

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="new-artifact-heading"
      className="fixed inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-slate-200 bg-white p-6 shadow-lg dark:border-slate-700 dark:bg-slate-900"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="new-artifact-heading"
          className="text-lg font-semibold tracking-tight"
        >
          New artifact
        </h2>
        <div
          role="tablist"
          aria-label="Artifact shape"
          className="mt-3 flex gap-1 border-b border-slate-200 dark:border-slate-700"
        >
          <TabButton
            id="markdown"
            label="Markdown"
            active={tab === "markdown"}
            onSelect={setTab}
          />
          <TabButton
            id="upload"
            label="Upload file"
            active={tab === "upload"}
            onSelect={setTab}
          />
          <TabButton
            id="url"
            label="Link URL"
            active={tab === "url"}
            onSelect={setTab}
          />
        </div>
        <div className="mt-4">
          {tab === "markdown" ? (
            <MarkdownForm
              projectSlug={projectSlug}
              collectionPrefix={collectionPrefix}
              onClose={onClose}
            />
          ) : null}
          {tab === "upload" ? (
            <UploadForm
              projectSlug={projectSlug}
              collectionPrefix={collectionPrefix}
              onClose={onClose}
            />
          ) : null}
          {tab === "url" ? (
            <UrlForm
              projectSlug={projectSlug}
              collectionPrefix={collectionPrefix}
              onClose={onClose}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

function TabButton({
  id,
  label,
  active,
  onSelect,
}: {
  id: Tab;
  label: string;
  active: boolean;
  onSelect: (id: Tab) => void;
}) {
  return (
    <button
      role="tab"
      aria-selected={active}
      type="button"
      onClick={() => onSelect(id)}
      className={`rounded-t px-3 py-1.5 text-sm ${
        active
          ? "border-b-2 border-slate-900 font-semibold text-slate-900 dark:border-slate-100 dark:text-slate-100"
          : "text-slate-500 hover:text-slate-700 dark:hover:text-slate-200"
      }`}
    >
      {label}
    </button>
  );
}

function navigateToDetail(
  detail: ArtifactDetail,
  projectSlug: string,
  collectionPrefix: string,
  navigate: ReturnType<typeof useNavigate>,
) {
  navigate(
    `/projects/${projectSlug}/collections/${collectionPrefix}/artifacts/${detail.name}`,
  );
}

function MarkdownForm({
  projectSlug,
  collectionPrefix,
  onClose,
}: {
  projectSlug: string;
  collectionPrefix: string;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const mutation = useCreateArtifact(projectSlug, collectionPrefix);
  const nameId = useId();
  const titleId = useId();
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    mutation.mutate(
      { name: name.trim(), title: title.trim() },
      {
        onSuccess: (detail) =>
          navigateToDetail(detail, projectSlug, collectionPrefix, navigate),
      },
    );
  };

  return (
    <form onSubmit={submit} className="space-y-3">
      <NameField
        id={nameId}
        value={name}
        onChange={setName}
        collectionPrefix={collectionPrefix}
      />
      <TitleField id={titleId} value={title} onChange={setTitle} />
      <FormError error={mutation.error} />
      <FormFooter
        disabled={mutation.isPending || !name || !title}
        pending={mutation.isPending}
        onClose={onClose}
        submitLabel="Create"
      />
    </form>
  );
}

function UploadForm({
  projectSlug,
  collectionPrefix,
  onClose,
}: {
  projectSlug: string;
  collectionPrefix: string;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const mutation = useCreateBlobArtifact(projectSlug, collectionPrefix);
  const nameId = useId();
  const titleId = useId();
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [file, setFile] = useState<File | null>(null);

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!file) return;
    const form = new FormData();
    form.append("name", name.trim());
    form.append("title", title.trim());
    form.append("file", file);
    mutation.mutate(form, {
      onSuccess: (detail) =>
        navigateToDetail(detail, projectSlug, collectionPrefix, navigate),
    });
  };

  return (
    <form onSubmit={submit} className="space-y-3">
      <NameField
        id={nameId}
        value={name}
        onChange={setName}
        collectionPrefix={collectionPrefix}
      />
      <TitleField id={titleId} value={title} onChange={setTitle} />
      <label className="block text-sm">
        <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
          File
        </span>
        <input
          type="file"
          required
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          className="block w-full text-sm"
        />
        <span className="mt-1 block text-xs text-slate-500">
          PDF, Office, and common images up to 50 MB by default.
        </span>
      </label>
      <FormError error={mutation.error} />
      <FormFooter
        disabled={mutation.isPending || !name || !title || !file}
        pending={mutation.isPending}
        onClose={onClose}
        submitLabel="Upload"
      />
    </form>
  );
}

function UrlForm({
  projectSlug,
  collectionPrefix,
  onClose,
}: {
  projectSlug: string;
  collectionPrefix: string;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const mutation = useCreateUrlArtifact(projectSlug, collectionPrefix);
  const nameId = useId();
  const titleId = useId();
  const urlId = useId();
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [url, setUrl] = useState("");

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    mutation.mutate(
      { name: name.trim(), title: title.trim(), url: url.trim() },
      {
        onSuccess: (detail) =>
          navigateToDetail(detail, projectSlug, collectionPrefix, navigate),
      },
    );
  };

  return (
    <form onSubmit={submit} className="space-y-3">
      <NameField
        id={nameId}
        value={name}
        onChange={setName}
        collectionPrefix={collectionPrefix}
      />
      <TitleField id={titleId} value={title} onChange={setTitle} />
      <label className="block text-sm">
        <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
          URL
        </span>
        <input
          id={urlId}
          type="url"
          required
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://example.com/spec"
          className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
        />
        <span className="mt-1 block text-xs text-slate-500">
          Must start with http:// or https://.
        </span>
      </label>
      <FormError error={mutation.error} />
      <FormFooter
        disabled={mutation.isPending || !name || !title || !url}
        pending={mutation.isPending}
        onClose={onClose}
        submitLabel="Create link"
      />
    </form>
  );
}

function NameField({
  id,
  value,
  onChange,
  collectionPrefix,
}: {
  id: string;
  value: string;
  onChange: (v: string) => void;
  collectionPrefix: string;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
        Name
      </span>
      <input
        id={id}
        required
        value={value}
        onChange={(e) => onChange(e.target.value)}
        pattern="[A-Za-z0-9._\-]+"
        placeholder={`${collectionPrefix}-example`}
        className="w-full rounded border border-slate-300 px-2 py-1 font-mono text-sm dark:border-slate-600 dark:bg-slate-800"
      />
      <span className="mt-1 block text-xs text-slate-500">
        Filename stem. Letters, digits, dot, underscore, hyphen only.
      </span>
    </label>
  );
}

function TitleField({
  id,
  value,
  onChange,
}: {
  id: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
        Title
      </span>
      <input
        id={id}
        required
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded border border-slate-300 px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-800"
      />
    </label>
  );
}

function FormError({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <p className="text-sm text-rose-600" role="alert">
      {String(error)}
    </p>
  );
}

function FormFooter({
  disabled,
  pending,
  onClose,
  submitLabel,
}: {
  disabled: boolean;
  pending: boolean;
  onClose: () => void;
  submitLabel: string;
}) {
  return (
    <div className="flex justify-end gap-2 pt-2">
      <button
        type="button"
        onClick={onClose}
        className="rounded border border-slate-300 px-3 py-1 text-sm hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-slate-800"
      >
        Cancel
      </button>
      <button
        type="submit"
        disabled={disabled}
        className="rounded bg-slate-900 px-3 py-1 text-sm text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900"
      >
        {pending ? "Working…" : submitLabel}
      </button>
    </div>
  );
}

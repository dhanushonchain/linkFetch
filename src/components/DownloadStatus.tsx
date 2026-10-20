import type { FileInfo } from "../lib/tauri";

export type Phase = "running" | "completed" | "failed" | "cancelled";

interface DownloadStatusProps {
  phase: Phase;
  percent: number | null;
  detail: string | null;
  /** Position in the queue, e.g. "2/5". */
  position: string | null;
  files: FileInfo[];
  hasError: boolean;
  onReveal: (path: string) => void;
  onOpen: (path: string) => void;
  onRetry: () => void;
  onCancel: () => void;
}

/** Thin progress plus the small actions for the current result. */
export function DownloadStatus({
  phase,
  percent,
  detail,
  position,
  files,
  hasError,
  onReveal,
  onOpen,
  onRetry,
  onCancel,
}: DownloadStatusProps) {
  if (phase === "running") {
    return (
      <div className="space-y-2">
        <div className="h-[2px] w-full overflow-hidden rounded-full bg-border">
          {percent !== null ? (
            <div
              className="h-full rounded-full bg-accent transition-[width] duration-200"
              style={{ width: `${Math.min(Math.max(percent, 0), 100)}%` }}
            />
          ) : (
            <div className="h-full w-1/3 animate-pulse rounded-full bg-accent/60" />
          )}
        </div>
        <div className="flex items-center justify-between gap-3 text-[11px] text-muted">
          <span className="truncate">
            {percent !== null ? `${Math.round(percent)}%` : null}
            {percent !== null ? " \u00b7 " : null}
            {position ? `${position} \u00b7 ` : null}
            {detail ?? "Working..."}
          </span>
          <button type="button" onClick={onCancel} className="shrink-0 hover:text-ink">
            Cancel
          </button>
        </div>
      </div>
    );
  }

  const file = files[files.length - 1];
  const showFile = phase === "completed" && !hasError && file;

  return (
    <div className="flex items-center justify-between gap-3 text-[11px]">
      <span className="flex min-w-0 items-center gap-1.5 text-muted">
        {showFile ? (
          <svg viewBox="0 0 12 12" className="h-3 w-3 shrink-0 text-accent" aria-hidden="true">
            <path
              d="M2.5 6.5l2.5 2.5 4.5-5"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        ) : (
          <svg viewBox="0 0 12 12" className="h-3 w-3 shrink-0" aria-hidden="true">
            <circle cx="6" cy="6" r="5" fill="none" stroke="currentColor" strokeWidth="1.2" />
            <path d="M6 3.2v3.4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
            <circle cx="6" cy="8.6" r="0.7" fill="currentColor" />
          </svg>
        )}
        <span className="truncate">{showFile ? file.name : (detail ?? "Something went wrong")}</span>
      </span>
      <span className="flex shrink-0 gap-3">
        {hasError ? (
          <button type="button" onClick={onRetry} className="text-accent hover:opacity-80">
            Retry
          </button>
        ) : null}
        {showFile ? (
          <>
            <button type="button" onClick={() => onReveal(file.path)} className="text-muted hover:text-ink">
              Show in Finder
            </button>
            <button type="button" onClick={() => onOpen(file.path)} className="text-muted hover:text-ink">
              Open file
            </button>
          </>
        ) : null}
      </span>
    </div>
  );
}

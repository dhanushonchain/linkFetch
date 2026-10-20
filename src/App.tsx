import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { DownloadButton } from "./components/DownloadButton";
import { DownloadStatus, type Phase } from "./components/DownloadStatus";
import { LinkInput } from "./components/LinkInput";
import { TargetSelect } from "./components/TargetSelect";
import { loadDestination, loadTarget, saveDestination, saveTarget } from "./lib/prefs";
import * as api from "./lib/tauri";
import type { FileInfo } from "./lib/tauri";
import { hostOf, parseLinks, type Target } from "./lib/validation";

const NEEDS_YT_DLP: Target[] = ["video", "audio", "video_audio"];

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Something went wrong";
}

export default function App() {
  const [text, setText] = useState("");
  const [target, setTarget] = useState<Target>(loadTarget);
  const [destination, setDestination] = useState<string | null>(loadDestination);

  const [phase, setPhase] = useState<Phase | "idle">("idle");
  const [percent, setPercent] = useState<number | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [source, setSource] = useState<{ key: string; unsupported: string | null } | null>(null);
  const [dependencies, setDependencies] = useState<api.Dependencies | null>(null);

  const jobId = useRef<string | null>(null);

  const parsed = useMemo(() => parseLinks(text), [text]);
  const firstUrl = parsed.links[0]?.url ?? null;
  const host = firstUrl ? hostOf(firstUrl) : "";
  const sourceKey = firstUrl ? `${firstUrl}|${target}` : null;
  const limitation = source && source.key === sourceKey ? source.unsupported : null;
  const linkCount = useRef(0);

  useEffect(() => {
    linkCount.current = parsed.links.length;
  }, [parsed.links.length]);

  useEffect(() => {
    api.dependencyStatus().then(setDependencies).catch(() => setDependencies(null));
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let cancelled = false;

    api
      .onProgress((progress) => {
        if (cancelled) return;
        setPhase("running");
        setPercent(progress.percent);
        setDetail(progress.filename ?? null);
      })
      .then((unlisten) => unlisteners.push(unlisten))
      .catch(() => undefined);

    api
      .onItem((item) => {
        if (cancelled) return;
        if (item.ok) {
          setFiles((current) => [...current, ...item.files]);
          setError(null);
        } else {
          setError(item.error?.message ?? "Download failed");
          // A single failed link is the whole result, a batch keeps going.
          if (linkCount.current <= 1) setPhase("failed");
        }
      })
      .then((unlisten) => unlisteners.push(unlisten))
      .catch(() => undefined);

    api
      .onJob((job) => {
        if (cancelled) return;
        jobId.current = null;
        if (job.cancelled) {
          setPhase("cancelled");
          setDetail("Cancelled");
        } else if (job.succeeded > 0) {
          setPhase("completed");
        } else {
          setPhase("failed");
        }
      })
      .then((unlisten) => unlisteners.push(unlisten))
      .catch(() => undefined);

    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (!firstUrl || !sourceKey) return;
    let cancelled = false;
    api
      .describeUrl(firstUrl, target)
      .then((info) => {
        if (!cancelled) setSource({ key: sourceKey, unsupported: info.unsupported });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [firstUrl, target, sourceKey]);

  const running = phase === "running";
  const missingTool =
    target === "audio" || target === "video_audio"
      ? !dependencies?.ffmpeg
        ? "FFmpeg is not installed, so audio cannot be converted."
        : !dependencies?.yt_dlp
          ? "yt-dlp is not installed."
          : null
      : NEEDS_YT_DLP.includes(target) && !dependencies?.yt_dlp
        ? "yt-dlp is not installed."
        : null;

  const canDownload = parsed.links.length > 0 && !running;

  const start = useCallback(async () => {
    if (parsed.links.length === 0 || running) return;
    setPhase("running");
    setPercent(null);
    setDetail(null);
    setFiles([]);
    setError(null);
    try {
      jobId.current = await api.startDownload(
        parsed.links.map((link) => link.url),
        target,
        destination,
      );
    } catch (caught) {
      jobId.current = null;
      setPhase("failed");
      setError(errorMessage(caught));
    }
  }, [parsed.links, running, target, destination]);

  const cancel = useCallback(async () => {
    if (jobId.current) await api.cancelDownload(jobId.current);
  }, []);

  const chooseDestination = useCallback(async () => {
    const picked = await api.pickDestination();
    if (picked) {
      setDestination(picked);
      saveDestination(picked);
    }
  }, []);

  const changeTarget = useCallback((next: Target) => {
    setTarget(next);
    saveTarget(next);
  }, []);

  const onTextChange = useCallback((value: string) => {
    setText(value);
    if (phase !== "running") {
      setPhase("idle");
      setFiles([]);
      setError(null);
      setPercent(null);
      setDetail(null);
    }
  }, [phase]);

  const statusDetail = error ?? detail;

  return (
    <main className="flex h-full flex-col gap-3 px-5 py-4">
      <LinkInput
        value={text}
        onChange={onTextChange}
        onSubmit={start}
        disabled={running}
        singleLine={!text.includes("\n")}
      />

      <TargetSelect value={target} onChange={changeTarget} disabled={running} />

      {limitation ? (
        <p className="-mt-1 text-[11px] leading-snug text-muted">{limitation}</p>
      ) : missingTool ? (
        <p className="-mt-1 text-[11px] leading-snug text-muted">{missingTool}</p>
      ) : null}

      <DownloadButton
        disabled={!canDownload}
        loading={running}
        label={running ? "Downloading" : "Download"}
        onClick={start}
      />

      {phase !== "idle" ? (
        <DownloadStatus
          phase={phase}
          percent={percent}
          detail={statusDetail}
          files={files}
          hasError={error !== null}
          onReveal={(path) => api.revealInFinder(path)}
          onOpen={(path) => api.openFile(path)}
          onRetry={start}
          onCancel={cancel}
        />
      ) : null}

      <footer className="mt-auto flex items-center justify-between gap-3 border-t border-border pt-2 text-[11px] text-muted">
        <button
          type="button"
          onClick={chooseDestination}
          disabled={running}
          className="shrink-0 rounded-[8px] bg-accent px-2.5 py-1 font-medium text-[#171717] transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          Change
        </button>
        <span className="truncate text-right">
          {host ? `${host} · ` : ""}
          {destination ?? "Downloads"}
        </span>
      </footer>
    </main>
  );
}

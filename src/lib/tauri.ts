import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Target } from "./validation";

export interface AppError {
  code: string;
  message: string;
}

export interface FileInfo {
  path: string;
  name: string;
  size: number | null;
}

export interface Progress {
  job_id: string;
  index: number;
  total: number;
  state:
    | "validating"
    | "downloading"
    | "extracting"
    | "converting"
    | "completed"
    | "failed"
    | "cancelled";
  url: string;
  filename: string | null;
  percent: number | null;
  message: string | null;
}

export interface ItemResult {
  job_id: string;
  index: number;
  url: string;
  ok: boolean;
  files: FileInfo[];
  error: AppError | null;
}

export interface JobResult {
  job_id: string;
  total: number;
  succeeded: number;
  failed: number;
  cancelled: boolean;
}

export interface Dependencies {
  yt_dlp: string | null;
  yt_dlp_version: string | null;
  ffmpeg: string | null;
}

export interface SourceInfo {
  host: string;
  site: string | null;
  supported_targets: Target[];
  unsupported: string | null;
}

export function startDownload(
  urls: string[],
  target: Target,
  destination: string | null,
): Promise<string> {
  return invoke<string>("start_download", { urls, target, destination });
}

export function cancelDownload(jobId: string): Promise<void> {
  return invoke<void>("cancel_download", { jobId });
}

export function dependencyStatus(): Promise<Dependencies> {
  return invoke<Dependencies>("dependency_status");
}

export function defaultDestination(): Promise<string> {
  return invoke<string>("default_destination_command");
}

export function pickDestination(): Promise<string | null> {
  return invoke<string | null>("pick_destination");
}

export function revealInFinder(path: string): Promise<void> {
  return invoke<void>("reveal_in_finder", { path });
}

export function openFile(path: string): Promise<void> {
  return invoke<void>("open_file", { path });
}

export function describeUrl(url: string, target: Target): Promise<SourceInfo> {
  return invoke<SourceInfo>("describe_url", { url, target });
}

export function onProgress(handler: (progress: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("linkfetch://progress", (event) => handler(event.payload));
}

export function onItem(handler: (item: ItemResult) => void): Promise<UnlistenFn> {
  return listen<ItemResult>("linkfetch://item", (event) => handler(event.payload));
}

export function onJob(handler: (job: JobResult) => void): Promise<UnlistenFn> {
  return listen<JobResult>("linkfetch://job", (event) => handler(event.payload));
}

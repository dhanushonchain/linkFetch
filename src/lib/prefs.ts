import type { Target } from "./validation";

const TARGET_KEY = "linkfetch.target";
const DESTINATION_KEY = "linkfetch.destination";

/** Remember the last used target and destination folder locally. */
export function loadTarget(): Target {
  const value = localStorage.getItem(TARGET_KEY);
  if (value === "video" || value === "audio" || value === "image" || value === "gif" || value === "text") {
    return value;
  }
  return "video";
}

export function saveTarget(target: Target): void {
  localStorage.setItem(TARGET_KEY, target);
}

export function loadDestination(): string | null {
  return localStorage.getItem(DESTINATION_KEY);
}

export function saveDestination(path: string | null): void {
  if (path) localStorage.setItem(DESTINATION_KEY, path);
  else localStorage.removeItem(DESTINATION_KEY);
}

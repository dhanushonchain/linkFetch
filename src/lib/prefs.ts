import { TARGETS, type Target } from "./validation";

const TARGET_KEY = "linkfetch.target";
const DESTINATION_KEY = "linkfetch.destination";

/** Remember the last used target and destination folder locally. */
export function loadTarget(): Target {
  const value = localStorage.getItem(TARGET_KEY);
  const match = TARGETS.find((target) => target.value === value);
  return match ? match.value : "video";
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

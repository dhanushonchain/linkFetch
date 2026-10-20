export type Target = "video" | "audio" | "image" | "gif" | "text";

export const TARGETS: { value: Target; label: string }[] = [
  { value: "video", label: "Video" },
  { value: "audio", label: "Audio" },
  { value: "image", label: "Image" },
  { value: "gif", label: "GIF" },
  { value: "text", label: "Text" },
];

export interface ParsedLink {
  raw: string;
  url: string;
}

export interface LinkParseResult {
  links: ParsedLink[];
  invalid: { raw: string; reason: string }[];
}

/** Basic URL check: http(s) only, with a host. */
export function isValidUrl(raw: string): boolean {
  const value = raw.trim();
  if (!value) return false;
  const withScheme = value.includes("://") ? value : `https://${value}`;
  try {
    const url = new URL(withScheme);
    if (url.protocol !== "http:" && url.protocol !== "https:") return false;
    if (!url.hostname) return false;
    // A public host always has a dot; bare names are not fetchable.
    return url.hostname.includes(".");
  } catch {
    return false;
  }
}

/** Split the textarea content into valid and invalid links. */
export function parseLinks(text: string): LinkParseResult {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  const links: ParsedLink[] = [];
  const invalid: { raw: string; reason: string }[] = [];

  for (const raw of lines) {
    if (isValidUrl(raw)) {
      const withScheme = raw.includes("://") ? raw : `https://${raw}`;
      links.push({ raw, url: withScheme });
    } else {
      invalid.push({ raw, reason: "not a valid http(s) link" });
    }
  }

  return { links, invalid };
}

/** Host of a link, for the small source hint. */
export function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

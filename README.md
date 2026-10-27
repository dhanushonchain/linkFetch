# linkFetch

**Paste. Choose. Fetch.**

A minimal native macOS utility for downloading or extracting content from a
link. Paste a URL, pick what you want out of it, press Download. The file
lands in your Downloads folder.

<img src="assets/logo.png" width="160" alt="linkFetch logo">

---

## What it does

| Target   | What happens                                                                |
| -------- | --------------------------------------------------------------------------- |
| `Video`  | The video is downloaded **video‑only** (no audio) with `yt‑dlp`.
| `Audio`  | The audio is extracted with `yt-dlp` and converted to MP3 (needs FFmpeg).    |
| `Video + Audio` | A single merged MP4 containing both video and audio (no separate MP3).
| `Image`  | The image file is fetched directly and written to disk.                       |
| `GIF`    | The GIF is fetched directly and written to disk.                              |
| `Text`   | The page is fetched and its readable article text is saved as a `.txt` file.  |

* One or many links — one per line. They are processed sequentially.
* `Video + Audio` produces a merged MP4 file (requires FFmpeg).
  needs FFmpeg and takes roughly twice as long as `Video`. The extracted track
  gets an `(audio)` suffix in its file name.
* The last used target is remembered between launches.
* Completed files can be revealed in Finder or opened with the default app.
* Files are saved to `~/Downloads` unless you pick another folder.

## Supported links

* **Direct media** (`Image`, `GIF`): any `http(s)` URL that serves the media
  file itself. The response `Content-Type` is checked, so a link to an HTML
  page is rejected instead of silently saving a web page.
* **Video / Audio / Video + Audio**: any URL that `yt-dlp` supports (YouTube, Vimeo, SoundCloud,
  Twitter/X, Reddit, TikTok, and many more — see the
  [yt-dlp supported sites](https://github.com/yt-dlp/yt-dlp#supported-sites)
  list). Direct media URLs work as well.
* **Text**: any public HTML page. The article text is extracted with
  `readability`; pages that need JavaScript or a login are reported as
  unsupported instead of returning an empty file.

Sites that do not expose their media as a plain file (Instagram, TikTok, X,
…) report *“Instagram does not expose direct media files. Try Video, Audio or
Text.”* when you pick `Image` or `GIF`.

## Requirements

* macOS 11 or newer (built and tested on macOS 26, Apple Silicon).
* `yt-dlp` — required for `Video` and `Audio`:

  ```sh
  brew install yt-dlp
  ```

* `FFmpeg` — required for `Audio` (MP3 conversion) and for merging the best
  video and audio streams:

  ```sh
  brew install ffmpeg
  ```

Both tools are looked up in `PATH`, in `/opt/homebrew/bin`, `/usr/local/bin`,
`/usr/bin` and `~/.local/bin`. To point the app at a specific build, set
`LINKFETCH_YT_DLP_PATH` or `LINKFETCH_FFMPEG_PATH` before launching it.

If a tool is missing the app says so plainly instead of failing silently.

## Development

Requirements: Node.js 20+, Rust 1.90+, and the two tools above.

```sh
npm install
npm run tauri dev      # run the desktop app with hot reload
```

Useful commands:

| Command             | What it does                          |
| ------------------- | ------------------------------------- |
| `npm run dev`       | Frontend only, in the browser          |
| `npm run build`     | Type check + production frontend build |
| `npm run tauri build` | Build the `.app` and `.dmg`           |
| `npm test`          | Frontend unit tests (Vitest)           |
| `npm run lint`      | Lint the frontend (oxlint)             |
| `cargo test`        | Rust unit tests (in `src-tauri`)       |
| `cargo test -- --ignored` | Live network checks (needs internet) |

## Building the app

```sh
npm run tauri build
```

Output:

* `src-tauri/target/release/bundle/macos/linkFetch.app`
* `src-tauri/target/release/bundle/dmg/linkFetch_0.1.0_aarch64.dmg`

The bundle is unsigned, so macOS will ask you to confirm the first launch
(right click the app → *Open*). No administrator privileges are needed.

## Architecture

```
src/                     React + TypeScript frontend
  components/            LinkInput, TargetSelect, DownloadButton, DownloadStatus
  lib/validation.ts      URL parsing and validation
  lib/tauri.ts           typed wrappers around the Rust commands and events
  lib/prefs.ts           local preferences (target, destination folder)
src-tauri/src/
  commands/              Tauri commands and the job queue
  download/              media, text and yt-dlp downloaders
  sources/               source registry and routing (URL → strategy)
  security/              URL checks, SSRF guard, safe file names
  error.rs               one error type with a stable code and message
```

The backend routes every link to a strategy: direct media, text extraction, or
`yt-dlp`. The frontend only ever sees three events: progress, per-item result
and job result.

## Privacy and security

* Local first: no accounts, no telemetry, no analytics, no cloud backend.
* Only the last target and the destination folder are stored (in the webview's
  local storage). Nothing about your links is kept.
* Links to `localhost`, private networks and link-local addresses are refused,
  including after DNS resolution and after redirects.
* Redirects are limited to five hops and must stay on `http(s)`.
* Downloads are size limited (2 GB) and time limited (5 minutes).
* File names are sanitised, and a download can never write outside the
  destination folder.
* `yt-dlp` is started with an explicit executable path and an argument list —
  no shell strings are ever built.
* linkFetch does not bypass paywalls, DRM, logins or access controls, and it
  never reads cookies or credentials from your browser.

## Current limitations

* Only `yt-dlp` handles video and audio; there is no built-in extractor for
  social sites that do not expose their media as a file.
* Text extraction works on server-rendered HTML. Pages that render their
  content with JavaScript, or that require a login, are reported as
  unsupported.
* `Image` and `GIF` need a direct media URL; there is no page scraping for
  image targets.
* Downloads are sequential, not parallel.
* The app is unsigned; macOS Gatekeeper will ask on first launch.

## Troubleshooting

| Message                              | Cause and fix                                                             |
| ------------------------------------ | ------------------------------------------------------------------------- |
| `yt-dlp is not installed.`           | `brew install yt-dlp`                                                     |
| `FFmpeg is not installed.`           | `brew install ffmpeg`                                                     |
| `This source is not supported yet.`  | `yt-dlp` does not know that site, or the page needs a login.              |
| `Unexpected response: expected Image but the server sent text/html` | The link points at a page, not at an image file.              |
| `links to local or private networks are not allowed` | The URL points at your own machine or a LAN address.        |
| `The download timed out.`            | The server is slow; try again or use a different link.                    |
| `The server returned HTTP 403.`      | The site refused the request (rate limit or access control).              |

## License

MIT

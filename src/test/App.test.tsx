import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "../App";

const { listeners, invokeMock } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return () => {
      listeners.delete(name);
    };
  }),
}));

function emit(name: string, payload: unknown) {
  listeners.get(name)?.({ payload });
}

const startArgs = () => invokeMock.mock.calls.find(([command]) => command === "start_download");

beforeEach(() => {
  listeners.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command: string) => {
    switch (command) {
      case "dependency_status":
        return { yt_dlp: "/usr/local/bin/yt-dlp", yt_dlp_version: "2026.01", ffmpeg: "/usr/local/bin/ffmpeg" };
      case "describe_url":
        return { host: "example.com", site: null, supported_targets: ["video", "audio", "image", "gif", "text"], unsupported: null };
      case "start_download":
        return "job-1";
      default:
        return null;
    }
  });
});

describe("linkFetch", () => {
  it("starts with a disabled download button", () => {
    render(<App />);
    expect(screen.getByRole("button", { name: "Download" })).toBeDisabled();
  });

  it("enables the button once a valid link is entered", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    expect(screen.getByRole("button", { name: "Download" })).toBeEnabled();
  });

  it("keeps the button disabled for an invalid link", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "not a url");
    expect(screen.getByRole("button", { name: "Download" })).toBeDisabled();
  });

  it("passes the selected target to the backend", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.selectOptions(screen.getByLabelText("Target"), "image");
    await user.click(screen.getByRole("button", { name: "Download" }));

    await waitFor(() => expect(startArgs()).toBeDefined());
    expect(startArgs()?.[1]).toMatchObject({
      urls: ["https://example.com/a.png"],
      target: "image",
    });
  });

  it("remembers the last used target between launches", () => {
    localStorage.setItem("linkfetch.target", "video_audio");
    render(<App />);

    expect(screen.getByLabelText("Target")).toHaveValue("video_audio");
  });

  it("offers a combined video and audio target", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.getByRole("option", { name: "Video + Audio" })).toBeInTheDocument();

    await user.type(screen.getByLabelText("Link"), "https://example.com/watch");
    await user.selectOptions(screen.getByLabelText("Target"), "video_audio");
    await user.click(screen.getByRole("button", { name: "Download" }));

    await waitFor(() => expect(startArgs()).toBeDefined());
    expect(startArgs()?.[1]).toMatchObject({
      urls: ["https://example.com/watch"],
      target: "video_audio",
    });
  });

  it("shows a loading state while the job runs and prevents a second submit", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.click(screen.getByRole("button", { name: "Download" }));

    const button = await screen.findByRole("button", { name: /Downloading/ });
    expect(button).toBeDisabled();

    await user.click(button);
    expect(invokeMock.mock.calls.filter(([command]) => command === "start_download")).toHaveLength(1);
  });

  it("renders progress and the file name while downloading", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.click(screen.getByRole("button", { name: "Download" }));

    emit("linkfetch://progress", {
      job_id: "job-1",
      index: 0,
      total: 1,
      state: "downloading",
      url: "https://example.com/a.png",
      filename: "a.png",
      percent: 42,
      message: null,
    });

    expect(await screen.findByText("42% · a.png")).toBeInTheDocument();
    const bar = document.querySelector("div[style*='width']");
    expect(bar).toHaveStyle({ width: "42%" });
  });

  it("shows the queue position for a batch", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.click(screen.getByRole("button", { name: "Download" }));

    emit("linkfetch://progress", {
      job_id: "job-1",
      index: 1,
      total: 3,
      state: "downloading",
      url: "https://example.com/b.png",
      filename: "b.png",
      percent: 10,
      message: null,
    });

    expect(await screen.findByText("10% · 2/3 · b.png")).toBeInTheDocument();
  });

  it("shows an error and offers a retry", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.click(screen.getByRole("button", { name: "Download" }));

    emit("linkfetch://item", {
      job_id: "job-1",
      index: 0,
      url: "https://example.com/a.png",
      ok: false,
      files: [],
      error: { code: "unsupported_source", message: "This source is not supported yet." },
    });

    expect(await screen.findByText("This source is not supported yet.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([command]) => command === "start_download")).toHaveLength(2),
    );
  });

  it("shows the completed file with Finder and open actions", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(screen.getByLabelText("Link"), "https://example.com/a.png");
    await user.click(screen.getByRole("button", { name: "Download" }));

    emit("linkfetch://item", {
      job_id: "job-1",
      index: 0,
      url: "https://example.com/a.png",
      ok: true,
      files: [{ path: "/Users/me/Downloads/a.png", name: "a.png", size: 10 }],
      error: null,
    });
    emit("linkfetch://job", { job_id: "job-1", total: 1, succeeded: 1, failed: 0, cancelled: false });

    await user.click(await screen.findByRole("button", { name: "Show in Finder" }));
    expect(invokeMock).toHaveBeenCalledWith("reveal_in_finder", { path: "/Users/me/Downloads/a.png" });

    await user.click(screen.getByRole("button", { name: "Open file" }));
    expect(invokeMock).toHaveBeenCalledWith("open_file", { path: "/Users/me/Downloads/a.png" });
  });

  it("explains when the selected target is not supported by the source", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "describe_url") {
        return {
          host: "instagram.com",
          site: "Instagram",
          supported_targets: ["video", "audio", "text"],
          unsupported: "Instagram does not expose direct media files. Try Video, Audio or Text.",
        };
      }
      if (command === "dependency_status") {
        return { yt_dlp: null, yt_dlp_version: null, ffmpeg: null };
      }
      return null;
    });

    render(<App />);
    await user.type(screen.getByLabelText("Link"), "https://instagram.com/p/x");
    await user.selectOptions(screen.getByLabelText("Target"), "image");

    expect(
      await screen.findByText("Instagram does not expose direct media files. Try Video, Audio or Text."),
    ).toBeInTheDocument();
  });

  it("submits with Enter for a single link", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Link");
    await user.type(input, "https://example.com/a.png{Enter}");

    await waitFor(() => expect(startArgs()).toBeDefined());
  });

  it("keeps Enter for newlines in a batch and submits with Cmd+Enter", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Link");
    await user.type(input, "https://example.com/a.png{Shift>}{Enter}{/Shift}https://example.com/b.png");
    expect(startArgs()).toBeUndefined();

    await user.keyboard("{Meta>}{Enter}{/Meta}");
    await waitFor(() => expect(startArgs()).toBeDefined());
    expect(startArgs()?.[1]).toMatchObject({
      urls: ["https://example.com/a.png", "https://example.com/b.png"],
    });
  });
});

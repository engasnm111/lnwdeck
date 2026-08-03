import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { listen } from "@tauri-apps/api/event";
import { UpdateNotification } from "./UpdateNotification";
import * as native from "../lib/native";

vi.mock("../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return { ...actual, checkForUpdate: vi.fn(), installUpdate: vi.fn() };
});

type Handler = (event: { payload: unknown }) => void;

/** Captures the event handlers the component registers. */
function captureListeners() {
  const handlers = new Map<string, Handler>();
  vi.mocked(listen).mockImplementation(
    async (event: string, handler: unknown) => {
      handlers.set(event, handler as Handler);
      return () => {};
    },
  );
  return handlers;
}

describe("UpdateNotification", () => {
  beforeEach(() => {
    vi.mocked(native.installUpdate).mockReset();
    vi.mocked(native.checkForUpdate).mockReset();
  });

  it("renders nothing until the backend reports an update", async () => {
    captureListeners();
    const { container } = render(<UpdateNotification />);
    await waitFor(() => expect(listen).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("announces an available version from the backend event", async () => {
    const handlers = captureListeners();
    render(<UpdateNotification />);
    await waitFor(() => expect(handlers.has("update-available")).toBe(true));

    handlers.get("update-available")?.({
      payload: { version: "0.2.1", notes: "quota fixes" },
    });

    await waitFor(() =>
      expect(screen.getByText("Version 0.2.1")).toBeInTheDocument(),
    );
    expect(screen.getByText(/quota fixes/)).toBeInTheDocument();
  });

  it("shows real download progress while installing", async () => {
    const handlers = captureListeners();
    vi.mocked(native.installUpdate).mockImplementation(
      () => new Promise(() => {}),
    );
    render(<UpdateNotification />);
    await waitFor(() => expect(handlers.has("update-available")).toBe(true));
    handlers.get("update-available")?.({
      payload: { version: "0.2.1", notes: null },
    });

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Install and restart" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Install and restart" }),
    );
    handlers.get("update-progress")?.({
      payload: { downloaded: 500, total: 1000 },
    });

    await waitFor(() =>
      expect(screen.getByText(/Installing version 0.2.1 \(50%\)/)).toBeInTheDocument(),
    );
  });

  it("reports a failed install instead of claiming success", async () => {
    const handlers = captureListeners();
    vi.mocked(native.installUpdate).mockRejectedValue(
      new Error("UPDATE_SIGNATURE_INVALID"),
    );
    render(<UpdateNotification />);
    await waitFor(() => expect(handlers.has("update-available")).toBe(true));
    handlers.get("update-available")?.({
      payload: { version: "0.2.1", notes: null },
    });

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Install and restart" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Install and restart" }),
    );

    await waitFor(() =>
      expect(
        screen.getByText("Update failed: UPDATE_SIGNATURE_INVALID"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("surfaces a failed background check", async () => {
    const handlers = captureListeners();
    render(<UpdateNotification />);
    await waitFor(() => expect(handlers.has("update-check-failed")).toBe(true));

    handlers.get("update-check-failed")?.({
      payload: { code: "UPDATE_ENDPOINT_UNREACHABLE" },
    });

    await waitFor(() =>
      expect(
        screen.getByText(
          /The update check did not complete \(UPDATE_ENDPOINT_UNREACHABLE\)/,
        ),
      ).toBeInTheDocument(),
    );
  });
});

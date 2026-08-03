import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UpdateView } from "./UpdateView";
import userEvent from "@testing-library/user-event";

describe("UpdateView", () => {
  it("renders check button when idle", () => {
    render(<UpdateView />);
    expect(screen.getByRole("button", { name: /check for updates/i })).toBeInTheDocument();
  });

  it("shows available state after check", async () => {
    const user = userEvent.setup();
    render(<UpdateView />);
    await user.click(screen.getByRole("button", { name: /check/i }));
    expect(screen.getByText(/new version/i)).toBeInTheDocument();
  });

  it("shows download button when available", async () => {
    const user = userEvent.setup();
    render(<UpdateView />);
    await user.click(screen.getByRole("button", { name: /check/i }));
    expect(screen.getByRole("button", { name: /download update/i })).toBeInTheDocument();
  });

  it("shows ready state after download", async () => {
    const user = userEvent.setup();
    render(<UpdateView />);
    await user.click(screen.getByRole("button", { name: /check/i }));
    await user.click(screen.getByRole("button", { name: /download/i }));
    expect(screen.getByText(/please restart/i)).toBeInTheDocument();
  });
});

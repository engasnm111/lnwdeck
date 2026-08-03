import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SettingsPage } from "./SettingsPage";

describe("SettingsPage", () => {
  it("renders settings heading", () => {
    render(<SettingsPage />);
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  it("has accessible form controls", () => {
    render(<SettingsPage />);
    expect(screen.getByLabelText("Theme")).toBeInTheDocument();
    expect(screen.getByLabelText("Auto-refresh interval")).toBeInTheDocument();
  });
});

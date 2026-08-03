import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SystemPage } from "./SystemPage";

describe("SystemPage", () => {
  it("renders system heading", () => {
    render(<SystemPage />);
    expect(screen.getByRole("heading", { name: "System" })).toBeVisible();
  });

  it("has delete data button", () => {
    render(<SystemPage />);
    expect(screen.getByRole("button", { name: "Delete all data" })).toBeInTheDocument();
  });
});

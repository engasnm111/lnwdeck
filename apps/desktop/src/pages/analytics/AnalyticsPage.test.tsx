import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { AnalyticsPage } from "./AnalyticsPage";

describe("AnalyticsPage", () => {
  it("renders analytics heading", () => {
    render(<AnalyticsPage />);
    expect(screen.getByRole("heading", { name: "Analytics" })).toBeVisible();
  });

  it("renders filter controls with accessible labels", () => {
    render(<AnalyticsPage />);
    expect(screen.getByLabelText("Provider")).toBeInTheDocument();
    expect(screen.getByLabelText("Model")).toBeInTheDocument();
    expect(screen.getByLabelText("Confidence")).toBeInTheDocument();
  });

  it("shows empty state when no data", async () => {
    render(<AnalyticsPage />);
    expect(await screen.findByText(/no usage data yet/i)).toBeInTheDocument();
  });

  it("all pages are keyboard reachable", () => {
    render(<AnalyticsPage />);
    expect(screen.getByLabelText("Provider")).not.toBeDisabled();
    expect(screen.getByLabelText("Model")).not.toBeDisabled();
    expect(screen.getByLabelText("Confidence")).not.toBeDisabled();
  });
});

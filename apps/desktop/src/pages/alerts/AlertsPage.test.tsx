import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { AlertsPage } from "./AlertsPage";

describe("AlertsPage", () => {
  it("renders alerts heading", () => {
    render(<AlertsPage />);
    expect(screen.getByRole("heading", { name: "Alerts" })).toBeVisible();
  });
});

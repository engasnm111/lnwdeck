import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { CostsPage } from "./CostsPage";

describe("CostsPage", () => {
  it("renders costs heading", () => {
    render(<CostsPage />);
    expect(screen.getByRole("heading", { name: "Costs" })).toBeVisible();
  });
});

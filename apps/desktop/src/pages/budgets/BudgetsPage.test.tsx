import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { BudgetsPage } from "./BudgetsPage";

describe("BudgetsPage", () => {
  it("renders budgets heading", () => {
    render(<BudgetsPage />);
    expect(screen.getByRole("heading", { name: "Budgets" })).toBeVisible();
  });
});

import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ModelsPage } from "./ModelsPage";

describe("ModelsPage", () => {
  it("renders models heading", () => {
    render(<ModelsPage />);
    expect(screen.getByRole("heading", { name: "Models" })).toBeVisible();
  });
});

import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import App from "./App";

describe("App", () => {
  it("renders the inwdeck product name", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "inwdeck" })).toBeVisible();
  });
});

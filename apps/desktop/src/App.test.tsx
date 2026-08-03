import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router";
import App from "./App";

describe("App", () => {
  it("renders the shell with the product name and the active page title", async () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <App />
      </MemoryRouter>,
    );
    expect(screen.getByRole("link", { name: "lnwdeck" })).toBeVisible();
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { level: 1, name: "Overview" }),
      ).toBeVisible(),
    );
  });

  it("does not route the widget or tray windows", () => {
    render(
      <MemoryRouter initialEntries={["/widget"]}>
        <App />
      </MemoryRouter>,
    );
    // The widget has its own HTML entry; the SPA must not render it.
    expect(screen.queryByLabelText("Hide widget")).not.toBeInTheDocument();
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { TokenValue } from "./TokenValue";

describe("TokenValue", () => {
  it("toggles a compact token count to its comma-grouped exact value", () => {
    render(
      <TokenValue
        value={1_234_567}
        label="Total tokens"
        exactLabel="Show exact token count"
      />,
    );

    const button = screen.getByRole("button", { name: /total tokens/i });
    expect(button).toHaveTextContent("1.2M");
    expect(button).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(button);

    expect(button).toHaveTextContent("1,234,567");
    expect(button).toHaveAttribute("aria-expanded", "true");
  });

  it("collapses the exact value with Escape", () => {
    render(
      <TokenValue
        value={1_234}
        label="Total tokens"
        exactLabel="Show exact token count"
      />,
    );
    const button = screen.getByRole("button", { name: /total tokens/i });
    fireEvent.click(button);
    expect(button).toHaveTextContent("1,234");
    fireEvent.keyDown(button, { key: "Escape" });
    expect(button).toHaveTextContent("1.2K");
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("keeps small counts as non-interactive exact values", () => {
    render(<TokenValue value={999} label="Tokens" exactLabel="Exact" />);
    expect(screen.getByText("999")).not.toHaveRole("button");
  });

  it("keeps a localized suffix attached while toggling the value", () => {
    render(
      <TokenValue
        value={1_234}
        label="tokens"
        exactLabel="1,234 tokens"
        suffix=" tokens"
      />,
    );
    const button = screen.getByRole("button", { name: "tokens: 1,234" });
    expect(button).toHaveTextContent("1.2K tokens");
    fireEvent.click(button);
    expect(button).toHaveTextContent("1,234 tokens");
  });
});

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PetMascot } from "./PetMascot";
import type { PetMood } from "./petState";

describe("PetMascot", () => {
  it("marks the mascot svg as decorative and exposes the mood as text", () => {
    const { container } = render(
      <PetMascot mood="critical" reaction={null} locked={false} />,
    );
    const svg = container.querySelector(".pet-svg");
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("focusable", "false");
    expect(screen.getByText("Quota mood: Critical")).toBeInTheDocument();
  });

  it("names every mood in the status line with plain text", () => {
    const moods: Array<[PetMood, string]> = [
      ["happy", "Happy"],
      ["worried", "Worried"],
      ["critical", "Critical"],
      ["stale", "Stale"],
      ["error", "Error"],
      ["sleeping", "Sleeping"],
    ];
    for (const [mood, label] of moods) {
      const { unmount } = render(
        <PetMascot mood={mood} reaction={null} locked={false} />,
      );
      expect(screen.getByText(`Quota mood: ${label}`)).toBeInTheDocument();
      unmount();
    }
  });

  it("applies the mood class and the celebration reaction class", () => {
    const { container, rerender } = render(
      <PetMascot mood="worried" reaction={null} locked={false} />,
    );
    const stage = container.querySelector(".pet-stage");
    expect(stage).toHaveClass("pet-mood-worried");
    expect(stage).not.toHaveClass("pet-react-celebrate");

    rerender(<PetMascot mood="worried" reaction="celebrate" locked={false} />);
    expect(container.querySelector(".pet-stage")).toHaveClass(
      "pet-react-celebrate",
    );

    rerender(<PetMascot mood="worried" reaction={null} locked={false} />);
    expect(container.querySelector(".pet-stage")).not.toHaveClass(
      "pet-react-celebrate",
    );
  });

  it("acts as a Tauri drag region only while unlocked", () => {
    const { container, rerender } = render(
      <PetMascot mood="happy" reaction={null} locked={false} />,
    );
    expect(container.querySelector(".pet-stage")).toHaveAttribute(
      "data-tauri-drag-region",
      "",
    );

    rerender(<PetMascot mood="happy" reaction={null} locked={true} />);
    expect(container.querySelector(".pet-stage")).not.toHaveAttribute(
      "data-tauri-drag-region",
    );
  });

  it("contains no interactive or focusable content", () => {
    const { container } = render(
      <PetMascot mood="happy" reaction={null} locked={false} />,
    );
    expect(
      container.querySelectorAll("button, a, input, select, [tabindex]"),
    ).toHaveLength(0);
  });
});

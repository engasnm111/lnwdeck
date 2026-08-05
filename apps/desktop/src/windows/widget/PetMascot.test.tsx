import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PetMascot } from "./PetMascot";
import type { PetMood } from "./petState";

describe("PetMascot", () => {
  it("marks the mascot svg as decorative and exposes the mood as text", () => {
    const { container } = render(
      <PetMascot mood="critical" reaction={null} locked={false} imported={null} />,
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
        <PetMascot mood={mood} reaction={null} locked={false} imported={null} />,
      );
      expect(screen.getByText(`Quota mood: ${label}`)).toBeInTheDocument();
      unmount();
    }
  });

  it("applies the mood class and the celebration reaction class", () => {
    const { container, rerender } = render(
      <PetMascot mood="worried" reaction={null} locked={false} imported={null} />,
    );
    const stage = container.querySelector(".pet-stage");
    expect(stage).toHaveClass("pet-mood-worried");
    expect(stage).not.toHaveClass("pet-react-celebrate");

    rerender(<PetMascot mood="worried" reaction="celebrate" locked={false} imported={null} />);
    expect(container.querySelector(".pet-stage")).toHaveClass(
      "pet-react-celebrate",
    );

    rerender(<PetMascot mood="worried" reaction={null} locked={false} imported={null} />);
    expect(container.querySelector(".pet-stage")).not.toHaveClass(
      "pet-react-celebrate",
    );
  });

  it("acts as a Tauri drag region only while unlocked", () => {
    const { container, rerender } = render(
      <PetMascot mood="happy" reaction={null} locked={false} imported={null} />,
    );
    expect(container.querySelector(".pet-stage")).toHaveAttribute(
      "data-tauri-drag-region",
      "",
    );

    rerender(<PetMascot mood="happy" reaction={null} locked={true} imported={null} />);
    expect(container.querySelector(".pet-stage")).not.toHaveAttribute(
      "data-tauri-drag-region",
    );
  });

  it("contains no interactive or focusable content", () => {
    const { container } = render(
      <PetMascot mood="happy" reaction={null} locked={false} imported={null} />,
    );
    expect(
      container.querySelectorAll("button, a, input, select, [tabindex]"),
    ).toHaveLength(0);
  });

  it("renders an imported pet atlas from the local petlocal store", () => {
    const { container } = render(
      <PetMascot
        mood="happy"
        reaction={null}
        locked={false}
        imported={{
          id: "sprout",
          displayName: "Sprout",
          spriteVersionNumber: 2,
        }}
      />,
    );
    const atlas = container.querySelector(".pet-atlas");
    expect(atlas).not.toBeNull();
    expect(atlas).toHaveAttribute("aria-hidden", "true");
    expect(atlas).toHaveAttribute("data-sprite-version", "2");
    expect((atlas as HTMLElement).style.backgroundImage).toContain(
      "petlocal://pets/sprout/spritesheet.webp",
    );
    expect(container.querySelector(".pet-svg")).toBeNull();
    expect(screen.getByText("Quota mood: Happy")).toBeInTheDocument();
  });

  it("keeps the built-in robot when no pet is imported", () => {
    const { container } = render(
      <PetMascot mood="sleeping" reaction={null} locked={false} imported={null} />,
    );
    expect(container.querySelector(".pet-svg")).not.toBeNull();
    expect(container.querySelector(".pet-atlas")).toBeNull();
  });
});

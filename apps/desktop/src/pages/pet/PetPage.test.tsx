import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as native from "../../lib/native";
import { PetPage } from "./PetPage";

vi.mock("../../lib/native", () => ({
  fetchPetWindowSettings: vi.fn(),
  listWidgetPets: vi.fn(),
  showPetWindow: vi.fn(),
  hidePetWindow: vi.fn(),
  setPetCharacter: vi.fn(),
  setPetSpeed: vi.fn(),
  setPetOpacity: vi.fn(),
  setPetAutoSleep: vi.fn(),
  setPetSizePreset: vi.fn(),
  setPetStayInPlace: vi.fn(),
  setPetPose: vi.fn(),
  importWidgetPet: vi.fn(),
  removeWidgetPet: vi.fn(),
}));

const petSettings = (overrides: Partial<native.PetWindowSettingsData> = {}) => ({
  visible: false,
  character: "youyou",
  speed: "normal",
  opacity: 1,
  autoSleep: true,
  sizePreset: "medium" as native.PetSizePreset,
  stayInPlace: false,
  poseWave: true,
  poseJump: true,
  poseLookLeft: true,
  poseLookRight: true,
  poseWaiting: true,
  poseReview: true,
  ...overrides,
});

const manifest = (overrides: Partial<native.PetManifest> = {}) => ({
  id: "youyou",
  displayName: "Youyou",
  description: "A round cat",
  spritesheetPath: "spritesheet.webp",
  spriteVersionNumber: 2,
  ...overrides,
});

describe("PetPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchPetWindowSettings).mockReset();
    vi.mocked(native.listWidgetPets).mockReset();
    vi.mocked(native.showPetWindow).mockReset();
    vi.mocked(native.hidePetWindow).mockReset();
    vi.mocked(native.setPetCharacter).mockReset();
    vi.mocked(native.setPetSpeed).mockReset();
    vi.mocked(native.setPetOpacity).mockReset();
    vi.mocked(native.setPetAutoSleep).mockReset();
    vi.mocked(native.setPetSizePreset).mockReset();
    vi.mocked(native.setPetStayInPlace).mockReset();
    vi.mocked(native.setPetPose).mockReset();
    vi.mocked(native.setPetSizePreset).mockResolvedValue("medium");
    vi.mocked(native.importWidgetPet).mockReset();
    vi.mocked(native.removeWidgetPet).mockReset();

    vi.mocked(native.fetchPetWindowSettings).mockResolvedValue(petSettings());
    vi.mocked(native.listWidgetPets).mockResolvedValue([
      manifest(),
      manifest({
        id: "sharkler",
        displayName: "Sharkler",
        description: "Robot shark",
      }),
    ]);
  });

  it("renders the stored pet state, not defaults", async () => {
    render(<PetPage />);

    await waitFor(() => {
      expect(screen.getByLabelText("Show desktop pet")).toBeInTheDocument();
    });

    const toggle = screen.getByLabelText("Show desktop pet") as HTMLInputElement;
    expect(toggle.checked).toBe(false);
    expect(
      (screen.getByLabelText("Walk speed") as HTMLSelectElement).value,
    ).toBe("normal");
    expect(screen.getByText("Youyou")).toBeInTheDocument();
    expect(screen.getByText("Sharkler")).toBeInTheDocument();
    expect(screen.getAllByText("Active").length).toBeGreaterThan(0);
  });

  it("shows the pet through the backend and reports the stored state", async () => {
    vi.mocked(native.hidePetWindow).mockResolvedValue(undefined);
    vi.mocked(native.showPetWindow).mockResolvedValue(undefined);
    vi.mocked(native.fetchPetWindowSettings).mockResolvedValueOnce(
      petSettings(),
    );

    render(<PetPage />);
    const toggle = await screen.findByLabelText("Show desktop pet");
    await userEvent.click(toggle);

    await waitFor(() => {
      expect(native.showPetWindow).toHaveBeenCalledOnce();
    });
  });

  it("selects a character through the backend", async () => {
    vi.mocked(native.setPetCharacter).mockResolvedValue("sharkler");
    vi.mocked(native.fetchPetWindowSettings).mockResolvedValueOnce(
      petSettings(),
    );

    render(<PetPage />);
    const useButton = await screen.findByRole("button", { name: "Use" });
    await userEvent.click(useButton);

    await waitFor(() => {
      expect(native.setPetCharacter).toHaveBeenCalledWith("sharkler");
    });
  });

  it("persists speed and auto-sleep changes through the backend", async () => {
    vi.mocked(native.setPetSpeed).mockResolvedValue("fast");
    vi.mocked(native.setPetAutoSleep).mockResolvedValue(false);

    render(<PetPage />);
    const speed = await screen.findByLabelText("Walk speed");
    await userEvent.selectOptions(speed, "fast");

    await waitFor(() => {
      expect(native.setPetSpeed).toHaveBeenCalledWith("fast");
    });

    const sleep = screen.getByLabelText("Auto-sleep after inactivity");
    await userEvent.click(sleep);

    await waitFor(() => {
      expect(native.setPetAutoSleep).toHaveBeenCalledWith(false);
    });
  });

  it("persists stay-in-place and pose toggles through the backend", async () => {
    vi.mocked(native.setPetStayInPlace).mockResolvedValue(true);
    vi.mocked(native.setPetPose).mockResolvedValue(false);

    render(<PetPage />);
    const stay = await screen.findByLabelText("Stay in place");
    await userEvent.click(stay);

    await waitFor(() => {
      expect(native.setPetStayInPlace).toHaveBeenCalledWith(true);
    });

    const jump = screen.getByLabelText("Jump");
    await userEvent.click(jump);

    await waitFor(() => {
      expect(native.setPetPose).toHaveBeenCalledWith("pet_pose_jump", false);
    });
  });

  it("re-renders the pose toggle from the echoed backend value", async () => {
    vi.mocked(native.setPetPose).mockResolvedValue(false);
    vi.mocked(native.fetchPetWindowSettings)
      .mockResolvedValueOnce(petSettings())
      .mockResolvedValueOnce(petSettings({ poseJump: false }));

    render(<PetPage />);
    const jump = await screen.findByLabelText("Jump");
    expect(jump).toBeChecked();

    await userEvent.click(jump);

    await waitFor(() => {
      expect(screen.getByLabelText("Jump")).not.toBeChecked();
    });
  });

  it("imports a community pet and refreshes the roster", async () => {
    vi.mocked(native.importWidgetPet).mockResolvedValue(
      manifest({ id: "solaire", displayName: "Solaire" }),
    );

    render(<PetPage />);
    const input = await screen.findByLabelText("Official Codex Pets URL");
    await userEvent.type(input, "https://codex-pets.net/#/pets/solaire");
    await userEvent.click(screen.getByRole("button", { name: "Import pet" }));

    await waitFor(() => {
      expect(native.importWidgetPet).toHaveBeenCalledWith(
        "https://codex-pets.net/#/pets/solaire",
      );
    });
    await waitFor(() => {
      expect(native.listWidgetPets).toHaveBeenCalledTimes(2);
    });
  });

  it("reports a refused import instead of hiding the failure", async () => {
    vi.mocked(native.importWidgetPet).mockRejectedValue(
      new Error("not a valid pet id"),
    );

    render(<PetPage />);
    const input = await screen.findByLabelText("Official Codex Pets URL");
    await userEvent.type(input, "https://evil.example/pets/bad-pet");
    await userEvent.click(screen.getByRole("button", { name: "Import pet" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        "not a valid pet id",
      );
    });
  });

  it("removes an installed pet and refreshes the roster", async () => {
    vi.mocked(native.removeWidgetPet).mockResolvedValue(undefined);

    render(<PetPage />);
    const removeButtons = await screen.findAllByRole("button", {
      name: "Remove",
    });
    await userEvent.click(removeButtons[0]);

    await waitFor(() => {
      expect(native.removeWidgetPet).toHaveBeenCalledWith("youyou");
    });
  });
});

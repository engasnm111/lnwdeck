import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SessionsPage } from "./SessionsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchSessions: vi.fn(),
    renameSession: vi.fn(),
    renameProject: vi.fn(),
  };
});

const overview = (
  overrides: Partial<native.SessionsOverview> = {},
): native.SessionsOverview => ({
  window: "last_7d",
  generated_at: "2026-08-04T00:00:00Z",
  since: "2026-07-28T00:00:00Z",
  request_count: 4,
  tokens_input: 850,
  tokens_output: 250,
  cost: "0.110000",
  projects: [
    {
      project_hash: "p2",
      display_name: "lnwdeck",
      request_count: 1,
      tokens_input: 500,
      tokens_output: 100,
      cost: "0.100000",
      first_seen_at: "2026-08-01T00:00:00Z",
      last_seen_at: "2026-08-04T00:00:00Z",
      sessions: [
        {
          session_hash: "s3",
          display_name: "fix dropdown",
          provider_id: "claude",
          request_count: 1,
          tokens_input: 500,
          tokens_output: 100,
          cost: "0.100000",
          first_seen_at: "2026-08-01T00:00:00Z",
          last_seen_at: "2026-08-04T00:00:00Z",
        },
      ],
    },
    {
      project_hash: "p1",
      display_name: "Project 02",
      request_count: 3,
      tokens_input: 350,
      tokens_output: 150,
      cost: "0.010000",
      first_seen_at: "2026-08-02T00:00:00Z",
      last_seen_at: "2026-08-03T00:00:00Z",
      sessions: [
        {
          session_hash: "s2",
          display_name: "Session 01",
          provider_id: "opencode",
          request_count: 2,
          tokens_input: 280,
          tokens_output: 100,
          cost: "0.008000",
          first_seen_at: "2026-08-02T00:00:00Z",
          last_seen_at: "2026-08-03T00:00:00Z",
        },
        {
          session_hash: "s1",
          display_name: "Session 02",
          provider_id: "opencode",
          request_count: 1,
          tokens_input: 70,
          tokens_output: 50,
          cost: "0.002000",
          first_seen_at: "2026-08-02T00:00:00Z",
          last_seen_at: "2026-08-02T00:00:00Z",
        },
      ],
    },
  ],
  providers: ["claude", "opencode"],
  ...overrides,
});

describe("SessionsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchSessions).mockReset();
    vi.mocked(native.renameSession).mockReset();
    vi.mocked(native.renameProject).mockReset();
  });

  it("renders projects and their sessions with usage totals", async () => {
    vi.mocked(native.fetchSessions).mockResolvedValue(overview());
    render(<SessionsPage />);

    await waitFor(() => expect(screen.getByText("lnwdeck")).toBeInTheDocument());
    expect(screen.getByText("Project 02")).toBeInTheDocument();
    expect(screen.getByText("fix dropdown")).toBeInTheDocument();
    expect(screen.getByText("Session 01")).toBeInTheDocument();
    expect(screen.getByText("Session 02")).toBeInTheDocument();
    expect(screen.getByText("0.100000")).toBeInTheDocument();
  });

  it("labels the unassigned bucket and groups unassigned sessions", async () => {
    vi.mocked(native.fetchSessions).mockResolvedValue(
      overview({
        projects: [
          {
            project_hash: "",
            display_name: "",
            request_count: 2,
            tokens_input: 10,
            tokens_output: 5,
            cost: "0.001000",
            first_seen_at: null,
            last_seen_at: null,
            sessions: [
              {
                session_hash: "",
                display_name: "",
                provider_id: "copilot",
                request_count: 2,
                tokens_input: 10,
                tokens_output: 5,
                cost: "0.001000",
                first_seen_at: null,
                last_seen_at: null,
              },
            ],
          },
        ],
      }),
    );
    render(<SessionsPage />);

    await waitFor(() =>
      expect(screen.getByText("Unassigned")).toBeInTheDocument(),
    );
  });

  it("renames a project and reloads", async () => {
    vi.mocked(native.fetchSessions).mockResolvedValue(overview());
    vi.mocked(native.renameProject).mockResolvedValue(undefined);
    render(<SessionsPage />);

    await waitFor(() => expect(screen.getByText("lnwdeck")).toBeInTheDocument());
    await userEvent.click(screen.getAllByLabelText(/rename project/i)[0]);
    const input = screen.getByRole("textbox", { name: /rename project/i });
    await userEvent.clear(input);
    await userEvent.type(input, "tracker-v2{enter}");

    await waitFor(() =>
      expect(native.renameProject).toHaveBeenCalledWith("p2", "tracker-v2"),
    );
    expect(native.fetchSessions).toHaveBeenCalledTimes(2);
  });

  it("renames a session and reloads", async () => {
    vi.mocked(native.fetchSessions).mockResolvedValue(overview());
    vi.mocked(native.renameSession).mockResolvedValue(undefined);
    render(<SessionsPage />);

    await waitFor(() => expect(screen.getByText("fix dropdown")).toBeInTheDocument());
    await userEvent.click(screen.getAllByLabelText(/rename session/i)[0]);
    const input = screen.getByRole("textbox", { name: /rename session/i });
    await userEvent.clear(input);
    await userEvent.type(input, "dark-mode-fix{enter}");

    await waitFor(() =>
      expect(native.renameSession).toHaveBeenCalledWith("s3", "dark-mode-fix"),
    );
    expect(native.fetchSessions).toHaveBeenCalledTimes(2);
  });

  it("shows an empty state when no sessions exist", async () => {
    vi.mocked(native.fetchSessions).mockResolvedValue(
      overview({ projects: [], request_count: 0, tokens_input: 0, tokens_output: 0, cost: "0.000000" }),
    );
    render(<SessionsPage />);

    await waitFor(() =>
      expect(screen.getByText("No sessions recorded")).toBeInTheDocument(),
    );
  });

  it("surfaces a backend failure instead of rendering a table", async () => {
    vi.mocked(native.fetchSessions).mockRejectedValue(new Error("sessions: db locked"));
    render(<SessionsPage />);

    await waitFor(() =>
      expect(screen.getByText("sessions: db locked")).toBeInTheDocument(),
    );
  });
});

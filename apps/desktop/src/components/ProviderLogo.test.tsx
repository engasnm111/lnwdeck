import { describe, expect, it } from "vitest";
import { modelDisplayName, providerDisplayName } from "./ProviderLogo";

describe("display names", () => {
  it("canonicalizes provider ids and removes storage source suffixes", () => {
    expect(providerDisplayName({ provider_id: "opencode", display_name: "Opencode" })).toBe("OpenCode (Go)");
    expect(providerDisplayName({ provider_id: "opencode_cli", display_name: "OpenCode" })).toBe("OpenCode (Free)");
    expect(
      providerDisplayName({
        provider_id: "opencode",
        display_name: "OpenCode - local_sqlite",
      }),
    ).toBe("OpenCode (Go)");
    expect(providerDisplayName({ provider_id: "openai_codex", display_name: "openai_codex" })).toBe("OpenAI Codex");
  });

  it("turns unknown and source-suffixed model labels into user-facing text", () => {
    expect(modelDisplayName("unknown")).toBe("Unknown model");
    expect(modelDisplayName("OpenCode - local_sqlite")).toBe("OpenCode");
  });
});

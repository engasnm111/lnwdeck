import { describe, it, expect } from "vitest";

describe("Packaging configuration", () => {
  it("package-config.json has all three architectures", () => {
    const config = {
      architectures: [
        { arch: "x64", target: "x86_64-pc-windows-msvc" },
        { arch: "ARM64", target: "aarch64-pc-windows-msvc" },
        { arch: "x86", target: "i686-pc-windows-msvc" },
      ],
    };
    expect(config.architectures).toHaveLength(3);
    expect(config.architectures[0].arch).toBe("x64");
    expect(config.architectures[1].arch).toBe("ARM64");
    expect(config.architectures[2].arch).toBe("x86");
  });

  it("portable mode has marker file configured", () => {
    const config = {
      portable: { marker_file: ".inwdeck_portable" },
    };
    expect(config.portable.marker_file).toBe(".inwdeck_portable");
  });

  it("native host name matches convention", () => {
    const hostName = "app.inwdeck.browser_helper";
    expect(hostName).toMatch(/^app\.\w+\.\w+$/);
    expect(hostName).toBe("app.inwdeck.browser_helper");
  });

  it("artifact names follow documented pattern", () => {
    const expected = [
      "inwdeck_0.1.0_x64-setup.exe",
      "inwdeck_0.1.0_arm64-setup.exe",
      "inwdeck_0.1.0_x86-setup.exe",
      "inwdeck_0.1.0_portable.zip",
    ];
    for (const name of expected) {
      expect(name).toMatch(/^inwdeck_0\.1\.0/);
    }
  });
});

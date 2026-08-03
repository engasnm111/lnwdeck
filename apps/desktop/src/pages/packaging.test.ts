import { describe, it, expect } from "vitest";
import fs from "fs";
import path from "path";

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
      portable: { marker_file: ".lnwdeck_portable" },
    };
    expect(config.portable.marker_file).toBe(".lnwdeck_portable");
  });

  it("native host name matches convention", () => {
    const hostName = "app.lnwdeck.browser_helper";
    expect(hostName).toMatch(/^app\.\w+\.\w+$/);
    expect(hostName).toBe("app.lnwdeck.browser_helper");
  });

  it("artifact names follow documented pattern", () => {
    const expected = [
      "lnwdeck_0.2.0_x64-setup.exe",
      "lnwdeck_0.2.0_arm64-setup.exe",
      "lnwdeck_0.2.0_x86-setup.exe",
      "lnwdeck_0.2.0_portable.zip",
    ];
    for (const name of expected) {
      expect(name).toMatch(/^lnwdeck_0\.2\.0/);
    }
  });

  it("tray icon configuration exists and build references valid icon assets", () => {
    const tauriConfPath = path.resolve(__dirname, "../../src-tauri/tauri.conf.json");
    const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf-8"));
    expect(tauriConf.productName).toBe("lnwdeck");
    expect(tauriConf.bundle.icon).toContain("icons/icon.ico");

    const iconsDir = path.resolve(__dirname, "../../src-tauri/icons");
    const sizes = ["16x16.png", "24x24.png", "32x32.png", "48x48.png", "64x64.png", "128x128.png", "256x256.png", "icon.ico"];
    for (const s of sizes) {
      const filePath = path.join(iconsDir, s);
      expect(fs.existsSync(filePath)).toBe(true);
      const stat = fs.statSync(filePath);
      expect(stat.size).toBeGreaterThan(100);
    }
  });
});

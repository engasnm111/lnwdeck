import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

/**
 * Packaging configuration checks.
 *
 * Every assertion reads a real file from the repository. The previous version of
 * this suite declared the expected configuration inside the test and asserted
 * against its own literal, so it could never fail.
 */

const REPO_ROOT = path.resolve(__dirname, "../../../..");
const DESKTOP_ROOT = path.resolve(__dirname, "../..");

function readJson(relative: string): Record<string, unknown> {
  const file = path.join(REPO_ROOT, relative);
  return JSON.parse(fs.readFileSync(file, "utf-8"));
}

describe("packaging configuration", () => {
  const tauriConf = readJson("apps/desktop/src-tauri/tauri.conf.json") as {
    productName: string;
    version: string;
    identifier: string;
    bundle: { icon: string[]; createUpdaterArtifacts: boolean; targets: string };
    plugins: { updater: { endpoints: string[]; pubkey: string } };
  };
  const installer = readJson("installer/package-config.json") as {
    version: string;
    architectures?: Array<{ arch: string; target: string }>;
    portable?: { marker_file?: string };
  };

  it("declares the same version everywhere a release reads it", () => {
    const cargoToml = fs.readFileSync(
      path.join(REPO_ROOT, "apps/desktop/src-tauri/Cargo.toml"),
      "utf-8",
    );
    const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargoToml)?.[1];

    expect(cargoVersion).toBeDefined();
    expect(tauriConf.version).toBe(cargoVersion);
    expect(installer.version).toBe(cargoVersion);

    const packagePortable = fs.readFileSync(
      path.join(REPO_ROOT, "scripts/package-portable.ps1"),
      "utf-8",
    );
    // The portable script names the archive, so a hardcoded default would
    // ship a mislabelled ZIP on the next version bump. It must derive the
    // version from the canonical installer/package-config.json instead.
    expect(packagePortable).toContain("installer/package-config.json");
    expect(packagePortable).toContain(
      '$OutputFile = "lnwdeck_${Version}_portable.zip"',
    );
    expect(packagePortable).not.toMatch(/\$Version = "\d+\.\d+\.\d+"/);
  });

  it("configures the three Windows architectures the release builds", () => {
    const workflow = fs.readFileSync(
      path.join(REPO_ROOT, ".github/workflows/release.yml"),
      "utf-8",
    );
    for (const target of [
      "x86_64-pc-windows-msvc",
      "aarch64-pc-windows-msvc",
      "i686-pc-windows-msvc",
    ]) {
      expect(workflow).toContain(target);
    }

    const architectures = installer.architectures ?? [];
    expect(architectures.map((entry) => entry.target)).toEqual([
      "x86_64-pc-windows-msvc",
      "aarch64-pc-windows-msvc",
      "i686-pc-windows-msvc",
    ]);
  });

  it("enables signed updater artifacts and a real endpoint", () => {
    expect(tauriConf.bundle.createUpdaterArtifacts).toBe(true);
    expect(tauriConf.plugins.updater.endpoints.length).toBeGreaterThan(0);
    for (const endpoint of tauriConf.plugins.updater.endpoints) {
      expect(endpoint.startsWith("https://")).toBe(true);
    }
    // The public key must be present and non-trivial, otherwise no signature
    // could ever be verified.
    expect(tauriConf.plugins.updater.pubkey.length).toBeGreaterThan(40);
  });

  it("ships every icon the bundle configuration references", () => {
    for (const icon of tauriConf.bundle.icon) {
      const file = path.join(DESKTOP_ROOT, "src-tauri", icon);
      expect(fs.existsSync(file), `${icon} must exist`).toBe(true);
      expect(fs.statSync(file).size).toBeGreaterThan(100);
    }
    // The tray needs the small sizes too.
    for (const size of ["16x16.png", "24x24.png", "32x32.png", "48x48.png"]) {
      const file = path.join(DESKTOP_ROOT, "src-tauri/icons", size);
      expect(fs.existsSync(file), `${size} must exist`).toBe(true);
    }
  });

  it("declares a capability for every window the app creates", () => {
    const capabilityDir = path.join(DESKTOP_ROOT, "src-tauri/capabilities");
    const files = fs
      .readdirSync(capabilityDir)
      .filter((name) => name.endsWith(".json"));
    expect(files.length).toBeGreaterThanOrEqual(2);

    const windows = new Set<string>();
    for (const name of files) {
      const capability = JSON.parse(
        fs.readFileSync(path.join(capabilityDir, name), "utf-8"),
      ) as { windows: string[]; permissions: string[] };
      for (const label of capability.windows) {
        windows.add(label);
      }
      expect(capability.permissions.length).toBeGreaterThan(0);
    }
    // Without these the webview cannot listen for backend events.
    expect(windows.has("main")).toBe(true);
    expect(windows.has("widget")).toBe(true);
  });

  it("keeps both window entry points in the built frontend", () => {
    const viteConfig = fs.readFileSync(
      path.join(DESKTOP_ROOT, "vite.config.ts"),
      "utf-8",
    );
    expect(viteConfig).toContain("widget.html");
    expect(fs.existsSync(path.join(DESKTOP_ROOT, "index.html"))).toBe(true);
    expect(fs.existsSync(path.join(DESKTOP_ROOT, "widget.html"))).toBe(true);
  });

  it("names the portable marker used to detect a portable install", () => {
    expect(installer.portable?.marker_file).toBe(".lnwdeck_portable");
    const packagePortable = fs.readFileSync(
      path.join(REPO_ROOT, "scripts/package-portable.ps1"),
      "utf-8",
    );
    expect(packagePortable).toContain(".lnwdeck_portable");
  });
});

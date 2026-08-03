import { writeFileSync } from "node:fs";

const version = process.env.GITHUB_REF_NAME?.replace(/^v/, "") ?? "0.0.0";
const releaseDate = new Date().toISOString().split("T")[0];
const baseUrl = `https://releases.inwdeck.app/${version}`;

const manifest = {
  version,
  release_date: releaseDate,
  artifacts: [
    {
      target: "x86_64-pc-windows-msvc",
      url: `${baseUrl}/inwdeck_${version}_x64-setup.exe`,
      sha256: "",
      signature: "",
      size_bytes: 0,
    },
    {
      target: "aarch64-pc-windows-msvc",
      url: `${baseUrl}/inwdeck_${version}_arm64-setup.exe`,
      sha256: "",
      signature: "",
      size_bytes: 0,
    },
    {
      target: "i686-pc-windows-msvc",
      url: `${baseUrl}/inwdeck_${version}_x86-setup.exe`,
      sha256: "",
      signature: "",
      size_bytes: 0,
    },
  ],
};

writeFileSync("latest.json", JSON.stringify(manifest, null, 2));
console.log(`Generated latest.json for version ${version}`);

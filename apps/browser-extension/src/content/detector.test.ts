import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const detectorPath = resolve(dirname(fileURLToPath(import.meta.url)), "detector.ts");
const content = readFileSync(detectorPath, "utf-8");

if (!content.includes("detectQuota")) {
    throw new Error("detector.ts must define detectQuota function");
}

if (!content.includes("chrome.runtime.sendMessage")) {
    throw new Error("detector.ts must call chrome.runtime.sendMessage");
}

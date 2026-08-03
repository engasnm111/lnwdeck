import { readFileSync, readdirSync, statSync } from "node:fs";
import { resolve, extname } from "node:path";

const FORBIDDEN_PATTERNS = [
  { name: "bearer_token", regex: /Bearer\s+[A-Za-z0-9\-._~+/]+=*/gi },
  { name: "api_key_sk", regex: /sk-[A-Za-z0-9]{16,}/g },
  { name: "windows_path_users", regex: /[A-Z]:\\Users\\/gi },
  { name: "unix_path_home", regex: /\/home\/\w+/g },
  { name: "generic_api_key", regex: /api_key\s*[:=]\s*['"]?\w{8,}['"]?/gi },
  { name: "jwt_token", regex: /eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g },
  { name: "private_key_header", regex: /-----BEGIN (RSA )?PRIVATE KEY-----/ },
  { name: "aws_access_key", regex: /AKIA[0-9A-Z]{16}/g },
  { name: "generic_password", regex: /password\s*[:=]\s*['"]\S+['"]/gi },
  { name: "connection_string", regex: /Server=\w+;Database=\w+;/gi },
];

const SCAN_EXTENSIONS = new Set([
  ".sqlite", ".db", ".json", ".log", ".txt", ".csv", ".ts", ".tsx", ".js", ".rs", ".toml"
]);

function* walkDir(dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = resolve(dir, entry.name);
    if (entry.isDirectory() && !entry.name.startsWith(".") && entry.name !== "node_modules" && entry.name !== "target" && entry.name !== "tests") {
      yield* walkDir(fullPath);
    } else if (entry.isFile() && SCAN_EXTENSIONS.has(extname(entry.name).toLowerCase())) {
      if (fullPath.includes("/tests/") || fullPath.includes("\\tests\\") || entry.name.endsWith(".test.ts") || entry.name.endsWith(".test.tsx") || entry.name.endsWith(".spec.ts")) {
        continue;
      }
      yield fullPath;
    }
  }
}

function scanFile(filePath) {
  const violations = [];
  try {
    const content = readFileSync(filePath, "utf-8");
    for (const pattern of FORBIDDEN_PATTERNS) {
      const matches = content.match(pattern.regex);
      if (matches) {
        violations.push({
          file: filePath,
          pattern: pattern.name,
          count: matches.length,
          sample: matches[0].substring(0, 80),
        });
      }
    }
  } catch {
    // skip binary/unreadable files
  }
  return violations;
}

function main() {
  const scanRoots = [
    resolve(process.cwd(), "crates/domain"),
    resolve(process.cwd(), "crates/application"),
    resolve(process.cwd(), "crates/storage"),
    resolve(process.cwd(), "crates/pricing"),
    resolve(process.cwd(), "crates/analytics"),
    resolve(process.cwd(), "crates/hook-manager"),
    resolve(process.cwd(), "crates/provider-runtime"),
    resolve(process.cwd(), "crates/providers"),
    resolve(process.cwd(), "apps"),
    resolve(process.cwd(), "packages"),
    resolve(process.cwd(), "schemas"),
    resolve(process.cwd(), "assets"),
    resolve(process.cwd(), "scripts"),
    resolve(process.cwd(), "e2e"),
  ];

  const allViolations = [];
  for (const root of scanRoots) {
    try {
      for (const filePath of walkDir(root)) {
        allViolations.push(...scanFile(filePath));
      }
    } catch {
      // directory may not exist yet
    }
  }

  if (allViolations.length > 0) {
    console.error(`\n=== PRIVACY SCAN FAILED: ${allViolations.length} violations found ===\n`);
    for (const v of allViolations) {
      console.error(`  [${v.pattern}] ${v.file} (${v.count} matches, e.g. "${v.sample}")`);
    }
    console.error("\n=== SCAN FAILED ===\n");
    process.exit(1);
  }

  console.log("=== Privacy scan passed: no forbidden patterns detected ===");
}

main();

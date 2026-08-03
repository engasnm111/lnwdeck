import { execSync } from "node:child_process";

const run = (label, command) => {
  process.stdout.write(`${label}... `);
  try {
    execSync(command, { stdio: "pipe", shell: true });
    console.log("OK");
  } catch (err) {
    console.log("FAIL");
    process.stderr.write(err.stderr?.toString() ?? err.message ?? String(err));
    process.exitCode = 1;
  }
};

run("cargo fmt --check", "cargo fmt --check");
run("cargo clippy", 'cargo clippy --workspace --all-targets --all-features -- -D warnings');
run("pnpm typecheck", "pnpm -r run typecheck");

if (process.exitCode) {
  process.exit(1);
}

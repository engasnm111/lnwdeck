import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    include: ["e2e/**/*.spec.ts"],
    testTimeout: 900_000,
    hookTimeout: 900_000,
  },
});

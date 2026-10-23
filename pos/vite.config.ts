import { defineConfig } from "vitest/config";

declare const process: { env: Record<string, string | undefined> };

// ヘッダーのビルド番号：GitHub Actions の実行番号（手元のビルドは dev）
const build = process.env.GITHUB_RUN_NUMBER ?? process.env.JANUS_BUILD ?? "dev";

export default defineConfig({
  root: "web",
  base: "./",
  define: {
    "import.meta.env.VITE_BUILD": JSON.stringify(build),
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2022",
  },
  test: {
    root: ".",
    include: ["web/src/**/*.test.ts"],
  },
});

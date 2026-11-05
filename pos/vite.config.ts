import { defineConfig } from "vitest/config";

const stamp = new Intl.DateTimeFormat("en-CA", {
  timeZone: "Asia/Tokyo",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})
  .formatToParts(new Date())
  .filter((p) => p.type !== "literal")
  .map((p) => p.value)
  .join("");

export default defineConfig({
  root: "web",
  base: "./",
  define: {
    "import.meta.env.VITE_BUILD": JSON.stringify(`build:${stamp}`),
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

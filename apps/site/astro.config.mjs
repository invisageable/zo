// @ts-check
import { defineConfig } from "astro/config";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import vercel from "@astrojs/vercel";
import { remarkZo } from "./src/core/lang/remark-zo.ts";
import { remarkSpeechNav } from "./src/core/lang/remark-speech-nav.ts";
import { remarkShiftHeadings } from "./src/core/lang/remark-shift-headings.ts";

// https://astro.build/config
export default defineConfig({
  output: "server",
  adapter: vercel(),
  markdown: {
    remarkPlugins: [remarkZo, remarkSpeechNav, remarkShiftHeadings],
  },
  vite: {
    envDir: "../..",
    plugins: [
      paraglideVitePlugin({
        project: "./project.inlang",
        outdir: "./src/paraglide",
        strategy: ["cookie", "preferredLanguage", "baseLocale"],
      }),
    ],
  },
});

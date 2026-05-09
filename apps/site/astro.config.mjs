// @ts-check
import { defineConfig } from "astro/config";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import vercel from "@astrojs/vercel";
import sitemap from "@astrojs/sitemap";
import { remarkZo } from "./src/core/lang/remark-zo.ts";
import { remarkNewsNav } from "./src/core/lang/remark-news-nav.ts";
import { remarkShiftHeadings } from "./src/core/lang/remark-shift-headings.ts";

// https://astro.build/config
export default defineConfig({
  site: "https://zo.compilords.house",
  output: "server",
  adapter: vercel(),
  integrations: [sitemap()],
  markdown: {
    remarkPlugins: [remarkZo, remarkNewsNav, remarkShiftHeadings],
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

// @ts-check
import { defineConfig } from "astro/config";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import { remarkZo } from "./src/core/lang/remark-zo.ts";
import { remarkSpeechNav } from "./src/core/lang/remark-speech-nav.ts";

// https://astro.build/config
export default defineConfig({
  i18n: {
    defaultLocale: "en",
    locales: ["de", "en", "fr", "zh", "ja"],
    routing: { prefixDefaultLocale: false },
  },
  markdown: {
    remarkPlugins: [remarkZo, remarkSpeechNav],
  },
  vite: {
    envDir: "../..",
    plugins: [
      paraglideVitePlugin({
        project: "./project.inlang",
        outdir: "./src/paraglide",
      }),
    ],
  },
});

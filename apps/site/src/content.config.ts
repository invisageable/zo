import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

// Multi-locale layout: every entry lives under <locale>/.
// Entry `id` becomes "<locale>/<slug>", which downstream code splits to
// match the active request locale (with fallback to baseLocale).
const news = defineCollection({
  loader: glob({
    pattern: "*/S0*/S0*.md",
    base: "./src/content/news",
  }),
  schema: z.object({
    title: z.string().optional(),
    subtitle: z.string().optional(),
  }),
});

const initiation = defineCollection({
  loader: glob({
    pattern: "*/*.md",
    base: "./src/content/initiation",
  }),
  schema: z.object({
    title: z.string().optional(),
    order: z.number().optional(),
  }),
});

export const collections = { news, initiation };

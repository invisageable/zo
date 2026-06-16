import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

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

const spec = defineCollection({
  loader: glob({
    pattern: "*/*.md",
    base: "./src/content/spec",
  }),
  schema: z.object({
    title: z.string().optional(),
  }),
});

const faq = defineCollection({
  loader: glob({
    pattern: "*/*.md",
    base: "./src/content/faq",
  }),
  schema: z.object({
    title: z.string().optional(),
  }),
});

const howto = defineCollection({
  loader: glob({
    pattern: "**/*.md",
    base: "./src/content/how-to",
  }),
  schema: z.object({
    category: z.string(),
    group: z.string().optional(),
    title: z.string().optional(),
    order: z.number().optional(),
    code: z.string(),
  }),
});

export const collections = { news, initiation, spec, faq, howto };

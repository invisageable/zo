import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const speeches = defineCollection({
  loader: glob({
    pattern: "S0*/S0*.md",
    base: "./src/content/speeches",
  }),
  schema: z.object({
    title: z.string().optional(),
    subtitle: z.string().optional(),
  }),
});

const initiation = defineCollection({
  loader: glob({
    pattern: "*.md",
    base: "./src/content/initiation",
  }),
  schema: z.object({
    title: z.string().optional(),
    order: z.number().optional(),
  }),
});

export const collections = { speeches, initiation };

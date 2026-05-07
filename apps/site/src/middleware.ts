import { defineMiddleware } from "astro:middleware";
import { paraglideMiddleware } from "./paraglide/server.js";

// Wrap every request in Paraglide's locale resolver. The strategy chain
// (cookie -> Accept-Language -> baseLocale) runs here, populates
// AsyncLocalStorage, and m.foo() calls inside .astro pages return the
// right language for the rest of the request.
//
// Vary headers tell the CDN to key its cache on the cookie + the
// Accept-Language header. Without them, an edge cache will happily
// serve one user's French page to a German visitor.
export const onRequest = defineMiddleware((context, next) => {
  return paraglideMiddleware(context.request, async () => {
    const response = await next();
    const existing = response.headers.get("Vary");
    const additions = ["Cookie", "Accept-Language"];
    const merged = existing
      ? Array.from(new Set([
          ...existing.split(",").map((v) => v.trim()).filter(Boolean),
          ...additions,
        ])).join(", ")
      : additions.join(", ");
    response.headers.set("Vary", merged);
    return response;
  });
});

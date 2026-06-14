const ZO_NAME = "zo";
const ZO_PRONONCIATION = "zu";
const ZSX_NAME = "zsx";
const CODELORD_NAME = "codelord";

/**
 * Transforms text into zo text convention
 * 
 * note: uppercased `I` are lowercased. The rest is capitalized, there is some
 * words exception that must be lowercased also.
 * 
 * @param text — the text to transform
 * @returns the formatted text
 */
export function textTransform(text: string): string {
  // Split into tag/non-tag chunks. HTML tags (and their attributes) pass
  // through untouched — only the text BETWEEN tags gets the transform.
  return text
    .split(/(<[^>]+>)/)
    .map((chunk) => {
      if (chunk.startsWith("<") && chunk.endsWith(">")) return chunk;
      return transformText(chunk);
    })
    .join("");
}

function transformText(text: string): string {
  return text
    .split(/\b/)
    .map((segment) => {
      if (segment.toLowerCase() === ZO_NAME) return ZO_NAME;
      if (segment.toLowerCase() === ZO_PRONONCIATION) return ZO_PRONONCIATION;
      if (segment.toLowerCase() === ZSX_NAME) return ZSX_NAME;
      if (segment.toLowerCase() === CODELORD_NAME) return CODELORD_NAME;

      return segment
        .split("")
        .map((char) => {
          if (char.toLowerCase() === "i") return "i";
          return char.toUpperCase();
        })
        .join("");
    })
    .join("");
}

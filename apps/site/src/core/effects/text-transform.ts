const ZO_NAME = "zo";
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
  return text
    .split(/\b/) // split by word boundaries
    .map(segment => {
      if (segment.toLowerCase() === ZO_NAME) return ZO_NAME;
      if (segment.toLowerCase() === CODELORD_NAME) return CODELORD_NAME;

      return segment
        .split("")
        .map(char => {
          if (char.toLowerCase() === "i") return "i";
          return char.toUpperCase();
        })
        .join("");
    })
    .join("");
}

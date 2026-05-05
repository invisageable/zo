// Module-cached promise — single GitHub Releases fetch per build, shared
// by every component that needs the latest zo version.
let cached: Promise<string> | null = null;

export function fetchLatestVersion(): Promise<string> {
  if (cached) return cached;

  cached = (async () => {
    try {
      const headers: HeadersInit = import.meta.env.GITHUB_TOKEN
        ? { Authorization: `Bearer ${import.meta.env.GITHUB_TOKEN}` }
        : {};

      const response = await fetch(
        "https://api.github.com/repos/invisageable/zo/releases/latest",
        { headers },
      );
      const data = await response.json();
      return (data.tag_name ?? "0.0.0").replace(/^v/, "");
    } catch {
      return "0.0.0";
    }
  })();

  return cached;
}

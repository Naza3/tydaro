import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

// Keep the review artifact self-contained: React, icons, chart paths and styles
// are embedded, so the same preview can be opened offline without a web server.
const directory = resolve("dist");
let html = await readFile(resolve(directory, "index.html"), "utf8");
const scripts = [
  ...html.matchAll(/<script\b[^>]*src="([^"]+)"[^>]*><\/script>/g),
];
const styles = [
  ...html.matchAll(/<link\b[^>]*rel="stylesheet"[^>]*href="([^"]+)"[^>]*>/g),
];
if (scripts.length !== 1 || styles.length !== 1) {
  throw new Error(
    "Expected one bundled script and stylesheet; review standalone packaging.",
  );
}
for (const [tag, source] of scripts) {
  const code = await readFile(resolve(directory, source), "utf8");
  html = html.replace(
    tag,
    () =>
      `<script type="module">${code.replace(/<\/script/gi, "<\\/script")}</script>`,
  );
}
for (const [tag, source] of styles) {
  const css = await readFile(resolve(directory, source), "utf8");
  html = html.replace(tag, () => `<style>${css}</style>`);
}
await writeFile(resolve(directory, "tydaro-preview.html"), html);
console.log("Offline preview: dist/tydaro-preview.html");

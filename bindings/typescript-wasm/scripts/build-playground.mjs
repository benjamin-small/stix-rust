// Assembles the static playground into playground-dist/: the page's own files,
// the wasm-pack web build (as pkg/), and the book's vendored Mermaid.
// Run via `npm run build:playground`, which builds pkg-web/ first.
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const from = (...parts) => join(root, ...parts);
const out = from("playground-dist");

if (!existsSync(from("pkg-web", "stix_wasm.js"))) {
  console.error("pkg-web/ is missing; run `npm run build:web` first");
  process.exit(1);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out);
cpSync(from("playground"), out, { recursive: true });
// Skip wasm-pack's npm packaging files; the page needs only the JS and wasm.
const PACKAGING = new Set([".gitignore", "package.json", "README.md"]);
cpSync(from("pkg-web"), join(out, "pkg"), {
  recursive: true,
  filter: (src) => !PACKAGING.has(basename(src)),
});
cpSync(from("..", "..", "docs", "book", "mermaid.min.js"), join(out, "mermaid.min.js"));
const vendor = join(out, "vendor");
const lib = from("node_modules", "@poietic-tech", "graphing-library");
for (const sub of ["dist", "pkg"]) {
  if (!existsSync(join(lib, sub))) {
    console.error(`@poietic-tech/graphing-library/${sub} is missing; run npm ci`);
    process.exit(1);
  }
  cpSync(join(lib, sub), join(vendor, "graphing-library", sub), {
    recursive: true,
    filter: (src) => !src.endsWith(".map") && !src.endsWith(".d.ts"),
  });
}
mkdirSync(join(vendor, "marked"), { recursive: true });
cpSync(from("node_modules", "marked", "lib", "marked.esm.js"), join(vendor, "marked", "marked.esm.js"));
cpSync(from("..", "..", "datasets", "hackers-1995"), join(out, "datasets", "hackers-1995"), {
  recursive: true,
  filter: (src) => !src.endsWith("README.md"),
});
console.log(`playground assembled in ${out}`);

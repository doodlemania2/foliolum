// The gate's second reader (docs/architecture.md, "The Phase 1 gate"): reads a .pub file with Pubwork's
// patched libmspub WebAssembly reader and prints, as one JSON line, what it found on each page. The
// gate compares that with foliolum-pub's import of the same file.
//
//   node gate/pubwork/oracle.mjs READER_DIR FILE.pub
//
// READER_DIR holds Pubwork's pubreader.mjs, pubreader.wasm and pubmodel.js at the pin in
// gate/fetch-fixtures.sh (MPL-2.0, fetched, never vendored). This script is ours and only calls them.
//
// Output: {"ok":true,"partial":bool,"pages":[PAGE...],"masters":[PAGE...]} or {"ok":false,"error":"..."}
//   PAGE = {"w":pt,"h":pt,"text":n,"image":n,"shape":n,"table":n,"stories":["..."]}
// Counts are leaf items (group members, not groups), with libmspub's ways of drawing some things
// brought in line with foliolum-pub's (see below). "stories" is the text on the
// page, one entry per story (libmspub gives each box of a linked story the whole story, so a story is
// listed once per page) and per table cell, whitespace collapsed, sorted.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { join, resolve } from 'node:path';

const [dir, file] = process.argv.slice(2);
if (!dir || !file) {
  console.error('usage: node oracle.mjs READER_DIR FILE.pub');
  process.exit(1);
}
const out = (o) => process.stdout.write(JSON.stringify(o) + '\n');

const norm = (s) => s.replace(/\s+/g, ' ').trim();
const parasText = (paras) => paras.map((pa) => pa.runs.map((r) => r.text).join('')).join('\n');

// Where libmspub draws one thing as two, or calls it something else, count it the way foliolum-pub
// imports it:
// - a text box's fill, or the shape that holds the text, arrives as a separate shape grouped with the
//   box, about or around its bounds; foliolum-pub keeps the fill on the frame, so it is not counted;
// - a filled shape's or a picture's outline arrives as a second, unfilled shape about its bounds (out
//   by up to the line width); it is part of that shape or picture;
// - a picture cropped to a shape arrives as a shape filled with the picture; it is a picture;
// - WordArt arrives as a shape carrying its words; foliolum-pub imports it as text.
const TOL = 1;
const near = (a, b, tol) => Math.abs(a - b) <= tol;
const sameBox = (a, b, tol = TOL) => near(a.x, b.x, tol) && near(a.y, b.y, tol) && near(a.w, b.w, tol) && near(a.h, b.h, tol);
const contains = (a, b) => a.x <= b.x + TOL && a.y <= b.y + TOL && a.x + a.w >= b.x + b.w - TOL && a.y + a.h >= b.y + b.h - TOL;
const filled = (it) => it.fill && it.fill.type !== 'none';
const plainShape = (it) => it.type === 'shape' && !it.wordart;
const isOutline = (it, siblings) => plainShape(it) && !filled(it) && it.stroke
  && siblings.some((o) => o !== it && ((plainShape(o) && filled(o)) || o.type === 'image') && sameBox(o, it, 1.5 + it.stroke.width));
const isBoxFill = (it, siblings) => {
  if (!plainShape(it)) return false;
  const texts = siblings.filter((t) => t.type === 'text');
  if (texts.some((t) => sameBox(t, it))) return true;
  // text in a shape: the group is the shape (and its outline) and one text box inside it
  return texts.length === 1 && siblings.every((o) => o.type === 'text' || plainShape(o)) && contains(it, texts[0]);
};
const kind = (it) => (it.wordart ? 'text' : it.type === 'shape' && it.fill?.type === 'image' ? 'image' : it.type);

function summarize(page) {
  const s = { w: page.w, h: page.h, text: 0, image: 0, shape: 0, table: 0, stories: [] };
  const seen = new Set();
  const story = (key, t) => {
    if (seen.has(key)) return;
    seen.add(key);
    t = norm(t);
    if (t) s.stories.push(t);
  };
  const walk = (items) => {
    for (const it of items) {
      if (it.items) { walk(it.items); continue; }
      if (isOutline(it, items) || isBoxFill(it, items)) continue;
      if (it.wordart) { s.text += 1; story(it.id, it.wordart.text || ''); continue; }
      const k = kind(it);
      s[k] = (s[k] || 0) + 1;
      if (it.type === 'text') story(it.story != null ? `s${it.story}` : it.id, parasText(it.paras));
      else if (it.type === 'table') for (const row of it.rows) for (const c of row.cells) story(`${it.id}:${s.stories.length}:${row.cells.indexOf(c)}`, parasText(c.paras));
    }
  };
  walk(page.items);
  s.stories.sort();
  return s;
}

try {
  const root = resolve(dir);
  const { buildDocument } = await import(pathToFileURL(join(root, 'pubmodel.js')).href);
  // The reader is an Emscripten module built for browsers and CommonJS Node; under ES modules it still
  // looks for these two globals. It is given the .wasm bytes, so it reads no files itself.
  globalThis.__dirname = root;
  globalThis.require = createRequire(join(root, 'pubreader.mjs'));
  const create = (await import(pathToFileURL(join(root, 'pubreader.mjs')).href)).default;
  const wasmBinary = readFileSync(join(root, 'pubreader.wasm'));
  const M = await create({ wasmBinary, print() {}, printErr() {} });
  const bytes = new Uint8Array(readFileSync(file));
  const p = M._malloc(bytes.length);
  M.HEAPU8.set(bytes, p);
  let rc;
  try { rc = M._pub_convert(p, bytes.length); } finally { M._free(p); }
  if (rc === 1) {
    out({ ok: false, error: 'the reader could not read this file' });
  } else {
    const ops = JSON.parse(M.UTF8ToString(M._pub_result(), M._pub_result_size()));
    const doc = buildDocument(ops, { source: 'pub' });
    out({ ok: true, partial: rc === 2, pages: doc.pages.map(summarize), masters: doc.masters.map(summarize) });
  }
} catch (e) {
  out({ ok: false, error: String(e?.message || e) });
}

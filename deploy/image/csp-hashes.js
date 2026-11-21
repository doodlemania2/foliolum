// Prints the CSP source for the page's one inline <script> and one inline <style>:
//
//   node deploy/image/csp-hashes.js <index.html>   ->   'sha256-...' 'sha256-...'
//
// The sha256 of the exact bytes between the tags, as the browser hashes them. Fails unless there is
// exactly one of each, rather than write a policy that blocks the editor or allows too much.
//
// Plain ES2017 on purpose: the CI runner's node is 10 (no matchAll, no optional chaining).
"use strict";
const fs = require("fs");
const crypto = require("crypto");

const page = fs.readFileSync(process.argv[2], "utf8");

// Only elements with a body: <script src=...></script> has an empty one and needs no hash.
function bodies(tag) {
  const re = new RegExp("<" + tag + "\\b(?![^>]*\\bsrc=)[^>]*>([\\s\\S]*?)</" + tag + ">", "g");
  const found = [];
  let m;
  while ((m = re.exec(page)) !== null) found.push(m[1]);
  return found;
}

const scripts = bodies("script");
const styles = bodies("style");
if (scripts.length !== 1 || styles.length !== 1) {
  console.error("expected one inline script and one inline style, found " + scripts.length + " and " + styles.length);
  process.exit(1);
}
const h = (s) => "'sha256-" + crypto.createHash("sha256").update(s, "utf8").digest("base64") + "'";
console.log(h(scripts[0]) + " " + h(styles[0]));

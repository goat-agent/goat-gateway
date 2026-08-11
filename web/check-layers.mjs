import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const ORDER = ["shared", "entities", "features", "widgets", "pages", "app"];
const complaints = [];

function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.tsx?$/.test(entry)) inspect(path);
  }
}

function inspect(path) {
  const [, layer, slice] = path.split("/");
  const rank = ORDER.indexOf(layer ?? "");
  if (rank === -1) return;

  for (const [, imported] of readFileSync(path, "utf8").matchAll(/from "(@\/[^"]+)"/g)) {
    const [, theirLayer, theirSlice, theirSegment] = imported.split("/");
    const theirRank = ORDER.indexOf(theirLayer ?? "");
    if (theirRank > rank) {
      complaints.push(`${path} reaches up to ${imported}`);
    }
    if (theirRank === rank && theirSlice !== slice && layer !== "shared" && theirSegment !== "@x") {
      complaints.push(`${path} crosses to ${imported} without an @x public API`);
    }
  }
}

walk("src");
if (complaints.length > 0) {
  console.error(complaints.join("\n"));
  process.exit(1);
}
console.log("layers are clean");

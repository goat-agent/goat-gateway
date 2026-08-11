import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const ORDER = ["shared", "entities", "features", "widgets", "pages", "app"];
const complaints = [];

for (const path of walk("src")) {
  const [, layer, slice] = path.split("/");
  if (!layer || !ORDER.includes(layer)) continue;

  for (const imported of readFileSync(path, "utf8").matchAll(/from "@\/([^"]+)"/g)) {
    const [target, targetSlice, ...rest] = imported[1].split("/");
    if (!target || !ORDER.includes(target)) continue;

    if (ORDER.indexOf(target) > ORDER.indexOf(layer)) {
      complaints.push(`${path} reaches up into ${imported[1]}`);
    } else if (target === layer && targetSlice !== slice && layer !== "shared") {
      const sideways = layer === "entities" ? rest.length > 0 : true;
      if (sideways) complaints.push(`${path} reaches sideways into ${imported[1]}`);
    } else if (target !== "shared" && target !== layer && rest.length > 0) {
      complaints.push(`${path} imports past the public API of ${target}/${targetSlice}`);
    }
  }
}

if (complaints.length > 0) {
  console.error(complaints.join("\n"));
  process.exit(1);
}
console.log("layers hold");

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.tsx?$/.test(entry)) yield path;
  }
}

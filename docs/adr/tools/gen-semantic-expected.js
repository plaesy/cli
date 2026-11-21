// Menulis hasil validate-ir.js (Node) per berkas ke semantic-expected.json.
// Rust membaca berkas ini di src/irvalidate/irvalidate_test.rs untuk memastikan paritas.
// Jalankan ulang setelah mengubah contoh, fixture, atau validate-ir.js.
"use strict";
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const adr = path.resolve(__dirname, "..");
const dirs = ["examples", "examples-negative-semantic", "corpus"];
const irs = ["flow-ir-1", "data-ir-1", "api-ir-1", "app-ir-1", "design-ir-1"];
const results = [];
for (const dir of dirs) {
  for (const ir of irs) {
    const full = path.join(adr, dir, ir);
    if (!fs.existsSync(full)) continue;
    for (const file of fs.readdirSync(full).filter(f => f.endsWith(".json")).sort()) {
      const rel = `${dir}/${ir}/${file}`;
      const res = spawnSync(process.execPath, [path.join(__dirname, "validate-ir.js"), path.join(full, file)], { encoding: "utf8" });
      results.push({ file: rel, semanticOk: res.status === 0 });
    }
  }
}
fs.writeFileSync(path.join(__dirname, "semantic-expected.json"), JSON.stringify(results, null, 2) + "\n");
console.log(`ok: ${results.length} berkas`);

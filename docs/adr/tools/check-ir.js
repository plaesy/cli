// Memeriksa bentuk semua IR di docs/adr (ADR 0009, bagian "Validasi").
// - examples/<ir>/*.json harus VALID terhadap schemas/<ir>.schema.json.
// - examples-negative/<ir>/*.json harus INVALID (fixture negatif).
// Jalankan: npm ci && npm run check   (dari docs/adr/tools)
"use strict";
const fs = require("fs");
const path = require("path");
const Ajv2020 = require("ajv/dist/2020");
const addFormats = require("ajv-formats");

const adr = path.resolve(__dirname, "..");
const irs = ["flow-ir-1", "data-ir-1", "api-ir-1", "app-ir-1", "design-ir-1"];
let failures = 0;

for (const ir of irs) {
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  const schema = JSON.parse(fs.readFileSync(path.join(adr, "schemas", `${ir}.schema.json`), "utf8"));
  const validate = ajv.compile(schema);

  for (const [dirName, expectValid] of [["examples", true], ["examples-negative", false]]) {
    const dir = path.join(adr, dirName, ir);
    if (!fs.existsSync(dir)) continue;
    for (const file of fs.readdirSync(dir).filter((f) => f.endsWith(".json")).sort()) {
      const doc = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8"));
      const ok = validate(doc);
      const pass = ok === expectValid;
      if (!pass) failures++;
      const label = `${dirName}/${ir}/${file}`;
      const detail = ok ? "VALID" : `INVALID (${validate.errors.length} error, pertama: ${validate.errors[0].instancePath || "/"} ${validate.errors[0].message})`;
      console.log(`${pass ? "OK  " : "FAIL"} ${label}: ${detail}`);
    }
  }
}

if (failures > 0) {
  console.error(`${failures} pemeriksaan gagal`);
  process.exit(1);
}
console.log("semua pemeriksaan bentuk lulus");

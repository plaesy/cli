// Memeriksa IR di docs/adr (ADR 0009, bagian "Validasi"):
//
// 1. Bentuk (JSON Schema, ajv):
//    - examples/<ir>/*.json harus VALID.
//    - examples-negative/<ir>/*.json harus INVALID.
//    - corpus/<ir>/*.json harus VALID.
// 2. Semantik (validate-ir.js, proses terpisah):
//    - examples/<ir>/*.json dan corpus/<ir>/*.json harus lolos (exit 0).
//    - examples-negative-semantic/<ir>/*.json harus gagal (exit 1).
// 3. Mode lenient (lenient.js):
//    - examples-lenient/<ir>/*.json harus INVALID dengan skema ketat, lalu VALID setelah
//      lenient memindahkan field tak dikenal ke extensions, dengan minimal satu peringatan.
// 4. Round trip serialisasi kanonik (korpus):
//    - stringify dengan kunci terurut lalu parse harus menghasilkan dokumen yang sama, dan
//      daftar ID node dan token tidak berubah. Ini TIDAK menguji round trip ke kode.
//
// Jalankan dari docs/adr/tools: npm ci && npm run check
"use strict";
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const Ajv2020 = require("ajv/dist/2020");
const addFormats = require("ajv-formats");
const { applyLenient } = require("./lenient");

const adr = path.resolve(__dirname, "..");
const irs = ["flow-ir-1", "data-ir-1", "api-ir-1", "app-ir-1", "design-ir-1"];
let failures = 0;
const report = (pass, label, detail) => {
  if (!pass) failures++;
  console.log(`${pass ? "OK  " : "FAIL"} ${label}: ${detail}`);
};
const jsonFiles = dir => (fs.existsSync(dir) ? fs.readdirSync(dir).filter(f => f.endsWith(".json")).sort() : []);
const readJson = p => JSON.parse(fs.readFileSync(p, "utf8"));

// Serialisasi kanonik: kunci objek diurutkan secara rekursif.
function canonical(v) {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === "object") {
    const out = {};
    for (const k of Object.keys(v).sort()) out[k] = canonical(v[k]);
    return out;
  }
  return v;
}

// Kumpulkan ID node dan nama token untuk memastikan tidak ada yang hilang.
function inventory(doc) {
  const ids = [];
  const walk = n => {
    if (Array.isArray(n)) return n.forEach(walk);
    if (n && typeof n === "object") {
      if (typeof n.id === "string") ids.push(n.id);
      Object.values(n).forEach(walk);
    }
  };
  walk(doc.components);
  walk(doc.pages);
  const tokens = [];
  const walkTok = (n, p) => {
    if (n && typeof n === "object" && "$value" in n) return tokens.push(p);
    if (n && typeof n === "object") for (const [k, v] of Object.entries(n)) if (k !== "extensions") walkTok(v, `${p}.${k}`);
  };
  walkTok(doc.tokens || {}, "");
  return { ids: ids.sort(), tokens: tokens.sort() };
}

// Tahap 1: bentuk
const schemas = {};
for (const ir of irs) {
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  const schema = readJson(path.join(adr, "schemas", `${ir}.schema.json`));
  schemas[ir] = schema;
  const validate = ajv.compile(schema);
  const cases = [["examples", true], ["examples-negative", false], ["corpus", true]];
  for (const [dirName, expectValid] of cases) {
    for (const file of jsonFiles(path.join(adr, dirName, ir))) {
      const doc = readJson(path.join(adr, dirName, ir, file));
      const ok = validate(doc);
      const detail = ok
        ? "VALID"
        : `INVALID (${validate.errors.length} error, pertama: ${validate.errors[0].instancePath || "/"} ${validate.errors[0].message})`;
      report(ok === expectValid, `bentuk ${dirName}/${ir}/${file}`, detail);
    }
  }
}

// Tahap 2: semantik
const semanticScript = path.join(__dirname, "validate-ir.js");
const runSemantic = file => spawnSync(process.execPath, [semanticScript, file], { encoding: "utf8" });
for (const ir of irs) {
  for (const [dirName, expectPass] of [["examples", true], ["corpus", true], ["examples-negative-semantic", false]]) {
    for (const file of jsonFiles(path.join(adr, dirName, ir))) {
      const res = runSemantic(path.join(adr, dirName, ir, file));
      const passed = res.status === 0;
      const detail = passed ? "semantik OK" : `semantik gagal: ${(res.stdout || "").trim().split("\n").slice(1, 2).join(" ") || "lihat validate-ir.js"}`;
      report(passed === expectPass, `semantik ${dirName}/${ir}/${file}`, detail);
    }
  }
}

// Tahap 3: mode lenient
for (const ir of irs) {
  const validate = new Ajv2020({ allErrors: true, strict: false });
  addFormats(validate);
  const v = validate.compile(schemas[ir]);
  for (const file of jsonFiles(path.join(adr, "examples-lenient", ir))) {
    const doc = readJson(path.join(adr, "examples-lenient", ir, file));
    const strictOk = v(doc);
    const { doc: lenientDoc, warnings } = applyLenient(schemas[ir], doc);
    const lenientOk = v(lenientDoc);
    report(!strictOk && lenientOk && warnings.length > 0, `lenient examples-lenient/${ir}/${file}`,
      `ketat=${strictOk ? "VALID" : "INVALID"}, lenient=${lenientOk ? "VALID" : "INVALID"}, peringatan=${warnings.length}`);
  }
}

// Tahap 4: round trip serialisasi kanonik (korpus design)
for (const file of jsonFiles(path.join(adr, "corpus", "design-ir-1"))) {
  const original = readJson(path.join(adr, "corpus", "design-ir-1", file));
  const text = JSON.stringify(canonical(original), null, 2);
  const reparsed = JSON.parse(text);
  const sameDoc = JSON.stringify(canonical(reparsed)) === JSON.stringify(canonical(original));
  const invA = inventory(original);
  const invB = inventory(reparsed);
  const sameInv = JSON.stringify(invA) === JSON.stringify(invB);
  const idempotent = JSON.stringify(canonical(JSON.parse(JSON.stringify(canonical(reparsed))))) === JSON.stringify(canonical(reparsed));
  report(sameDoc && sameInv && idempotent, `round-trip serialisasi corpus/design-ir-1/${file}`,
    `dokumen=${sameDoc ? "sama" : "beda"}, ID ${invA.ids.length}, token ${invA.tokens.length}, idempoten=${idempotent}`);
}

if (failures > 0) {
  console.error(`${failures} pemeriksaan gagal`);
  process.exit(1);
}
console.log("semua pemeriksaan bentuk, semantik, lenient, dan round trip serialisasi lulus");

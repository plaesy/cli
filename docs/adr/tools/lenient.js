// Mode lenient untuk pembaca dokumen dari versi lebih baru (ADR 0009, bagian 2).
//
// Field yang tidak dikenal oleh skema dipindahkan ke `extensions` pada objek terdekat
// yang punya `extensions`, dan dicatat sebagai peringatan. Nilainya tidak ditafsirkan.
// Hasilnya lalu bisa divalidasi dengan skema ketat.
//
// Batasan: penelusuran skema hanya memakai `properties`, `oneOf`, `anyOf`, `allOf`,
// `$ref` ke `#/$defs/...`, `items`, dan `additionalProperties` berbentuk skema.
"use strict";

function resolve(root, node) {
  let n = node;
  while (n && typeof n.$ref === "string" && n.$ref.startsWith("#/$defs/")) {
    n = root.$defs[n.$ref.slice("#/$defs/".length)];
  }
  return n;
}

// Kumpulkan semua skema objek yang berlaku untuk node ini (termasuk cabang oneOf/anyOf/allOf).
function objectSchemas(root, schema, out = []) {
  const s = resolve(root, schema);
  if (!s || typeof s !== "object") return out;
  if (s.properties || s.additionalProperties === false || s.unevaluatedProperties === false) out.push(s);
  for (const key of ["oneOf", "anyOf", "allOf"]) {
    for (const branch of s[key] || []) objectSchemas(root, branch, out);
  }
  if (s.then) objectSchemas(root, s.then, out);
  return out;
}

function allowedKeys(root, schema) {
  const branches = objectSchemas(root, schema);
  const keys = new Set();
  let closed = false;
  for (const b of branches) {
    for (const k of Object.keys(b.properties || {})) keys.add(k);
    if (b.additionalProperties === false || b.unevaluatedProperties === false) closed = true;
  }
  return { keys, closed, branches };
}

function propertySchema(root, branches, key) {
  for (const b of branches) {
    if (b.properties && b.properties[key]) return b.properties[key];
  }
  for (const b of branches) {
    if (b.additionalProperties && typeof b.additionalProperties === "object") return b.additionalProperties;
  }
  return null;
}

function lenientNode(root, value, schema, where, warnings) {
  if (value === null || typeof value !== "object") return value;
  const s = resolve(root, schema);
  if (Array.isArray(value)) {
    const itemSchema = s && s.items;
    return value.map((v, i) => lenientNode(root, v, itemSchema, `${where}/${i}`, warnings));
  }
  const { keys, closed, branches } = allowedKeys(root, s);
  const out = {};
  const unknown = {};
  for (const [k, v] of Object.entries(value)) {
    if (keys.has(k) || k === "extensions") {
      const ps = propertySchema(root, branches, k);
      out[k] = ps ? lenientNode(root, v, ps, `${where}/${k}`, warnings) : v;
    } else if (closed) {
      unknown[k] = v;
    } else {
      out[k] = v;
    }
  }
  if (Object.keys(unknown).length > 0) {
    if (!keys.has("extensions")) {
      // Tidak ada tempat untuk data tambahan: laporkan dan tetap biarkan validasi menolaknya.
      for (const k of Object.keys(unknown)) warnings.push(`${where}/${k}: field tidak dikenal, objek ini tidak punya extensions`);
      Object.assign(out, unknown);
    } else {
      out.extensions = { ...(out.extensions || {}), ...unknown };
      for (const k of Object.keys(unknown)) warnings.push(`${where}/${k}: field tidak dikenal, dipindah ke extensions`);
    }
  }
  return out;
}

// Mengembalikan { doc, warnings }. `schema` adalah objek JSON Schema yang sudah di-parse.
function applyLenient(schema, doc) {
  const warnings = [];
  const result = lenientNode(schema, doc, schema, "", warnings);
  return { doc: result, warnings };
}

module.exports = { applyLenient };

if (require.main === module) {
  const fs = require("fs");
  const [schemaPath, docPath] = process.argv.slice(2);
  if (!schemaPath || !docPath) {
    console.error("Pemakaian: node lenient.js <schema.json> <dokumen.json>");
    process.exit(2);
  }
  const schema = JSON.parse(fs.readFileSync(schemaPath, "utf8"));
  const doc = JSON.parse(fs.readFileSync(docPath, "utf8"));
  const { doc: out, warnings } = applyLenient(schema, doc);
  for (const w of warnings) console.error(`peringatan: ${w}`);
  process.stdout.write(JSON.stringify(out, null, 2) + "\n");
}

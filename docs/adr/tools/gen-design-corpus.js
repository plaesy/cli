// Membuat korpus desain sintetis (20 fixture) untuk design-ir/1 (ADR 0010, "Fidelity").
//
// Keluaran: ../corpus/design-ir-1/corpus-NN.json. Generator deterministik: seed tetap,
// jadi berkas yang sama dihasilkan setiap kali dijalankan.
//
// Batasan yang harus dibaca: korpus ini untuk menguji bentuk, semantik, dan kestabilan
// serialisasi. Korpus ini BELUM menguji round trip ke kode, karena adapter codegen belum ada
// (lihat ADR 0010, Fase 3). Jangan melaporkan fidelity dari korpus ini sebagai fidelity kode.
"use strict";
const fs = require("fs");
const path = require("path");

function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

const PALETTE = ["#2563EB", "#DC2626", "#FFFFFF", "#111827", "#16A34A", "#F59E0B"];
const FONTS = ["Inter", "Roboto", "Source Sans 3"];

function makeTokens(r, idx) {
  const color = {};
  PALETTE.forEach((hex, i) => {
    if (i < 3 + (idx % 3)) color[["primary", "danger", "surface", "text", "success", "warning"][i]] = { $type: "color", $value: hex };
  });
  const space = {};
  [4, 8, 16, 24].slice(0, 3 + (idx % 2)).forEach((px, i) => {
    space[["xs", "sm", "md", "lg"][i]] = { $type: "dimension", $value: `${px}px` };
  });
  const radius = { md: { $type: "dimension", $value: `${4 + (idx % 4) * 2}px` } };
  const font = {
    body: { $type: "typography", $value: { fontFamily: FONTS[idx % FONTS.length], fontSize: `${12 + (idx % 3) * 2}px`, fontWeight: 400 } },
  };
  const shadow = {
    card: { $type: "shadow", $value: { color: `rgba(15,23,42,0.${(idx % 5) + 1})`, offsetX: "0px", offsetY: "1px", blur: "2px", spread: "0px" } },
  };
  return { color, space, radius, font, shadow };
}

function colorRef(tokens, r) {
  const names = Object.keys(tokens.color);
  return `color.${names[Math.floor(r() * names.length)]}`;
}

function makeComponent(r, idx, cIdx, tokens) {
  const tones = ["primary", "danger"];
  const sizes = ["sm", "md"];
  return {
    id: `cp_c${idx}_${cIdx}`,
    name: `Komp${cIdx}`,
    root: {
      id: `nd_c${idx}_${cIdx}_root`,
      type: "frame",
      layout: { mode: "auto", direction: r() < 0.5 ? "horizontal" : "vertical", padding: "space.sm", gap: "space.sm", align: "center" },
      style: { fill: colorRef(tokens, r), radius: "radius.md" },
      children: [
        { id: `nd_c${idx}_${cIdx}_label`, type: "text", text: `Label ${cIdx}`, style: { font: "font.body", text_color: colorRef(tokens, r) } },
      ],
    },
    variants: { tone: tones, size: sizes },
  };
}

function makeNode(r, idx, depth, counter, tokens, compIds) {
  const id = () => `nd_${idx}_${counter.n++}`;
  const roll = r();
  if (depth > 2 || roll < 0.25) {
    return { id: id(), type: "text", text: `Teks ${counter.n}`, style: { font: "font.body", text_color: colorRef(tokens, r) } };
  }
  if (roll < 0.35) return { id: id(), type: "rect", style: { fill: colorRef(tokens, r), radius: "radius.md" } };
  if (roll < 0.42) return { id: id(), type: "image", src: `asset://img_${idx}_${counter.n}.png` };
  if (roll < 0.48) return { id: id(), type: "vector", path: "M0 0 L10 10" };
  if (roll < 0.53) {
    return { id: id(), type: "opaque", construct: "video-embed", source: { location: `pages/${idx}/video`, payload: "<video/>" } };
  }
  if (roll < 0.63 && compIds.length > 0) {
    const component = compIds[Math.floor(r() * compIds.length)];
    return { id: id(), type: "instance", component, variant: { tone: r() < 0.5 ? "primary" : "danger", size: r() < 0.5 ? "sm" : "md" } };
  }
  const direction = r() < 0.5 ? "horizontal" : "vertical";
  const mode = r() < 0.7 ? "auto" : "none";
  const node = {
    id: id(),
    type: "frame",
    layout: mode === "auto" ? { mode, direction, padding: "space.sm", gap: "space.sm", align: "center" } : { mode: "none" },
    style: { fill: colorRef(tokens, r) },
    children: [],
  };
  const n = 1 + Math.floor(r() * 3);
  for (let i = 0; i < n; i++) node.children.push(makeNode(r, idx, depth + 1, counter, tokens, compIds));
  return node;
}

function makeDoc(idx) {
  const r = rng(1000 + idx * 7919);
  const tokens = makeTokens(r, idx);
  const components = [];
  const nComp = 1 + (idx % 2);
  for (let c = 0; c < nComp; c++) components.push(makeComponent(r, idx, c, tokens));
  const compIds = components.map(c => c.id);
  const pages = [];
  const nPages = 1 + (idx % 2);
  for (let p = 0; p < nPages; p++) {
    const counter = { n: 0 };
    const frames = [];
    const nFrames = 1 + (idx % 3 === 0 ? 1 : 0);
    for (let f = 0; f < nFrames; f++) {
      // Frame halaman selalu berupa frame: pembungkus untuk konten acak.
      const inner = makeNode(r, idx * 10 + p, 1, counter, tokens, compIds);
      const frame = {
        id: `nd_${idx * 10 + p}_frame${f}`,
        type: "frame",
        name: `Frame ${f + 1}`,
        x: 0,
        y: 0,
        width: 1280,
        height: 800,
        layout: { mode: "auto", direction: "vertical", padding: "space.md", gap: "space.sm" },
        style: { fill: colorRef(tokens, r) },
        children: [inner],
      };
      frames.push(frame);
    }
    pages.push({ id: `pg_p${idx}_${p}`, name: `Halaman ${p + 1}`, frames });
  }
  return {
    ir: "design-ir/1",
    id: `dsg_corpus_${String(idx).padStart(2, "0")}`,
    version: 1,
    name: `corpus-${String(idx).padStart(2, "0")}`,
    tokens,
    components,
    pages,
  };
}

const outDir = path.resolve(__dirname, "..", "corpus", "design-ir-1");
fs.mkdirSync(outDir, { recursive: true });
for (let i = 1; i <= 20; i++) {
  const doc = makeDoc(i);
  fs.writeFileSync(path.join(outDir, `corpus-${String(i).padStart(2, "0")}.json`), JSON.stringify(doc, null, 2) + "\n");
}
console.log("ok: 20 fixture di", outDir);

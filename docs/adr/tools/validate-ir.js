#!/usr/bin/env node
// Validasi semantik untuk flow-ir/1, data-ir/1, api-ir/1, app-ir/1, dan design-ir/1.
// Skema bentuk dicek terpisah dengan ajv. Skrip ini hanya memeriksa aturan yang
// tidak bisa dinyatakan di JSON Schema (lihat bagian "Validasi semantik" di tiap ADR).
// Pemakaian: node validate-ir.js <berkas.json> [...]
// Keluar dengan kode 1 jika ada error.

const fs = require("fs");
const path = require("path");

function load(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function check(errors, cond, msg) {
  if (!cond) errors.push(msg);
}

// ---------- flow-ir/1 ----------
function validateFlow(doc) {
  const errors = [];
  const mods = doc.modules || [];
  const ids = mods.map(m => m.id);
  const idSet = new Set(ids);
  check(errors, idSet.size === ids.length, "id module tidak unik");

  const inputNames = new Set(Object.keys(doc.inputs || {}));
  const scopeSet = new Set(doc.scope || []);

  const refIds = (list) => (list || []);
  for (const m of mods) {
    const where = `module ${m.id}`;
    for (const r of [
      ...refIds(m.default),
      ...refIds(m.modules),
      ...(m.branches || []).flatMap(b => b.modules || []),
      m.on_reject,
    ].filter(Boolean)) {
      check(errors, idSet.has(r), `${where}: merujuk module yang tidak ada: ${r}`);
      check(errors, r !== m.id, `${where}: merujuk dirinya sendiri`);
    }
    if (m.on_error && !["fail", "continue"].includes(m.on_error)) {
      check(errors, idSet.has(m.on_error), `${where}: on_error merujuk module tidak ada: ${m.on_error}`);
    }
    if (m.kind === "subflow") {
      check(errors, m.flow_id !== doc.id, `${where}: subflow merujuk flow itu sendiri (rekursi)`);
    }
    if (m.scope) {
      check(errors, scopeSet.has(m.scope), `${where}: scope ${m.scope} melebar dari scope flow`);
    }
    // referensi flow.input.<nama>
    const text = JSON.stringify(m);
    for (const match of text.matchAll(/flow\.input\.([A-Za-z_][A-Za-z0-9_]*)/g)) {
      check(errors, inputNames.has(match[1]), `${where}: flow.input.${match[1]} tidak dideklarasikan`);
    }
    // referensi <module>.<field> di ekspresi (hanya nama yang diawali m dan cocok dengan id module)
    for (const match of text.matchAll(/\b(m[0-9A-Za-z_]*)\.[A-Za-z_]/g)) {
      const target = match[1];
      if (ids.includes(target)) continue;
      // bukan referensi module: abaikan kata seperti "mode"
    }
  }
  if (doc.failure_handler) {
    check(errors, idSet.has(doc.failure_handler), `failure_handler merujuk module tidak ada: ${doc.failure_handler}`);
  }
  return errors;
}

// ---------- data-ir/1 ----------
function validateData(doc) {
  const errors = [];
  const tables = doc.tables || [];
  const tableMap = new Map();
  for (const t of tables) {
    check(errors, !tableMap.has(t.name), `tabel duplikat: ${t.name}`);
    const cols = new Map((t.columns || []).map(c => [c.name, c]));
    check(errors, cols.size === (t.columns || []).length, `tabel ${t.name}: kolom duplikat`);
    for (const pk of t.primary_key || []) {
      check(errors, cols.has(pk), `tabel ${t.name}: primary_key merujuk kolom tidak ada: ${pk}`);
    }
    tableMap.set(t.name, cols);
  }
  const relIds = new Set();
  for (const r of doc.relations || []) {
    check(errors, !relIds.has(r.id), `relasi duplikat: ${r.id}`);
    relIds.add(r.id);
    const pairs = [];
    if (r.kind === "many_to_many") {
      pairs.push([r.left, r.junction.left_columns], [r.right, r.junction.right_columns]);
      check(errors, tableMap.has(r.junction.table), `${r.id}: junction ${r.junction.table} tidak ada`);
    } else {
      pairs.push([r.parent, r.parent.columns], [r.child, r.child.columns]);
    }
    const typeOf = (table, col) => {
      const c = (tableMap.get(table) || new Map()).get(col);
      return c ? JSON.stringify(c.type) : null;
    };
    if (r.kind === "many_to_many") {
      const pairsCheck = [
        [r.left, r.junction.left_columns],
        [r.right, r.junction.right_columns],
      ];
      for (const [end, jcols] of pairsCheck) {
        check(errors, tableMap.has(end.table), `${r.id}: tabel ${end.table} tidak ada`);
        check(errors, end.columns.length === jcols.length, `${r.id}: jumlah kolom junction tidak sama`);
        end.columns.forEach((c, i) => {
          const a = typeOf(end.table, c);
          const b = typeOf(r.junction.table, jcols[i]);
          check(errors, a !== null, `${r.id}: kolom ${end.table}.${c} tidak ada`);
          check(errors, b !== null, `${r.id}: kolom junction ${r.junction.table}.${jcols[i]} tidak ada`);
          check(errors, a === null || b === null || a === b, `${r.id}: tipe ${end.table}.${c} tidak cocok dengan junction`);
        });
      }
    } else {
      const [p, c] = [r.parent, r.child];
      check(errors, tableMap.has(p.table), `${r.id}: parent ${p.table} tidak ada`);
      check(errors, tableMap.has(c.table), `${r.id}: child ${c.table} tidak ada`);
      check(errors, p.columns.length === c.columns.length, `${r.id}: jumlah kolom parent dan child tidak sama`);
      p.columns.forEach((pc, i) => {
        const a = typeOf(p.table, pc);
        const b = typeOf(c.table, c.columns[i]);
        check(errors, a !== null, `${r.id}: kolom parent ${p.table}.${pc} tidak ada`);
        check(errors, b !== null, `${r.id}: kolom child ${c.table}.${c.columns[i]} tidak ada`);
        check(errors, a === null || b === null || a === b, `${r.id}: tipe ${p.table}.${pc} tidak cocok dengan ${c.table}.${c.columns[i]}`);
      });
    }
  }
  const relMap = new Map((doc.relations || []).map(r => [r.id, r]));
  for (const q of doc.queries || []) {
    const where = `query ${q.id}`;
    check(errors, tableMap.has(q.from), `${where}: from ${q.from} tidak ada`);
    const known = new Set([q.from]);
    for (const j of q.joins || []) {
      const r = relMap.get(j.relation);
      check(errors, !!r, `${where}: join merujuk relasi tidak ada: ${j.relation}`);
      if (!r) continue;
      const touch = r.kind === "many_to_many"
        ? [r.left.table, r.right.table]
        : [r.parent.table, r.child.table];
      check(errors, touch.some(t => known.has(t)), `${where}: join ${j.relation} tidak terhubung ke tabel yang sudah ada`);
      touch.forEach(t => known.add(t));
    }
    const aliases = new Set();
    const hasAgg = (q.select || []).some(s => s.agg);
    for (const s of q.select || []) {
      if (s.alias) {
        check(errors, !aliases.has(s.alias), `${where}: alias duplikat ${s.alias}`);
        aliases.add(s.alias);
      }
      if (s.table) {
        check(errors, known.has(s.table), `${where}: select memakai tabel di luar from/join: ${s.table}`);
        const cols = tableMap.get(s.table);
        if (cols && s.column) check(errors, cols.has(s.column), `${where}: kolom ${s.table}.${s.column} tidak ada`);
      }
    }
    if (hasAgg) {
      const grouped = new Set((q.group_by || []).map(g => `${g.table}.${g.column}`));
      for (const s of q.select || []) {
        if (!s.agg && s.table) {
          check(errors, grouped.has(`${s.table}.${s.column}`), `${where}: ${s.table}.${s.column} harus ada di group_by`);
        }
      }
    }
    for (const o of q.order_by || []) {
      check(errors, aliases.has(o.alias), `${where}: order_by merujuk alias tidak ada: ${o.alias}`);
    }
    const params = new Set(Object.keys(q.params || {}));
    const walk = (p) => {
      if (!p || typeof p !== "object") return;
      if (p.param) check(errors, params.has(p.param), `${where}: parameter tidak dideklarasikan: ${p.param}`);
      for (const v of Object.values(p)) walk(v);
    };
    walk(q.where);
  }
  return errors;
}

// ---------- api-ir/1 ----------
function validateApi(doc) {
  const errors = [];
  const types = new Set(Object.keys(doc.types || {}));
  const seen = new Set();
  const refs = (node) => {
    if (!node || typeof node !== "object") return;
    if (typeof node.$ref === "string") check(errors, types.has(node.$ref), `$ref tidak ada: ${node.$ref}`);
    for (const v of Object.values(node)) refs(v);
  };
  refs(doc.types);
  for (const ep of doc.endpoints || []) {
    const key = `${ep.method} ${ep.path}`;
    check(errors, !seen.has(key), `endpoint duplikat: ${key}`);
    seen.add(key);
    const placeholders = [...(ep.path || "").matchAll(/\{([a-z][a-z0-9_]*)\}/g)].map(m => m[1]);
    const declared = Object.keys(ep.path_params || {});
    for (const p of placeholders) check(errors, declared.includes(p), `${ep.id}: placeholder {${p}} tidak dideklarasikan di path_params`);
    for (const d of declared) check(errors, placeholders.includes(d), `${ep.id}: path_params ${d} tidak ada di path`);
    if (["GET", "DELETE"].includes(ep.method)) check(errors, !ep.body, `${ep.id}: ${ep.method} tidak boleh punya body`);
    refs(ep.body);
    refs(ep.responses);
    refs(ep.query_params);
    refs(ep.path_params);
  }
  return errors;
}

// ---------- app-ir/1 ----------
function validateApp(doc) {
  const errors = [];
  const screens = doc.screens || [];
  const screenMap = new Map();
  const routes = new Set();
  for (const s of screens) {
    check(errors, !screenMap.has(s.id), `layar duplikat: ${s.id}`);
    screenMap.set(s.id, s);
    check(errors, !routes.has(s.route), `route duplikat: ${s.route}`);
    routes.add(s.route);
    const routeParams = [...(s.route || "").matchAll(/:([a-z][a-z0-9_]*)/g)].map(m => m[1]);
    for (const p of routeParams) check(errors, !!(s.params || {})[p], `${s.id}: parameter route :${p} tidak dideklarasikan`);
  }
  if (doc.start_screen) check(errors, screenMap.has(doc.start_screen), `start_screen tidak ada: ${doc.start_screen}`);
  const cmpIds = new Set();
  const walkComp = (screen, c) => {
    check(errors, !cmpIds.has(c.id), `komponen duplikat: ${c.id}`);
    cmpIds.add(c.id);
    const declared = new Set(Object.keys(screen.params || {}));
    const queries = new Set(screen.queries || []);
    const check_vs = (v) => {
      if (!v || typeof v !== "object") return;
      if (v.param) check(errors, declared.has(v.param), `${screen.id}/${c.id}: param ${v.param} tidak ada di layar`);
      if (v.query) check(errors, queries.has(v.query), `${screen.id}/${c.id}: query ${v.query} tidak ada di layar`);
    };
    for (const v of Object.values(c.props || {})) check_vs(v);
    for (const e of c.events || []) {
      for (const v of Object.values(e.action.inputs || {})) check_vs(v);
      for (const nav of [e.action.on_success, e.action.on_failure]) {
        if (!nav) continue;
        check(errors, screenMap.has(nav.screen), `${screen.id}/${c.id}: navigasi ke layar tidak ada: ${nav.screen}`);
        for (const v of Object.values(nav.params || {})) check_vs(v);
      }
    }
    for (const ch of c.children || []) walkComp(screen, ch);
  };
  for (const s of screens) walkComp(s, s.root);
  return errors;
}

// ---------- design-ir/1 ----------
// Token DTCG (ADR 0010): daun adalah objek yang punya $value; grup adalah objek lain.
function flattenTokens(node, prefix, out) {
  if (node && typeof node === "object" && "$value" in node) {
    out.set(prefix, node);
    return;
  }
  if (node && typeof node === "object") {
    for (const [k, v] of Object.entries(node)) {
      if (k === "extensions") continue;
      flattenTokens(v, prefix ? `${prefix}.${k}` : k, out);
    }
  }
}

// Kategori token yang boleh dipakai di tiap properti (ADR 0010, "Aturan semantik").
const USE_CATEGORY = {
  fill: "color",
  stroke: "color",
  text_color: "color",
  radius: "radius",
  shadow: "shadow",
  font: "font",
  padding: "space",
  gap: "space",
};

const TOKEN_REF = /^(color|space|radius|font|shadow)\.[A-Za-z0-9_.-]+$/;

function checkTokenValue(errors, name, tok) {
  const v = tok.$value;
  switch (tok.$type) {
    case "color":
      check(errors, typeof v === "string" && /^(#[0-9A-Fa-f]{3,8}|rgba?\(.*\))$/.test(v), `token ${name}: nilai color tidak valid`);
      break;
    case "dimension":
      check(errors, typeof v === "string" && /^-?\d+(\.\d+)?px$/.test(v), `token ${name}: nilai dimension harus angka dengan satuan px`);
      break;
    case "typography":
      check(errors, !!v && typeof v === "object" && ["fontFamily", "fontSize", "fontWeight"].every(k => k in v), `token ${name}: typography wajib fontFamily, fontSize, fontWeight`);
      break;
    case "shadow":
      check(errors, !!v && typeof v === "object" && ["color", "offsetX", "offsetY", "blur"].every(k => k in v), `token ${name}: shadow wajib color, offsetX, offsetY, blur`);
      break;
    default:
      check(errors, false, `token ${name}: $type tidak dikenal (${tok.$type})`);
  }
}

function validateDesign(doc) {
  const errors = [];
  const tokenMap = new Map();
  flattenTokens(doc.tokens || {}, "", tokenMap);
  const tokenSet = new Set(tokenMap.keys());
  for (const [name, tok] of tokenMap) checkTokenValue(errors, name, tok);

  const compIds = new Set((doc.components || []).map(c => c.id));
  const compMap = new Map((doc.components || []).map(c => [c.id, c]));
  const nodeIds = new Set();
  const checkNode = (n, where) => {
    check(errors, !nodeIds.has(n.id), `node duplikat: ${n.id}`);
    nodeIds.add(n.id);
    const refs = [...Object.entries(n.style || {}), ...Object.entries(n.layout || {})];
    for (const [key, v] of refs) {
      if (typeof v !== "string" || !TOKEN_REF.test(v)) continue;
      check(errors, tokenSet.has(v), `${where}: token ${v} tidak ada`);
      const want = USE_CATEGORY[key];
      if (want) {
        check(errors, v.startsWith(want + "."), `${where}: properti ${key} memakai token kategori ${v.split(".")[0]}, seharusnya ${want}`);
      }
    }
    if (n.type === "instance") {
      check(errors, compIds.has(n.component), `${where}: komponen tidak ada: ${n.component}`);
      const comp = compMap.get(n.component);
      if (comp) {
        for (const [vname, vval] of Object.entries(n.variant || {})) {
          const allowed = (comp.variants || {})[vname];
          check(errors, !!allowed, `${where}: variant ${vname} tidak ada di ${n.component}`);
          if (allowed) check(errors, allowed.includes(vval), `${where}: nilai ${vval} tidak termasuk variant ${vname}`);
        }
      }
    }
    for (const ch of n.children || []) checkNode(ch, where);
  };
  for (const c of doc.components || []) checkNode(c.root, `komponen ${c.id}`);
  for (const p of doc.pages || []) for (const f of p.frames || []) checkNode(f, `halaman ${p.id}`);
  return errors;
}

const validators = {
  "flow-ir/1": validateFlow,
  "data-ir/1": validateData,
  "api-ir/1": validateApi,
  "app-ir/1": validateApp,
  "design-ir/1": validateDesign,
};

let failed = false;
for (const file of process.argv.slice(2)) {
  const doc = load(file);
  const fn = validators[doc.ir];
  if (!fn) {
    console.log(`${path.basename(file)}: ir tidak dikenal (${doc.ir})`);
    failed = true;
    continue;
  }
  const errors = fn(doc);
  if (errors.length === 0) {
    console.log(`${path.basename(file)}: semantik OK (${doc.ir})`);
  } else {
    failed = true;
    console.log(`${path.basename(file)}: ${errors.length} error semantik (${doc.ir})`);
    for (const e of errors) console.log(`  - ${e}`);
  }
}
process.exit(failed ? 1 : 0);

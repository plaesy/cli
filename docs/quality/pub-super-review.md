# Review of remaining `pub(super)` items

Status: reviewed 2026-11-18 on branch `chore/plaesy-program-refactor`.

Method: each item was compile-probed by removing `pub(super)` alone and running `cargo check --all-targets --locked`. An item that still compiles is private-safe; an item that fails is required by a sibling module or by a sibling test module. Result: 11 narrowed, 63 required.

Design decision for the 63: they are the public seam between submodules created by the split (for example `designlint/*`, `memory/cli/*`, `memory/search/*`, `tasks/*`). Narrowing them further would require moving code across module boundaries, which is a redesign and is not made here. The consumer list below is name-based and noisy (short names such as `c`, `a`, `check` match unrelated code); it is a locator, not proof. The compile probe is the proof.

| Location | Item | Name-based consumers (non-test files) |
| --- | --- | --- |
| `src/designlint/colour.rs:7` | `Rgb` | none outside; used by a sibling test module |
| `src/designlint/colour.rs:15` | `Colour` | designlint.rs, designlint/tokens.rs |
| `src/designlint/colour.rs:16` | `c` | analyze/detect.rs, analyze/fingerprint.rs, analyze/mod.rs |
| `src/designlint/colour.rs:17` | `a` | analyze/detect.rs, analyze/fingerprint.rs, analyze/mod.rs |
| `src/designlint/colour.rs:22` | `key` | analyze/output.rs, checklist.rs, cli/reach.rs |
| `src/designlint/colour.rs:33` | `hex` | designlint.rs, graph/incremental.rs, loop_run/receipt.rs |
| `src/designlint/colour.rs:65` | `parse_colour` | designlint/tokens.rs |
| `src/designlint/colour.rs:148` | `composite` | designlint.rs |
| `src/designlint/colour.rs:170` | `contrast_ratio` | designlint.rs |
| `src/designlint/fonts.rs:26` | `lint_fonts` | designlint.rs |
| `src/designlint/fonts.rs:51` | `font_metrics` | designlint.rs |
| `src/designlint/images.rs:17` | `lint_image_hosts` | designlint.rs |
| `src/designlint/images.rs:38` | `lint_figures` | designlint.rs |
| `src/designlint/tokens.rs:8` | `Tokens` | delegate/usage.rs, designlint.rs, loop_run/driver.rs |
| `src/designlint/tokens.rs:10` | `conflicts` | designlint.rs, memory/cli/output.rs, memory/graph/mod.rs |
| `src/designlint/tokens.rs:18` | `collect_tokens` | designlint.rs |
| `src/designlint/tokens.rs:44` | `resolve` | cli/memory.rs, cli/trim.rs, common/git.rs |
| `src/designlint/tokens.rs:60` | `split_top` | designlint.rs |
| `src/designlint/tokens.rs:89` | `css_decls` | designlint.rs |
| `src/designlint/tokens.rs:102` | `jsx_decls` | designlint.rs |
| `src/memory/cli/definitions.rs:10` | `Cli` | cli/mod.rs, cli/reach.rs, main.rs |
| `src/memory/cli/definitions.rs:13` | `config` | analyze/detect.rs, analyze/output.rs, cli/advisor.rs |
| `src/memory/cli/definitions.rs:17` | `db` | cli/memory.rs, cli/relay.rs, graph/types.rs |
| `src/memory/cli/definitions.rs:20` | `command` | analyze/mod.rs, checklist.rs, cli/advisor.rs |
| `src/memory/cli/definitions.rs:23` | `hygiene_strict_failure` | memory/cli/dispatch.rs |
| `src/memory/cli/definitions.rs:41` | `Mode` | memory/cli.rs, memory/cli/dispatch.rs |
| `src/memory/cli/definitions.rs:49` | `OutputFormat` | memory/cli/dispatch.rs |
| `src/memory/cli/definitions.rs:60` | `SAMPLE_PER_KIND` | memory/cli/dispatch.rs |
| `src/memory/cli/definitions.rs:63` | `Command` | cli/common.rs, cli/decisions.rs, cli/files.rs |
| `src/memory/cli/definitions.rs:405` | `GraphFormat` | memory/cli/dispatch.rs, memory/cli/output.rs |
| `src/memory/cli/definitions.rs:413` | `ProjectsSubcommand` | memory/cli/output.rs |
| `src/memory/cli/definitions.rs:440` | `ConfigSubcommand` | memory/cli.rs |
| `src/memory/cli/definitions.rs:464` | `ScheduleMode` | memory/cli.rs, memory/cli/dispatch.rs |
| `src/memory/cli/dispatch.rs:5` | `dispatch` | mcpserver.rs, memory/cli.rs, memory/service/mod.rs |
| `src/memory/cli/output.rs:8` | `parse_project_roots` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:47` | `validate_projects_filter` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:62` | `print_projects` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:102` | `print_project_detail` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:159` | `print_project_scoped` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:219` | `print_decision_list` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:263` | `print_context_list` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:284` | `print_curation_report` | memory/cli/dispatch.rs |
| `src/memory/cli/output.rs:356` | `print_json` | memory/cli.rs |
| `src/memory/cli/output.rs:363` | `require_dir` | memory/cli.rs, memory/cli/dispatch.rs |
| `src/memory/hygiene/markdown_links.rs:10` | `check` | analyze/mod.rs, checklist.rs, cli/analyze.rs |
| `src/memory/hygiene/paths.rs:55` | `exists_in_source_root` | memory/hygiene/scan.rs |
| `src/memory/notes/engine.rs:247` | `strip_last_verified` | memory/notes/mod.rs |
| `src/memory/notes/template.rs:299` | `yaml_key` | none outside; used by a sibling test module |
| `src/memory/notes/template.rs:303` | `yaml_string` | none outside; used by a sibling test module |
| `src/memory/search/citation.rs:133` | `docid_for` | memory/search/mod.rs |
| `src/memory/search/filter.rs:571` | `refine` | memory/search/query.rs |
| `src/memory/search/snapshot.rs:17` | `load_snapshot` | memory/search/mod.rs |
| `src/memory/search/snapshot.rs:62` | `save_snapshot_locked` | memory/search/indexing.rs |
| `src/memory/search/snapshot.rs:140` | `rebuild_hnsw_from_storage` | memory/search/indexing.rs |
| `src/memory/search/support.rs:5` | `git_revision` | memory/search/indexing.rs, memory/search/mod.rs |
| `src/memory/search/support.rs:65` | `archived_note_content` | memory/search/indexing.rs, memory/search/mod.rs |
| `src/memory/search/support.rs:166` | `reconcile_stale_temp_files` | memory/search/indexing.rs, memory/search/mod.rs |
| `src/tasks/creation.rs:167` | `write_task_files` | none outside; used by a sibling test module |
| `src/tasks/creation.rs:198` | `validate_task_name` | tasks/frontmatter.rs, tasks/issue.rs, tasks/mod.rs |
| `src/tasks/frontmatter.rs:96` | `path_has_symlink_component` | tasks/creation.rs, tasks/mod.rs, tasks/movement.rs |
| `src/tasks/movement.rs:40` | `acquire_issue_graph_lock` | tasks/creation.rs |
| `src/tasks/movement.rs:47` | `write_file_atomically` | tasks/creation.rs |
| `src/tasks/movement.rs:192` | `with_frontmatter_status` | none outside; used by a sibling test module |

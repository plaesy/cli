---
name: learning-loop
description: Preserves the durable outcome of substantive project work at task closeout by updating the project's source of truth and the Plaesy shared memory vault with progress, decisions, lessons, and unfinished-work checkpoints. Use when a task ends with a decision, a reusable lesson, a status change, or work worth resuming, even if the user never mentions memory. Skip routine edits such as typos, formatting, or dependency bumps that record no decision.
---

# Learning loop

## Overview

The user treats their Plaesy vault as shared memory across projects and agents. Run this closeout yourself; the user does not issue memory commands. The project's own files stay the source of truth. The vault holds an index of what matters, linked back to those files.

Do not assume a particular agent host, home layout, or vault location. The user may run Claude Code, Codex, Hermes, or another agent, and the vault may be anywhere. Discover each of these from the environment and configuration before use.

## When to use or skip

- Use: a decision was made, a non-obvious fix or lesson was found, project status changed, or work stopped midway and must resume later.
- Skip: routine edits, formatting, or a change whose only record is its commit. Finish without writing a note.

## Procedure

1. **Identify the project, its state, and the vault.**
   - Read the project's agent instructions file under whatever name this host uses (`AGENTS.md`, `CLAUDE.md`, or an equivalent), plus its status file, task tracker, and decision log.
   - Resolve the vault from the user's Plaesy configuration: `plaesy memory config` shows the effective settings, including the watched directory. Use the vault path the configuration gives. Never assume a fixed path.
   - If Plaesy, its CLI, or the vault is unavailable, record the result in the project's documentation and report the limitation.
2. **Choose the record type.** Keep it narrow:

   | Outcome | Destination | Vault type |
   | --- | --- | --- |
   | Progress and next step | Project status or context file | `context` |
   | Unfinished work | Session checkpoint in project docs | `context` |
   | Reusable procedure or lesson | Project docs, then vault knowledge | `knowledge` |
   | Lasting choice with rationale | Project decision log, then vault decision | `decision` |

3. **Update the project's source of truth first.** Then search the vault for an existing note on the same subject (`--vault <vault> search "<query>"`). Update it when one exists; do not duplicate it.
4. **Write the note.**
   - New note: `plaesy memory --vault <vault> remember "<title>" --folder <folder> [--type <type>] [--status <status>] [--project <project>]`. Pass the body on stdin when you omit it. If the vault has a matching template in `<vault>/templates/`, use `--template <name> --set KEY=VALUE`.
   - Changed note: `plaesy memory --vault <vault> fingerprint <path>`, then read the current note, prepare the full replacement text, and run `plaesy memory --vault <vault> update --if-match <sha256> --reason <correction|new-evidence|clarification|other> --yes <path>` with that text on stdin.
   - If the host exposes Plaesy as an MCP server instead of a shell, use its equivalents: `memory_remember`, `memory_fingerprint`, `memory_update` (`expected_sha256`), and `memory_search`.
   - If the host has no shell and no Plaesy MCP, skip the vault write, record the note text in the project documentation, and report that the vault was not updated.
5. **Check for skill work.** If the task exposed a reusable procedure, a repeated mistake, or missing guidance that a skill should carry, create or improve that skill:
   - Find the skill directories this host loads from. Check the host's documentation or the skills it reports; common locations include `~/.agents/skills/`, `~/.claude/skills/`, `~/.codex/skills/`, and a project-level `.agents/skills/` or `skills/`. Improve a skill that already covers the topic; create a new one only when none does.
   - If the host provides a `skill-creator` skill, use it for the edit. Otherwise apply these rules directly: keep the name and folder consistent, keep the description at most 1024 characters with "Use when" triggers, keep `SKILL.md` under 500 lines, and move long material into `references/` linked from `SKILL.md`.
   - Add gotchas from real failures, not hypothetical ones. Do not add content the agent already knows.
   - Validate with the project's linter if it has one (for Plaesy, `plaesy validate markdown <paths> --no-baseline`). Check that the name matches the folder and every linked file exists.
   - Write to the project's own skills directory when it has one. Write to a personal or global skills location only when the user asks for it.
   - Skip this step when the lesson is already in a skill or only applies to this task.
6. **Verify and report.** Search for the title and confirm the note returns. Then give one final line naming the project file, the vault note, and any skill created or changed, for example `Vault: updated decision <path>; skill: <path to SKILL.md>`.

## Gotchas

- `--vault` goes before the subcommand: `plaesy memory --vault <vault> remember …`.
- `remember` never overwrites. Repeating a title creates a second file, so search first.
- `update` replaces the whole note. Stdin must be the complete reviewed text, not a diff, and any edit makes the fingerprint stale.
- `status` is lifecycle state (`active`, `proposed`, `superseded`), never a progress message.
- Supersede a decision instead of deleting it: keep the old note and create the new one with `--supersedes`.
- Store no secrets, tokens, or transcript text. Mark each claim as verified or inferred.
- An installed `plaesy` binary can be older than the source. When a fix in the checkout matters, run the binary built from that checkout.
- Hosts differ in whether closeout runs automatically. Do not assume a hook or lifecycle event triggers this skill; the agent applies it when the task ends.

## Verification

- [ ] Project source of truth updated before the vault
- [ ] Vault searched before writing; no duplicate created
- [ ] Retrieval check returns the written or updated note, or the limitation is reported
- [ ] No secrets or raw transcript text stored
- [ ] Any skill created or changed keeps its name and folder consistent and passes the project's linter
- [ ] Final response names the note and any skill touched, or states that none was warranted

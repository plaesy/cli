---
name: learning-loop
description: At the end of substantive project work, preserve durable project state, context, lessons, knowledge, and decisions in the project's source of truth and the configured Plaesy shared memory. Skip routine work without a distinct reusable outcome.
---

# Learning loop

The user has designated the configured Plaesy vault as shared memory across projects and agents. Perform this closeout yourself; the user does not need to issue memory commands. Follow project and vault instructions, especially their established templates and write scopes. Never copy a whole conversation or unverified claim into memory.

1. Identify the current project from its repository and instructions. Read its existing status, task tracker, decisions, and relevant Plaesy notes before editing. Use `plaesy memory config` to find the configured vault; select that vault explicitly for writes. If Plaesy or the vault is unavailable, preserve the result in the project's established documentation and report the limitation.
2. Decide whether the work produced a durable outcome. A completed routine edit needs no new note. For substantive work, update the project's source of truth first, then search the vault for an existing matching note so the shared memory can link to or update it instead of duplicating it.
3. Preserve the narrowest useful record: current project progress and next step in the project note or context; a session checkpoint for unfinished work; a reusable procedure or lesson as knowledge; a lasting choice with rationale as a decision. Use the vault's matching template when available. Include project identity and a source reference. Distinguish verified results from inference, and do not store secrets or private transcript text.
4. Use `plaesy memory remember` for new notes. For a changed existing note, obtain `plaesy memory fingerprint` and use `plaesy memory update --if-match` with a reviewed replacement. Keep stable IDs and decision supersession history. The `status` of a project note is its lifecycle state, not a progress message. Use the installed CLI's `--help` for exact arguments.
5. Verify the written note can be retrieved and report the update briefly in the final response. If no note is warranted, finish without creating one.

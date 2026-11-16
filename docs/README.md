# Plaesy CLI documentation

Use this index to find documentation for the **currently implemented CLI**. The platform design is a separate, proposed target and must not be read as a description of shipped features.

## Get started

- Install and build: follow the repository's top-level `README.md`.
- Verify the installation with `plaesy status` and `plaesy --help`.
- [CLI command index](commands/README.md)
- [Memory subsystem guide](memory/overview.md)

## Command references

Each page documents one command and its supported options. For the authoritative current command tree, run `plaesy --help` or `plaesy <command> --help`.

- [Commands index](commands/README.md)
- [Memory command reference](commands/memory.md)
- [Task commands](commands/tasks.md)

## Memory subsystem

- [Overview](memory/overview.md)
- [Setup and vault selection](memory/setup.md)
- [Command reference](memory/reference.md)
- [Operations](memory/operations.md)
- [API and protocols](memory/api.md)
- [Graph guide](memory/graph.md)
- [MCP integration](memory/mcp.md)
- [Architecture](memory/architecture.md)

Other memory documents include design assessments and evaluation artifacts; they are not substitutes for the current command help.

## Platform design (proposed)

See [platform design index](platform/README.md). These documents describe goals and open design decisions, not implemented CLI behavior.

## Documentation status

This documentation is under active reconciliation with the CLI. If a page and `plaesy --help` disagree, treat the executable's help as the current command-surface authority and report the discrepancy for correction. Do not infer that an undocumented capability exists.

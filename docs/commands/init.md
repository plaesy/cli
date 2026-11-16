# `init`

Initialize project-local Plaesy metadata safely in the selected project directory. If no path is supplied, the current working directory is used.

```sh
plaesy init
plaesy init /path/to/project
plaesy init --dry-run /path/to/project
```

`--dry-run` reports what would be created without writing files. Re-running inside an initialized project is safe and preserves existing `.plaesy/` contents. The command refuses to replace a non-directory `.plaesy` path and refuses to create a project nested under another Plaesy project; a nested Git repository is treated as an independent project boundary. It creates only the `.plaesy/` directory—it does not create task folders or edit Git ignore rules, Memory settings, or agent instruction files. See `plaesy init --help` for the installed version's options.

See the [command index](README.md) and [Plaesy CLI documentation](../README.md).

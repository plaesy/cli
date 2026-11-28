# plaesy voice

Parses an already-transcribed sentence (English or Indonesian) into a task command and prints it as JSON. Nothing runs: it never creates, completes, or lists tasks, and it does not capture audio.

## Syntax

```sh
plaesy voice parse "<TEXT>"
```

## Example

```sh
plaesy voice parse "buat tugas beli susu"
plaesy voice parse "add task buy milk"
```

Review the JSON, then run the matching `plaesy tasks` command yourself if it is what the user meant.

`plaesy voice parse --help` is authoritative.

# plaesy reach

Queries external data sources (network) with response caching and rate limiting.

## Syntax

```sh
plaesy reach <PLATFORM> <QUERY> [--action search|view|...] [--limit 10] [--timeout 30s] [--cache-ttl 1h] [--no-cache] [--json] [--verbose]
plaesy reach list
plaesy reach configure <PLATFORM>
```

Platforms: web, youtube, github, rss, twitter, reddit, bilibili, xiaohongshu, linkedin, exa, v2ex, xueqiu, xiaoyuzhou, bosszhipin, facebook, instagram. The query is a URL, search terms, or a repository, depending on the platform.

## Examples

```sh
plaesy reach list
plaesy reach github "release workflow security" --action search --limit 5 --json
plaesy reach web https://example.com --action view
plaesy reach configure github     # stores credentials in the reach config
```

## Safety

Queries leave the machine. Do not put private project details in a query without the user's approval. `configure` stores credentials; never echo them into notes or logs. Check platform health with `plaesy doctor <platform>`.

`plaesy reach --help` is authoritative.

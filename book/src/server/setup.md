# Running `serve`

`rssdude serve` exposes the database over HTTP. Other machines (and other processes on the same machine) can then talk to it as a client.

## Start the server

```bash
rssdude serve                            # binds 127.0.0.1:8484 by default
rssdude serve --bind 0.0.0.0:8484        # listen on all interfaces (LAN-reachable)
rssdude serve --bind [::]:8484           # IPv6
```

The flag overrides the `bind` address from `~/.rssdude/config.toml` if both are set.

`serve` runs in the foreground. To run it as a daemon:

- macOS: write a launchd plist in `~/Library/LaunchAgents/` (see `man launchd.plist`)
- Linux: write a systemd unit in `~/.config/systemd/user/` and enable with `systemctl --user enable --now rssdude.service`
- Quick & dirty: `tmux new-session -d -s rssdude 'rssdude serve --bind 0.0.0.0:8484'`

## What's served

The HTTP layer is a thin shell over the same command functions the CLI uses, so behavior is identical wherever you run it. Endpoints mirror CLI subcommands:

| Endpoint | Equivalent CLI |
|---|---|
| `GET /api/feeds` | `rssdude list` |
| `POST /api/feeds` | `rssdude add` |
| `DELETE /api/feeds/<id>` | `rssdude remove` |
| `POST /api/sync` | `rssdude sync` |
| `GET /api/items` | `rssdude items` |
| `GET /api/items/<id>` | `rssdude read` |
| `POST /api/items/<id>/mark` | `rssdude mark` |
| `GET /api/digest` | `rssdude digest` |
| `GET /api/trending` | `rssdude trending` |
| `GET /api/stats` | `rssdude stats` |
| `GET /api/folders` | `rssdude folder list` |
| `POST /api/folders` | `rssdude folder create` |

You won't usually hit these endpoints directly — the `rssdude` CLI talks to them transparently when configured as a client.

## Auth

Optional bearer-token auth via the `[server] token` config field. See [multi-machine setup](multi-machine.md#auth).

## Health check

```bash
curl -i http://localhost:8484/api/feeds
```

If you set a token: `curl -H "Authorization: Bearer secret" http://localhost:8484/api/feeds`.

## Stopping it

It's a foreground process — `Ctrl-C`. If you daemonized via launchd/systemd, use the matching control command.

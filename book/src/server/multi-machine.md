# Multi-machine setup

The whole point of `serve` is having one box hold the database while you read from elsewhere. The setup is two config files and one running process.

## Topology

```
┌────────────────────────┐         ┌──────────────────┐
│   server box           │         │   laptop / phone │
│   (homeserver.local)   │◄────────┤   (client)       │
│   192.168.1.50         │  HTTP   │                  │
│                        │         │                  │
│   rssdude.redb         │         │   no DB locally  │
│   rssdude serve        │         │   rssdude CLI    │
└────────────────────────┘         └──────────────────┘
```

## On the server box

`~/.rssdude/config.toml`:

```toml
[server]
bind = "0.0.0.0:8484"
```

Then start it:

```bash
rssdude serve
```

`0.0.0.0` (not `127.0.0.1`) is what makes it reachable from the LAN. With `127.0.0.1`, only processes on the server box itself can connect.

If you want the server's CLI usage to talk to the local DB directly (faster than going through HTTP), don't set `address` on the server. The presence of `address` is what flips a machine into client mode.

## On the laptop / client

`~/.rssdude/config.toml`:

```toml
[server]
address = "192.168.1.50:8484"
```

That's it. Every `rssdude` CLI command on this machine now silently proxies to the server:

```bash
rssdude list                 # talks to 192.168.1.50:8484
rssdude items --unread
rssdude sync
```

Same binary, same flags, same output. You're now reading from the server's database without it ever leaving that box.

You can also reach the server by hostname if you've got mDNS / hosts entries:

```toml
[server]
address = "homeserver.local:8484"
```

## Auth

Optional but recommended on untrusted networks. Generate a token (any string), put it on **both** sides:

```toml
# ~/.rssdude/config.toml — same value on server and client
[server]
bind = "0.0.0.0:8484"           # server side only
address = "192.168.1.50:8484"   # client side only
token = "uOvJp6BxKMz6mJ1c"      # both sides
```

The client sends `Authorization: Bearer <token>` on every request; the server rejects requests with the wrong (or missing) token.

If `bind` and `address` are both set, the machine is treated as a client (`address` wins). Don't do that unless you know why.

## Standalone fallback

With no `[server]` block at all, `rssdude` runs standalone and uses the local DB at `~/.rssdude/rssdude.redb`. This is the default with no config file present.

## Firewall & reachability

A "connection refused" or timeout from the client usually means one of:

- the server isn't running (check with `ps` on the server box)
- the server is bound to `127.0.0.1`, not `0.0.0.0`
- the host firewall blocks inbound TCP/8484

Quick reachability check from the client:

```bash
nc -vz 192.168.1.50 8484
```

If `nc` connects, the network is fine — the issue is in `rssdude` config.

## Phones, tunnels, public exposure

For accessing your reader from outside your LAN, **don't** open port 8484 to the internet. Use a tunnel:

- [Tailscale](https://tailscale.com) — every device on a private mesh, address becomes the tailnet IP
- SSH port forwarding: `ssh -L 8484:localhost:8484 homeserver` and point the client at `localhost:8484`
- WireGuard, if you already have one set up

`rssdude` doesn't ship its own auth gateway because these tools already exist and do it better.

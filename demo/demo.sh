#!/usr/bin/env bash
# Drives an asciinema recording of rssdude. Uses an ephemeral DB so it
# doesn't touch the user's real one. Pacing tuned for readable playback.

set -e

export RSSDUDE_DB_PATH="$(mktemp -d)/rssdude.redb"
trap 'rm -rf "$(dirname "$RSSDUDE_DB_PATH")"' EXIT

# Pretty colors for the prompt and "comments"
GREEN=$'\033[1;32m'
DIM=$'\033[2m'
CYAN=$'\033[1;36m'
RESET=$'\033[0m'

PS="${GREEN}~${RESET} ${CYAN}\$${RESET} "

# type-out helper: prints a command char-by-char, then runs it
type_run() {
    local cmd="$1"
    printf "%s" "$PS"
    for ((i = 0; i < ${#cmd}; i++)); do
        printf "%s" "${cmd:$i:1}"
        sleep 0.018
    done
    printf "\n"
    sleep 0.35
    eval "$cmd"
    sleep 0.9
}

note() {
    printf "%s# %s%s\n" "$DIM" "$1" "$RESET"
    sleep 0.6
}

clear
sleep 0.5

note "📡 rssdude — local-first RSS in your terminal"
sleep 0.6

note "subscribe to a couple of feeds"
type_run "rssdude add https://hnrss.org/frontpage --tag tech"
type_run "rssdude add https://blog.rust-lang.org/feed.xml --tag rust"

note "list what we have"
type_run "rssdude list"

note "pull content (conditional GETs, 304-aware)"
type_run "rssdude sync"

note "see status with per-tag stats"
type_run "rssdude status"

note "browse unread items"
type_run "rssdude items --unread --limit 8"

note "search across everything we've pulled"
type_run "rssdude search rust --limit 5"

note "compute a 24h digest"
type_run "rssdude digest --since 24h"

note "everything also speaks --json for piping"
type_run "rssdude items --unread --limit 2 --json | jq '.[].title'"

sleep 0.6
note "👋 that's rssdude. there's a TUI too: 'rssdude browse'"
sleep 1.2

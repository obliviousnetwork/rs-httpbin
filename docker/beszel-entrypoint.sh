#!/bin/sh
# Container entrypoint that runs the Beszel monitoring agent alongside the app.
#
# The agent is best-effort: if it is not configured, if it fails to start, or if
# it dies, the app must be unaffected. So it is started in the background, never
# waited on, and the real command is exec'd either way.
#
# Configured by the app's environment:
#   BESZEL_HUB_URL   hub URL through the exe.dev proxy (enables the agent)
#   BESZEL_KEY       the hub's public key
#   BESZEL_TOKEN     the shared universal token (secret)
#   BESZEL_HUB_TOKEN exe.dev VM-scoped token for the private proxy (secret)
if [ -n "${BESZEL_HUB_URL:-}" ] && [ -n "${BESZEL_TOKEN:-}" ]; then
  # The hub identifies a system by the agent's fingerprint, which the agent
  # generates randomly on first run and keeps in its data directory. That
  # directory is ephemeral in a container, so without this the agent looks like
  # a brand-new machine on every restart and the hub accumulates a dead
  # duplicate system per deploy. Deriving the fingerprint from the Fly Machine
  # ID keeps it stable across restarts, while a replaced Machine -- which gets a
  # new ID -- correctly appears as a new system.
  if [ -n "${FLY_MACHINE_ID:-}" ]; then
    mkdir -p /var/lib/beszel-agent
    if [ ! -s /var/lib/beszel-agent/fingerprint ]; then
      printf '%s' "$(printf 'beszel:%s' "$FLY_MACHINE_ID" | sha256sum | cut -c1-48)" \
        > /var/lib/beszel-agent/fingerprint
    fi
  fi

  HUB_URL="$BESZEL_HUB_URL" \
  KEY="$BESZEL_KEY" \
  TOKEN="$BESZEL_TOKEN" \
  HUB_HEADERS="X-Exedev-Authorization: Bearer $BESZEL_HUB_TOKEN" \
  DISABLE_SSH=true \
  /usr/local/bin/beszel-agent >>/tmp/beszel-agent.log 2>&1 &
fi

exec "$@"

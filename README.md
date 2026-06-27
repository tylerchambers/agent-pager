# agent-pager

A tiny Rust CLI for paging via Telegram when an agent needs attention.

It is meant for agent sessions, shell scripts, tmux hooks, and tools that need to send a terse notification without depending on Oh My Pi internals.

## Security model

Telegram is a pager, not a secure transport.

Do not send:

- secrets
- logs
- stack traces
- diffs
- credentials
- URLs with tokens
- customer data

Store the Telegram bot token only in your shell environment, a local ignored `.env`, or another local secret manager. If a bot token is accidentally committed or pasted somewhere public, rotate it with BotFather.

## Install

Build from source:

```bash
cargo build --release
```

Install the binary somewhere on your `PATH`:

```bash
install -m 0755 target/release/agent-pager ~/.local/bin/agent-pager
```

Verify the CLI is available:

```bash
agent-pager --help
```

## Telegram setup

1. In Telegram, message **@BotFather**.
2. Run `/newbot`.
3. Pick a display name.
4. Pick a username ending in `bot`.
5. BotFather gives you a token. Save it as `AGENT_PAGER_TELEGRAM_BOT_TOKEN`.
6. Start a chat with your new bot and send it any message.
7. Get your chat id:

```bash
curl "https://api.telegram.org/bot$AGENT_PAGER_TELEGRAM_BOT_TOKEN/getUpdates"
```

Look for:

```json
"chat":{"id":123456789}
```

Save that value as `AGENT_PAGER_TELEGRAM_CHAT_ID`.

## Configure

Copy the template:

```bash
cp .env.example .env
chmod 600 .env
```

Edit `.env`:

```bash
AGENT_PAGER_TELEGRAM_BOT_TOKEN=123456:replace-me
AGENT_PAGER_TELEGRAM_CHAT_ID=123456789
AGENT_PAGER_DEFAULT_HOST=desktop
```

Load it into the current shell before using the CLI:

```bash
set -a
source .env
set +a
```

Or export the variables from your shell profile, tmux environment, direnv setup, or agent launcher.

Required variables:

| Variable | Purpose |
| --- | --- |
| `AGENT_PAGER_TELEGRAM_BOT_TOKEN` | Telegram bot token from BotFather. |
| `AGENT_PAGER_TELEGRAM_CHAT_ID` | Telegram chat id to receive pages. |

Optional variable:

| Variable | Purpose |
| --- | --- |
| `AGENT_PAGER_DEFAULT_HOST` | Host label shown in pages. Falls back to `$HOSTNAME`, then `unknown`. |

## Test Telegram directly

After loading the environment variables:

```bash
curl -sS -X POST "https://api.telegram.org/bot$AGENT_PAGER_TELEGRAM_BOT_TOKEN/sendMessage" \
  -d "chat_id=$AGENT_PAGER_TELEGRAM_CHAT_ID" \
  --data-urlencode "text=agent-pager raw API test from $(hostname)"
```

## Test agent-pager

```bash
agent-pager test
```

Expected result: Telegram receives a message like:

```text
agent-pager test from desktop
```

## Usage

Basic page:

```bash
agent-pager send "Agent needs review"
```

High-priority page with working directory and tmux session:

```bash
agent-pager send --priority high --cwd --tmux "Tests failed in wallet descriptor parser"
```

Example page:

```text
🔴 Agent needs attention
host: desktop
cwd: ~/src/walletd
tmux: main
priority: high
Tests failed in wallet descriptor parser.
SSH in and attach tmux.
```

If `--tmux` is passed outside tmux, the page includes:

```text
tmux: unavailable
```

## Environment template

`.env.example` is committed. `.env` and `.env.*` are ignored by Git.

Never commit real Telegram credentials.

## Development

Run tests:

```bash
cargo test
```

Format code:

```bash
cargo fmt
```

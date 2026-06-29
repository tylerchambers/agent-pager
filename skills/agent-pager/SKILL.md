---
name: agent-pager
description: Send Telegram pages or sanitized Markdown/document attachments when a human needs attention.
---

# agent-pager

Use `agent-pager` when the human explicitly asks to be paged, you are blocked on human-only input, or a requested long-running job finishes.

Do not page for routine progress. Do not repeat the same page unless asked.

Telegram is not secure transport. Never send secrets, credentials, tokens, customer data, private URLs, or unsanitized logs/diffs/stack traces.

Commands:

```bash
agent-pager send "Need review"
agent-pager send --priority high --cwd --tmux "Blocked on deploy approval"
agent-pager send --format markdown-v2 "*Build* failed in `parser`"
agent-pager send --document report.md "Review attached Markdown report"
generate-report | agent-pager send --document - --document-name report.md "Review attached report"
```

Use `--document` for larger Markdown/text/report files instead of splitting messages. `--format` affects short text messages and document captions only; Markdown document contents are uploaded unchanged.

After paging, continue safe independent work. If blocked, state exactly what response is needed.

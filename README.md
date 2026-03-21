# SeggWat CLI

Manage feedback, projects, and ratings from the terminal. The official command-line interface for the [SeggWat](https://seggwat.com) feedback platform.

[![CI](https://github.com/hauju/seggwat-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/hauju/seggwat-cli/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/seggwat-cli.svg)](https://crates.io/crates/seggwat-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Install

**crates.io:**

```bash
cargo install seggwat-cli
```

**Shell script (Linux & macOS):**

```bash
curl -fsSL https://seggwat.com/static/install.sh | sh
```

**From source:**

```bash
cargo install --git https://github.com/hauju/seggwat-cli
```

## Quick Start

```bash
# Authenticate (opens browser for OAuth login)
seggwat login

# List your projects
seggwat project list

# View feedback for a project
seggwat feedback list <project-id>

# Create feedback from the terminal
seggwat feedback create <project-id> --message "Button doesn't work on mobile" --type bug
```

Or use an API key directly:

```bash
export SEGGWAT_API_KEY=oat_your_api_key_here
seggwat project list
```

## Features

- **Full feedback management** — list, create, update, delete, and view statistics
- **Rating analytics** — helpful (thumbs up/down), star ratings, and NPS scores with visual charts
- **Project overview** — list projects, view details, get summaries with stats
- **Two auth methods** — OAuth login (opens browser) or API key for scripts and CI
- **JSON output** — `--json` flag for piping to `jq` or other tools
- **Shell completions** — bash, zsh, fish, PowerShell, elvish
- **Self-hosted support** — works with custom SeggWat instances via `--api-url`

## Commands

```
seggwat [OPTIONS] <COMMAND>

Commands:
  project      Manage projects (alias: p)
  feedback     Manage feedback (alias: fb)
  rating       Manage ratings (alias: r)
  login        Log in with your SeggWat account
  logout       Log out and clear cached tokens
  whoami       Show the currently authenticated user
  completions  Generate shell completions

Global Options:
  --api-url <URL>    SeggWat API base URL [env: SEGGWAT_API_URL] [default: https://seggwat.com]
  --api-key <KEY>    API key for authentication [env: SEGGWAT_API_KEY]
  --json             Output as JSON
  -v, --verbose      Enable debug logging
```

### Project Commands

```bash
seggwat project list                  # List all projects
seggwat project get <id>              # Project details
seggwat project summary <id>          # Project with feedback & rating stats
```

### Feedback Commands

```bash
seggwat feedback list <project-id>                                  # List feedback
seggwat feedback list <project-id> --status active --type bug       # Filtered list
seggwat feedback list <project-id> --search "mobile"                # Search feedback
seggwat feedback get <project-id> <feedback-id>                     # View single item
seggwat feedback create <project-id> -m "Great product!" --type praise
seggwat feedback update <project-id> <id> --status resolved --resolution-note "Fixed in v2.1"
seggwat feedback delete <project-id> <feedback-id>
seggwat feedback stats <project-id>                                 # Feedback counts
```

### Rating Commands

```bash
seggwat rating list <project-id>                           # List all ratings
seggwat rating list <project-id> --type star               # Filter by type
seggwat rating get <project-id> <rating-id>                # View single rating
seggwat rating delete <project-id> <rating-id>
seggwat rating stats <project-id>                          # Helpful stats (default)
seggwat rating stats <project-id> --type star              # Star rating distribution
seggwat rating stats <project-id> --type nps               # NPS score breakdown
```

## Authentication

### OAuth Login (Interactive)

```bash
seggwat login
```

Opens your browser for authentication. Tokens are cached at `~/.config/seggwat/tokens.json` with automatic refresh.

### API Key (Non-Interactive)

```bash
# Via environment variable
export SEGGWAT_API_KEY=oat_your_key_here

# Or via flag
seggwat --api-key oat_your_key_here project list
```

Generate API keys in the [SeggWat Dashboard](https://seggwat.com) under Organization Settings.

### Self-Hosted

```bash
seggwat --api-url https://feedback.yourcompany.com login \
  --zitadel-domain auth.yourcompany.com \
  --client-id YOUR_CLIENT_ID
```

## Shell Completions

```bash
# Bash
seggwat completions bash > ~/.local/share/bash-completion/completions/seggwat

# Zsh
seggwat completions zsh > ~/.zfunc/_seggwat

# Fish
seggwat completions fish > ~/.config/fish/completions/seggwat.fish
```

## JSON Output

All commands support `--json` for machine-readable output:

```bash
seggwat --json feedback list <project-id> | jq '.feedback[].message'
seggwat --json rating stats <project-id> --type nps | jq '.score'
```

## License

MIT

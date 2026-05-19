# ownify-desk

Local multi-agent management dashboard for [ownify microclaw](https://github.com/HaraldeRoessler/ownify-microclaw).

## What it does

ownify-desk runs on your machine and manages multiple AI agents (microclaw instances) through a web dashboard:

- **Create agents** with a click — each gets its own config, data directory, and port
- **Start/stop/restart** agents from the dashboard
- **Edit agent configs** — YAML editor with Matrix, Telegram, Discord, Slack presets
- **View logs** per agent in real-time
- **Monitor status** — running, stopped, crashed

```mermaid
graph TB
    subgraph "ownify-desk"
        UI["Web Dashboard<br/>localhost:9090"]
        CFG["Config Manager<br/>~/.ownify-desk/agents/"]
        PM["Process Manager<br/>spawn/monitor/stop"]
    end
    subgraph "Agents"
        A["microclaw agent<br/>:10961"]
        B["microclaw agent<br/>:10962"]
        C["microclaw agent<br/>:10963"]
    end
    subgraph "Providers"
        LLM["Anthropic / OpenAI / Ollama"]
    end
    UI --> CFG
    UI --> PM
    PM --> A
    PM --> B
    PM --> C
    A --> LLM
    B --> LLM
    C --> LLM
```

## Quick start

```sh
# Install microclaw first
git clone https://github.com/HaraldeRoessler/ownify-microclaw.git
cd ownify-microclaw && cargo build --release

# Install ownify-desk
git clone https://github.com/HaraldeRoessler/ownify-desk.git
cd ownify-desk && cargo build --release

# Start the dashboard
./target/release/ownify-desk start
```

Open `http://localhost:9090` and create your first agent.

## Data layout

```
~/.ownify-desk/
  agents/
    personal-assistant/
      meta.yaml                  # Agent metadata (name, port, auto-start)
      microclaw.config.yaml      # LLM provider + channel config
      data/                       # Sessions, memory, SQLite
      logs/microclaw.log          # Agent runtime logs
    devops-bot/
      meta.yaml
      microclaw.config.yaml
      data/
      logs/
```

## Tech stack

| Layer | Technology |
|---|---|
| Backend | Rust + axum |
| Frontend | React + TypeScript + Vite |
| Agent runtime | [ownify microclaw](https://github.com/HaraldeRoessler/ownify-microclaw) |

## License

MIT

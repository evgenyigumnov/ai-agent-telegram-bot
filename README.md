# 🤖 AI Agent Telegram Bot

A smart and friendly AI-powered Telegram bot that can answer questions, store and forget information, and even execute terminal commands on request. Built with Rust, powered by LLMs and vector search via Qdrant.

## 🧠 Features

- 🔐 Password-protected access
- 💬 Classifies user input (question, info, forget request, command, etc.)
- 📚 Stores and searches documents with vector embeddings (Qdrant)
- 🤖 Talks to an LLM for reasoning, classification, and responses
- 💥 Can execute Linux commands after confirmation
- 🔁 State-based interaction flow (e.g., confirmation dialogs)
- 🐳 Docker & Docker Compose support

## 🛠 Tech Stack

- Rust + Tokio async runtime
- [`teloxide`](https://github.com/teloxide/teloxide) — Telegram bot framework
- [`reqwest`](https://github.com/seanmonstar/reqwest) — HTTP client
- [`serde`](https://github.com/serde-rs/serde) — JSON serialization
- [`qdrant`](https://qdrant.tech/) — Vector search engine
- Any LLM that supports OpenAI-compatible API (e.g. [`lm-studio`](https://github.com/lmstudio-ai/lms), [`ollama`](https://github.com/ollama/ollama), etc.)

## 🚀 Quickstart (Docker Compose)

```bash
# Copy .env-example and configure your keys
cp .env-example .env

# Build and run
docker compose up --build
```

> Make sure your LLM server is running and accessible at the given URLs in `.env`.

## 📦 Environment Variables (`.env`)

```env
TELOXIDE_TOKEN=your_telegram_bot_token
OPENAI_API_KEY=dummy_or_real_key
CHAT_COMPLETIONS_URL=http://localhost:1234/v1/chat/completions
CHAT_COMPLETIONS_MODEL=gemma-3-12b-it
EMBEDDINGS_URL=http://127.0.0.1:1234/v1/embeddings
EMBEDDINGS_MODEL=lm-kit/text-embedding-bge-m3
EMBEDDINGS_LENGTH=1024
QDRANT_URL=http://localhost:6333
QDRANT_COLLECTION_NAME=documents
BOT_PASSWORD=supersecret
```

## 🧑‍💻 Local Development

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Run the bot
cargo run
```

Make sure Qdrant and your LLM API are running and accessible. You can use Docker for Qdrant:

```bash
docker run -p 6333:6333 qdrant/qdrant
```

And run LM Studio or similar on port 1234.

## 🤓 How it Works (Black Box)

1. Bot asks for a password
2. After login, it classifies each message using the LLM:
    - Question → Uses vector search + LLM to answer
    - Informational text → Stores it in Qdrant
    - Forget request → Confirms and deletes from Qdrant
    - Terminal command → Confirms before running it
3. Uses embeddings to find semantically similar data
4. Everything happens with friendly and minimal responses

## 📝 How it Works (White Box)
```mermaid
flowchart TD
    A[User sends message via Telegram] --> B[Telegram Bot receives message]
    B --> C{Current State?}
    C -- AwaitingPassword --> D[Validate Password: compare with BOT_PASSWORD]
    D -- Valid --> E[Switch to Pending State]
    D -- Invalid --> C
    E --> F[Process Message: exec_pending]
    F --> G[Call LLM to classify message type]
    G --> H{Message Type}
    
    H -- "1: Question" --> I[Extract Keywords using LLM]
    I --> J[Search Documents in Qdrant: exec_answer]
    J --> K[Append document context to question]
    K --> L[Call LLM to generate answer]
    L --> M[Send Answer to User]
    
    H -- "2: Statement" --> N[Save Information: exec_remember]
    N --> O[Generate embedding and add document to Qdrant]
    O --> P[Reply 'Information saved.']
    
    H -- "3: Forget Info" --> Q[Extract Keywords using LLM: new_forget]
    Q --> R[Search Document in Qdrant]
    R --> S[Prompt user to confirm deletion]
    S --> T{User confirms?}
    T -- Yes --> U[Delete document from Qdrant: exec_forget]
    U --> V[Reply 'Information forgotten.']
    T -- No --> W[Reply 'Information not forgotten.']
    
    H -- "4: Command" --> X[Extract Linux command using LLM: new_command]
    X --> Y[Prompt user for command execution confirmation]
    Y --> Z{User confirms?}
    Z -- Yes --> AA[Execute command in Linux terminal]
    AA --> AB[Return command output to user]
    Z -- No --> AC[Reply 'Command not executed.']
    
    H -- "Other" --> AD[Call LLM for chat response: exec_chat]
    AD --> AE[Send Chat reply to User]
```

## 📁 Project Structure

```
├── src/
│   ├── main.rs        # Telegram bot logic & state machine
│   ├── ai.rs          # LLM + embedding logic
│   └── qdrant.rs      # Qdrant vector DB integration
├── .env-example       # Config template
├── Dockerfile
├── docker-compose.yml
└── README.md
```

## 🧡 Credits

- Inspired by real assistant workflows
- Powered by open tools: Rust, Qdrant, and community LLMs

---

> Built with ❤️ and `cargo build --release`



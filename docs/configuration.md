# Configuration

DexDeck works without project configuration. Shared configuration is optional
at .dexdeck/config.toml and is created only by an explicit command. Per-user
project configuration and generated state live in platform-standard user
directories.

Precedence from highest to lowest is:

1. explicit CLI arguments;
2. an explicitly selected config file;
3. per-user project configuration;
4. shared project configuration;
5. detected project values;
6. built-in defaults.

Configuration files use schema_version = 1. Unknown fields are retained and
reported as warnings. Invalid types and values report the source path and
line/column when available. Local migrations create an atomic backup; shared
migrations require explicit confirmation and preserve comments.

Programs and Gradle arguments are arrays. String commands, implicit shells,
automatic .env loading, and NUL-containing arguments are rejected.

Values whose names look secret must use an environment reference:

    API_TOKEN = { from_env = "DEMO_API_TOKEN" }

Resolved secret values must never enter UI state, job metadata, errors, debug
logs, or machine-readable output.

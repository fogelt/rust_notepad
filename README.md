# Project Commands

To run the project:

```bash
cargo run
```

To build the project:

```bash
cargo packager --release
```

## Environment Configuration

To get started, you need to configure your environment variables. 

1. Copy the `.env_example` file and rename it to `.env`.
2. Open the `.env` file and replace the placeholder values with your specific configuration:

```text
GEMINI_API_KEY=your_actual_api_key_here
GEMINI_MODEL=gemini-3.1-flash-lite
```

You can obtain your Gemini API key by visiting the [Google AI Studio](https://aistudio.google.com/).
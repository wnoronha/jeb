# Local decisions with LiteRT-LM (Option A)

Run [EmbeddingGemma 2](https://ai.google.dev/gemma/docs/embeddinggemma/model_card_2) on-device via [LiteRT-LM](https://developers.google.com/edge/litert-lm/overview) and use `jeb` without the TypeSafe cloud API.

Local mode turns each `choice`, `noul`, and `score` question into **classification-style embedding comparisons** (cosine similarity + softmax). Results use the same JSON shape as cloud mode, but probabilities are **similarity-derived**, not Jev-calibrated.

---

## 1. Install LiteRT-LM and import the model

**Optional bootstrap** (creates `.venv` in the repo when `uv` is available, otherwise uses your active Python):

```bash
./scripts/bootstrap-litert-local.sh setup   # install + import
./scripts/bootstrap-litert-local.sh serve   # foreground server (separate terminal)
./scripts/bootstrap-litert-local.sh verify  # smoke-test /v1/embeddings
```

Manual steps (same result):

Text-only workloads match jeb’s JSON/CLI (recommended bundle ~165 MB):

```bash
pip install 'litert-lm==0.18.0'
litert-lm import litert-community/embeddinggemma-2-text-270m-litert-lm
litert-lm list   # note the ID column — jeb must use this exact name
```

Multimodal state (images/audio) requires the [740M bundle](https://huggingface.co/litert-community/embeddinggemma-2-740m-litert-lm) instead:

```bash
litert-lm import litert-community/embeddinggemma-2-740m-litert-lm
```

The **model id** sent to `/v1/embeddings` is always the name from `litert-lm list` (for example `embeddinggemma-2-740m.litertlm`), not necessarily the Hugging Face repo name. Google's docs sometimes show aliases like `embedding-gemma-v2-text.litertlm`; those only work if that id appears in `litert-lm list` (default import id or after `litert-lm rename`).

---

## 2. Start the embedding server

```bash
litert-lm serve --host 127.0.0.1 --port 9379
```

Leave this running (first load can take several seconds). Use GPU when available by passing the backend in the model id (see below).

Verify:

```bash
curl -s -X POST http://127.0.0.1:9379/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model": "embedding-gemma-v2-text.litertlm,cpu", "input": "task: classification | text: hello"}'
```

If you get `Model … not found`, run `litert-lm list` and set `LITERT_MODEL` to that id plus an optional backend suffix (`,cpu` or `,gpu`).

---

## 3. Run jeb in local mode

No `TYPESAFE_API_KEY` is required.

```bash
jeb --backend local \
  --state "Production API error rate spiked after deploy." \
  --choice "triage:What is the immediate mitigation?" \
  --option "rollback=Rollback to previous release" \
  --option "restart=Restart pods" \
  -d
```

Environment variables:

| Variable | Default | Purpose |
|----------|---------|---------|
| `JEB_BACKEND` | `cloud` | `local` or `cloud` |
| `LITERT_EMBED_URL` | `http://127.0.0.1:9379` | LiteRT `litert-lm serve` base URL |
| `LITERT_MODEL` | `embedding-gemma-v2-text.litertlm,cpu` | Model id sent to `/v1/embeddings` |
| `LITERT_SIMILARITY_TEMPERATURE` | `0.07` | Softmax temperature over cosine scores (lower = sharper) |

Example with JSON stdin:

```bash
export JEB_BACKEND=local
export LITERT_MODEL='embedding-gemma-v2-text.litertlm,gpu'  # macOS Apple Silicon, if supported
cat request.json | jeb -d
```

---

## 4. How decisions are computed

For each question, `jeb` builds LiteRT [task prefixes](https://developers.google.com/edge/litert-lm/embedding_models):

- Prefix: `task: classification | text: …`
- **Query:** state + instructions
- **Candidates:** option text, yes/no descriptions, or score level labels

It requests embeddings in one batch from `/v1/embeddings`, compares the query vector to each candidate with cosine similarity, and applies softmax with `LITERT_SIMILARITY_TEMPERATURE`.

---

## 5. Limitations

- Semantic routing only — validate on your own fixtures before automating high-stakes actions.
- Keep `litert-lm serve` warm for low latency in scripts and agents.
- Cloud Jev remains available with `--backend cloud` (default) and `TYPESAFE_API_KEY`.
- In local mode, `usage.input_tokens` counts **embedding HTTP requests** (one batch per question), not LLM tokens.

# jeb local backend demo

**Watch:** https://asciinema.org/a/6pEADiZhEdSdhIff

[![asciicast](https://asciinema.org/a/6pEADiZhEdSdhIff.svg)](https://asciinema.org/a/6pEADiZhEdSdhIff)

Local-only walkthrough (typed prompts, JSON fixtures, real `jeb` output).

```bash
.venv/bin/litert-lm serve --host 127.0.0.1 --port 9380
cargo build --release && ./demos/record-local-demos.sh
asciinema upload demos/jeb-local.cast
```

Recorder: `record_jeb_local_demo.py` · Fixtures: `fixtures/*.json`

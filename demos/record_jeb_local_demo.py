#!/usr/bin/env python3
"""Polished asciinema demo: jeb local backend + EmbeddingGemma 2 (LiteRT-LM)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

DEMO_DIR = Path(__file__).resolve().parent
ROOT = DEMO_DIR.parent
FIXTURES = DEMO_DIR / "fixtures"

CHAR_DELAY = 0.026
PAUSE_STEP = 0.9

R = "\033[0m"
B = "\033[1m"
CYAN = "\033[36m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
GRAY = "\033[90m"
DIM = "\033[2m"

WIDTH = 100


def out(text: str, delay: float | None = None) -> None:
    if delay is None:
        sys.stdout.write(text)
        sys.stdout.flush()
        return
    for ch in text:
        sys.stdout.write(ch)
        sys.stdout.flush()
        time.sleep(delay)


def banner() -> None:
    line = "═" * WIDTH
    out(f"\r\n{B}{CYAN}{line}{R}\r\n")
    out(f"{B}{GREEN}  jeb + EmbeddingGemma 2 on-device (LiteRT-LM local backend){R}\r\n")
    out(
        f"{GRAY}  System One JSON in  |  embedding classification out  |  no cloud API key{R}\r\n"
    )
    out(f"{B}{CYAN}{line}{R}\r\n\r\n")
    time.sleep(0.7)


def step(num: int, title: str) -> None:
    time.sleep(PAUSE_STEP)
    out(f"\r\n{B}{YELLOW}▶ Step {num}: {title}{R}\r\n")


def prompt(cmd: str) -> None:
    time.sleep(0.4)
    out(f"{B}{GREEN}dev:~${R} ")
    out(cmd, CHAR_DELAY)
    out("\r\n")
    time.sleep(0.3)


def note(text: str) -> None:
    out(f"{GRAY}{text}{R}\r\n")
    time.sleep(0.2)


def show_json(path: Path) -> None:
    pretty = json.dumps(json.loads(path.read_text()), indent=2)
    for line in pretty.splitlines():
        out(f"{DIM}{line}{R}\r\n", 0.003)


def jeb_run(
    jeb: Path,
    embed_url: str,
    model: str,
    fixture: Path | None,
    extra: list[str],
    env_extra: dict[str, str] | None = None,
) -> None:
    env = {
        **os.environ,
        "JEB_BACKEND": "local",
        "LITERT_EMBED_URL": embed_url,
        "LITERT_MODEL": model,
        **(env_extra or {}),
    }
    cmd = [
        str(jeb),
        "--backend",
        "local",
        "--litert-embed-url",
        embed_url,
        "--litert-model",
        model,
        "--pretty",
        *extra,
    ]
    stdin = fixture.open("rb") if fixture else subprocess.DEVNULL
    try:
        proc = subprocess.run(cmd, stdin=stdin, env=env, capture_output=True, check=False)
    finally:
        if fixture:
            stdin.close()
    if proc.returncode != 0:
        out(proc.stderr.decode() or f"jeb failed ({proc.returncode})\r\n")
        sys.exit(proc.returncode)
    time.sleep(0.25)
    out(proc.stdout.decode())


def main() -> None:
    jeb = Path(os.environ.get("JEB_BIN", ROOT / "target/release/jeb"))
    embed_url = os.environ.get("LITERT_EMBED_URL", "http://127.0.0.1:9380")
    model = os.environ.get("LITERT_MODEL", "embeddinggemma-2-740m.litertlm,cpu")

    if not jeb.is_file():
        print(f"jeb binary not found: {jeb}", file=sys.stderr)
        sys.exit(1)

    noul = FIXTURES / "noul-rollback.json"
    choice = FIXTURES / "choice-triage.json"
    score = FIXTURES / "score-severity.json"

    banner()

    step(1, "Local stack (already running for this demo)")
    note(f"[1] litert-lm serve  ->  {embed_url}/v1/embeddings")
    note(f"[2] model id         ->  {model}")
    note("[3] jeb --backend local compares query vs option embeddings (softmax)")
    prompt("export JEB_BACKEND=local")

    step(2, "Yes/no question (pipe System One JSON on stdin)")
    prompt(f"cat {noul.name}")
    show_json(noul)
    prompt(f"jeb --backend local --decision-only < {noul.name}")
    jeb_run(jeb, embed_url, model, noul, ["--decision-only"])

    step(3, "Multiple choice (on-call triage)")
    prompt(f"cat {choice.name}")
    show_json(choice)
    prompt(f"jeb --backend local --decision-only < {choice.name}")
    jeb_run(jeb, embed_url, model, choice, ["--decision-only"])

    step(4, "Score scale (full answer with probabilities)")
    prompt(f"cat {score.name}")
    show_json(score)
    prompt(f"jeb --backend local < {score.name}")
    jeb_run(jeb, embed_url, model, score, [])

    out(f"\r\n{GRAY}Local backend ready for scripts: export JEB_BACKEND=local{R}\r\n")


if __name__ == "__main__":
    main()

#!/usr/bin/env python
"""Paso 2: entrenar el adapter LoRA."""

from __future__ import annotations

import sys

from _bootstrap import setup

setup()

from juniperus.train import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

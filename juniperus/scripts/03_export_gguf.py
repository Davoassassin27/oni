#!/usr/bin/env python
"""Paso 3: mergear el adapter y exportar a GGUF."""

from __future__ import annotations

import sys

from _bootstrap import setup

setup()

from juniperus.export import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

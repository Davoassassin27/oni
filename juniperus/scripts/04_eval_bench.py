#!/usr/bin/env python
"""Paso 4: evaluar contra el bench y escribir reportes."""

from __future__ import annotations

import sys

from _bootstrap import setup

setup()

from juniperus.evaluate import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

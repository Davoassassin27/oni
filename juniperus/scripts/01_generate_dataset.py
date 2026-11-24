#!/usr/bin/env python
"""Paso 1: generar el dataset sintético (offline y/o profesor)."""

from __future__ import annotations

import sys

from _bootstrap import setup

setup()

from juniperus.dataset import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

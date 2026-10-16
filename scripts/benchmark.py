#!/usr/bin/env python3
"""Compatibility entry point for the failure-recovery regression suite."""
import runpy
from pathlib import Path
runpy.run_path(str(Path(__file__).with_name('a1_benchmark.py')), run_name='__main__')

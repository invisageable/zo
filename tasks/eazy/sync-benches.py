#!/usr/bin/env python3
"""Sync criterion benchmark reports to docs/benches/eazy/ and fix relative paths."""

import shutil
import re
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent
SRC = ROOT / "target" / "criterion"
DST = ROOT / "docs" / "benches" / "eazy"

def sync():
  if not SRC.exists():
    print(f"error: {SRC} does not exist. Run benchmarks first.")
    return

  # Remove old and copy fresh
  if DST.exists():
    shutil.rmtree(DST)
  shutil.copytree(SRC, DST)
  print(f"copied: {SRC.relative_to(ROOT)} -> {DST.relative_to(ROOT)}")

  # Move report/index.html to root index.html and fix paths
  report_index = DST / "report" / "index.html"
  root_index = DST / "index.html"

  if report_index.exists():
    content = report_index.read_text()
    # Remove ../ since we're moving to root level
    fixed = re.sub(r'href="\.\./([^"]+)"', r'href="\1"', content)
    root_index.write_text(fixed)
    print(f"  created: {root_index.relative_to(ROOT)}")
    # Remove report folder (only contains the index)
    shutil.rmtree(DST / "report")

  print("done.")

if __name__ == "__main__":
  sync()
"""Puts the in-repo package first on the path so the suite tests this checkout, not
whatever happens to be installed — a stale editable install from a since-renamed repo path
was found shadowing the real package with a broken module (HyperCast's zero-install
pattern, ported home)."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

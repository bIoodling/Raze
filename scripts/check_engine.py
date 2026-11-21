"""Check the preserved engine against the user's uploaded archive. Runs in CI only."""
import re
import zipfile
from pathlib import Path

with zipfile.ZipFile("Raze_Macro_Source_For_Codex.zip") as archive:
    original = archive.read("RazeMacro/src/main.rs").decode("utf-8")
start = original.index("fn macro_worker(")
end = original.index("unsafe fn register_raw_input(", start)
current = Path("RazeMacro/src/engine.rs").read_text(encoding="utf-8")
current = current[current.index("fn macro_worker("):]
# rustfmt only changes whitespace and trailing commas.
normalize = lambda s: re.sub(r"[\s,]", "", s)
assert normalize(original[start:end]) == normalize(current), "Original engine timing changed"
print("PASS: original macro_worker timing loop preserved")

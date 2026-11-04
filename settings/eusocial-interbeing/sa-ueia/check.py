#!/usr/bin/env python3
"""Sa'ueia workshop checker: join morphemes by table.yaml's rules, flag violations.

Reads table.yaml next to this file (or a path argument). The consonant inventory
is open, so no consonant is ever flagged; only rules in the table are enforced.
Exit status is 1 when any error is found; warnings do not fail the run.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path

import yaml

TABLE = Path(__file__).with_name("table.yaml")
PROVENANCES = {"deru", "weksa-placeholder"}


@dataclass(frozen=True)
class Finding:
    severity: str  # "error" | "warning"
    where: str
    message: str

    def __str__(self) -> str:
        return f"{self.severity}: {self.where}: {self.message}"


def letters(form: str) -> str:
    return form.replace("'", "").lower()


def join(morphemes: list[str]) -> str:
    """Concatenate, merging one identical letter at each join."""
    out = ""
    for m in morphemes:
        if out and m and out[-1] == m[0]:
            m = m[1:]
        out += m
    return out


def check_form(form: str, rules: dict, where: str, *, inside_morpheme: bool = False) -> list[Finding]:
    found = []
    text = letters(form)
    for ch in rules["forbidden_letters"]["value"]:
        if ch in text:
            found.append(Finding("error", where, f"{form!r} contains forbidden letter {ch!r}"))
    for a, b in zip(text, text[1:]):
        if a == b and (not inside_morpheme or rules["no_doubled_letters_inside_morpheme"]["value"]):
            found.append(Finding("error", where, f"{form!r} has doubled {a!r}"))
    limit = rules["max_word_letters"]
    if not inside_morpheme and len(text) > limit["value"]:
        found.append(Finding(limit["severity"], where, f"{form!r} is {len(text)} letters, over {limit['value']}"))
    return found


def check_table(table: dict) -> list[Finding]:
    rules = table["rules"]
    found = []
    for r_name, r in rules.items():
        if r.get("provenance") not in PROVENANCES:
            found.append(Finding("error", f"rules.{r_name}", "missing or unknown provenance"))
    if table["consonants"]["inventory"] != "open":
        found.append(Finding("error", "consonants", "inventory is not Deru's to guess; keep it open until given"))
    forms: dict[str, str] = {}
    for kind in ("roots", "suffixes"):
        for entry in table[kind]:
            form = entry["form"]
            where = f"{kind}.{form}"
            if entry.get("provenance") not in PROVENANCES:
                found.append(Finding("error", where, "missing or unknown provenance"))
            forms[form] = kind
            found += check_form(form, rules, where, inside_morpheme=True)
    for comp in table["compositions"]:
        where = "compositions." + "+".join(comp["morphemes"])
        if comp.get("provenance") not in PROVENANCES:
            found.append(Finding("error", where, "missing or unknown provenance"))
        for m in comp["morphemes"]:
            if m not in forms and not any(m == f2 for f2 in (c["expected"] for c in table["compositions"])):
                found.append(Finding("error", where, f"morpheme {m!r} is in no table row or earlier composition"))
        got = join(comp["morphemes"])
        if got != comp["expected"]:
            found.append(Finding("error", where, f"joins to {got!r}, table says {comp['expected']!r}"))
        found += check_form(comp["expected"], rules, where)
    return found


def main(argv: list[str]) -> int:
    path = Path(argv[1]) if len(argv) > 1 else TABLE
    findings = check_table(yaml.safe_load(path.read_text(encoding="utf-8")))
    for f in findings:
        print(f)
    print(f"{sum(f.severity == 'error' for f in findings)} error(s), "
          f"{sum(f.severity == 'warning' for f in findings)} warning(s)")
    return 1 if any(f.severity == "error" for f in findings) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

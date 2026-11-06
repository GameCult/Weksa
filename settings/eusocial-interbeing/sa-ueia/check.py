#!/usr/bin/env python3
"""Sa'ueia and Airawa workshop checker: join morphemes by table.yaml's rules,
flag violations.

usage: check.py [table.yaml] [--names names.yaml]

Reads table.yaml next to this file (or a path argument). With --names it also
joins every proposed name in that file from its morphemes and runs the result
through the same rules for its language. The consonant inventory is open, so no
consonant is ever flagged for being unlisted; only rules in the table are
enforced. Deru's phonotactics (2026-11-06) bind both languages: no two
consonants adjacent, no two identical vowels adjacent, k and never q.
Exit status is 1 when any error is found; warnings do not fail the run.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path

import yaml

TABLE = Path(__file__).with_name("table.yaml")
PROVENANCES = {"deru", "weksa-placeholder"}
LANGUAGES = ("saueia", "airawa")


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


def units(text: str, rules: dict) -> list[tuple[str, bool]]:
    """Split letters into (unit, is_vowel); a listed digraph is one consonant."""
    vowels = set(rules["vowels"]["value"])
    digraphs = rules["no_adjacent_consonants"].get("digraphs", [])
    out, i = [], 0
    while i < len(text):
        if text[i : i + 2] in digraphs:
            out.append((text[i : i + 2], False))
            i += 2
        else:
            out.append((text[i], text[i] in vowels))
            i += 1
    return out


def forbidden(rules: dict, language: str) -> list[str]:
    r = rules["forbidden_letters"]
    return list(r["value"]) if language == "saueia" else list(r.get("airawa", []))


def check_form(form: str, rules: dict, where: str, *, inside_morpheme: bool = False,
               language: str = "saueia") -> list[Finding]:
    found = []
    text = letters(form)
    for ch in forbidden(rules, language):
        if ch in text:
            found.append(Finding("error", where, f"{form!r} contains forbidden letter {ch!r}"))
    for (a, a_vowel), (b, b_vowel) in zip(units(text, rules), units(text, rules)[1:]):
        if not a_vowel and not b_vowel and rules["no_adjacent_consonants"]["value"]:
            found.append(Finding("error", where, f"{form!r} has adjacent consonants {a + b!r}"))
        if a_vowel and b_vowel and a == b and rules["no_identical_adjacent_vowels"]["value"]:
            found.append(Finding("error", where, f"{form!r} has identical adjacent vowels {a + b!r}"))
    limit = rules["max_word_letters"]
    if language == "saueia" and not inside_morpheme and len(text) > limit["value"]:
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


def check_names(table: dict, doc: dict) -> list[Finding]:
    """Join each proposed name from its morphemes and run it through the rules.

    Morphemes come from the table (Sa'ueia only) and from the roots in `doc`,
    per language. A name's written form must equal the join, ignoring case.
    """
    rules = table["rules"]
    found = []
    known: dict[str, dict[str, str]] = {lang: {} for lang in LANGUAGES}
    for kind in ("roots", "suffixes"):
        for entry in table[kind]:
            known["saueia"][entry["form"]] = "table"
    for lang, entries in doc.get("roots", {}).items():
        if lang not in LANGUAGES:
            found.append(Finding("error", f"roots.{lang}", "unknown language"))
            continue
        for entry in entries:
            where = f"roots.{lang}.{entry['form']}"
            if entry.get("provenance") not in PROVENANCES:
                found.append(Finding("error", where, "missing or unknown provenance"))
            known[lang][entry["form"]] = "names"
            found += check_form(entry["form"], rules, where, inside_morpheme=True, language=lang)
    for name in doc["names"]:
        lang = name.get("language")
        where = f"names.{name['form']}"
        if lang not in LANGUAGES:
            found.append(Finding("error", where, "missing or unknown language"))
            continue
        for m in name["morphemes"]:
            if m not in known[lang]:
                found.append(Finding("error", where, f"morpheme {m!r} is in no {lang} row"))
        got = join(name["morphemes"])
        if got != letters(name["form"]):
            found.append(Finding("error", where, f"joins to {got!r}, name says {name['form']!r}"))
        found += check_form(name["form"], rules, where, language=lang)
    return found


def main(argv: list[str]) -> int:
    args = argv[1:]
    names_path = None
    if "--names" in args:
        i = args.index("--names")
        names_path = Path(args[i + 1])
        del args[i : i + 2]
    path = Path(args[0]) if args else TABLE
    table = yaml.safe_load(path.read_text(encoding="utf-8"))
    findings = check_table(table)
    if names_path:
        findings += check_names(table, yaml.safe_load(names_path.read_text(encoding="utf-8")))
    for f in findings:
        print(f)
    print(f"{sum(f.severity == 'error' for f in findings)} error(s), "
          f"{sum(f.severity == 'warning' for f in findings)} warning(s)")
    return 1 if any(f.severity == "error" for f in findings) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

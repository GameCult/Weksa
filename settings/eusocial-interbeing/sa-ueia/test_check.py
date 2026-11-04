import copy
import unittest

import yaml

import check


def table():
    return yaml.safe_load(check.TABLE.read_text(encoding="utf-8"))


def errors(t):
    return [f for f in check.check_table(t) if f.severity == "error"]


class JoinTests(unittest.TestCase):
    def test_identical_letters_merge_at_join(self):
        self.assertEqual(check.join(["weru", "um"]), "werum")
        self.assertEqual(check.join(["ila", "werum"]), "ilawerum")

    def test_merge_is_one_letter_not_all(self):
        self.assertEqual(check.join(["ila", "aba"]), "ilaba")
        self.assertEqual(check.join(["ila", "ala"]), "ilala")

    def test_different_letters_concatenate(self):
        self.assertEqual(check.join(["ila", "um"]), "ilaum")

    def test_merge_applies_only_at_joins(self):
        # a doubled letter inside a morpheme is not collapsed by join
        self.assertEqual(check.join(["aa", "b"]), "aab")


class TableTests(unittest.TestCase):
    def test_shipped_table_is_clean(self):
        self.assertEqual(errors(table()), [])

    def test_forbidden_letter_in_root_flagged(self):
        t = table()
        t["roots"].append({"form": "haru", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertTrue(any("forbidden letter 'h'" in str(f) for f in errors(t)))

    def test_q_and_y_flagged(self):
        for ch in "qy":
            t = table()
            t["roots"].append({"form": f"a{ch}a", "gloss": "x", "provenance": "deru"})
            self.assertTrue(any(f"forbidden letter '{ch}'" in str(f) for f in errors(t)), ch)

    def test_k_allowed(self):
        t = table()
        t["roots"].append({"form": "kela", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertEqual(errors(t), [])

    def test_missing_or_unknown_provenance_flagged(self):
        for bad in (None, "guess"):
            t = table()
            t["suffixes"][0]["provenance"] = bad
            self.assertTrue(any("provenance" in str(f) for f in errors(t)), bad)

    def test_unmerged_composition_flagged(self):
        t = table()
        t["compositions"][0]["expected"] = "weruum"
        msgs = [str(f) for f in errors(t)]
        self.assertTrue(any("joins to 'werum'" in m for m in msgs))
        self.assertTrue(any("doubled 'u'" in m for m in msgs))

    def test_doubled_letter_inside_morpheme_flagged(self):
        t = table()
        t["roots"].append({"form": "ilaa", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertTrue(any("doubled 'a'" in str(f) for f in errors(t)))

    def test_unknown_morpheme_in_composition_flagged(self):
        t = table()
        t["compositions"].append({"morphemes": ["ila", "zo"], "expected": "ilazo", "provenance": "weksa-placeholder"})
        self.assertTrue(any("'zo'" in str(f) for f in errors(t)))

    def test_long_word_is_warning_not_error(self):
        t = table()
        t["compositions"].append({"morphemes": ["ila", "werum", "ila", "werum"], "expected": "ilawerumilawerum",
                                  "provenance": "weksa-placeholder"})
        self.assertEqual(errors(t), [])
        self.assertTrue(any(f.severity == "warning" for f in check.check_table(t)))

    def test_consonant_inventory_must_stay_open(self):
        t = table()
        t["consonants"]["inventory"] = ["l", "m"]
        self.assertTrue(any("inventory" in str(f) for f in errors(t)))

    def test_apostrophe_is_not_a_letter(self):
        self.assertEqual(check.letters("Sa'ueia"), "saueia")


if __name__ == "__main__":
    unittest.main()

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
        self.assertTrue(any("identical adjacent vowels 'uu'" in m for m in msgs))

    def test_identical_vowels_inside_morpheme_flagged(self):
        t = table()
        t["roots"].append({"form": "ilaa", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertTrue(any("identical adjacent vowels 'aa'" in str(f) for f in errors(t)))

    def test_different_adjacent_vowels_allowed(self):
        t = table()
        t["roots"].append({"form": "iela", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertEqual(errors(t), [])

    def test_adjacent_consonants_inside_morpheme_flagged(self):
        t = table()
        t["roots"].append({"form": "ilma", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertTrue(any("adjacent consonants 'lm'" in str(f) for f in errors(t)))

    def test_doubled_consonant_is_adjacent_consonants(self):
        t = table()
        t["roots"].append({"form": "illa", "gloss": "x", "provenance": "weksa-placeholder"})
        self.assertTrue(any("adjacent consonants 'll'" in str(f) for f in errors(t)))

    def test_join_that_leaves_two_consonants_flagged(self):
        t = table()
        t["roots"].append({"form": "sal", "gloss": "x", "provenance": "weksa-placeholder"})
        t["roots"].append({"form": "ma", "gloss": "x", "provenance": "weksa-placeholder"})
        t["compositions"].append({"morphemes": ["sal", "ma"], "expected": "salma", "provenance": "weksa-placeholder"})
        self.assertTrue(any("adjacent consonants 'lm'" in str(f) for f in errors(t)))

    def test_consonant_merge_at_join_is_clean(self):
        t = table()
        t["roots"].append({"form": "sal", "gloss": "x", "provenance": "weksa-placeholder"})
        t["roots"].append({"form": "la", "gloss": "x", "provenance": "weksa-placeholder"})
        t["compositions"].append({"morphemes": ["sal", "la"], "expected": "sala", "provenance": "weksa-placeholder"})
        self.assertEqual(errors(t), [])

    def test_apostrophe_does_not_separate_consonants(self):
        rules = table()["rules"]
        self.assertTrue(check.check_form("al'ma", rules, "x"))

    def test_th_is_one_consonant_but_sh_is_two(self):
        rules = table()["rules"]
        self.assertEqual(check.check_form("kerath", rules, "x", language="airawa"), [])
        self.assertTrue(check.check_form("kesha", rules, "x", language="airawa"))
        self.assertTrue(check.check_form("kethr", rules, "x", language="airawa"))

    def test_final_consonant_is_not_a_cluster(self):
        self.assertEqual(check.check_form("werum", table()["rules"], "x"), [])

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

    def test_the_rules_bind_both_languages(self):
        rules = table()["rules"]
        for lang in check.LANGUAGES:
            self.assertTrue(check.check_form("qara", rules, "x", language=lang), lang)
            self.assertTrue(check.check_form("goraa", rules, "x", language=lang), lang)
            self.assertTrue(check.check_form("gordak", rules, "x", language=lang), lang)

    def test_h_and_y_are_not_forbidden_for_airawa(self):
        rules = table()["rules"]
        self.assertEqual(check.check_form("hayo", rules, "x", language="airawa"), [])
        self.assertTrue(check.check_form("hayo", rules, "x", language="saueia"))

    def test_apostrophe_is_not_a_letter(self):
        self.assertEqual(check.letters("Sa'ueia"), "saueia")


NAMES = check.Path(__file__).with_name("names.yaml")


def names():
    return yaml.safe_load(NAMES.read_text(encoding="utf-8"))


def name_errors(doc):
    return [f for f in check.check_names(table(), doc) if f.severity == "error"]


class NameTests(unittest.TestCase):
    def test_every_proposed_name_passes(self):
        self.assertEqual(name_errors(names()), [])

    def test_the_closed_syllable_airawa_drafts_all_fail(self):
        # The first draft's Airawa names, built from closed-syllable roots.
        old = {"Gordak": ["gor", "dak"], "Gorqerath": ["gor", "qerath"], "Zegdak": ["zeg", "dak"],
               "Gorsar": ["gor", "sar"], "Tezsar": ["tez", "sar"], "Sartezur": ["sar", "tez", "ur"]}
        rules = table()["rules"]
        for form, parts in old.items():
            self.assertTrue(check.check_form(form, rules, form, language="airawa"), form)

    def test_unknown_morpheme_flagged(self):
        doc = names()
        doc["names"].append({"form": "Zoila", "language": "saueia", "morphemes": ["zo", "ila"]})
        self.assertTrue(any("'zo'" in str(f) for f in name_errors(doc)))

    def test_name_must_equal_its_join(self):
        doc = names()
        doc["names"].append({"form": "Ilawerum", "language": "saueia", "morphemes": ["ila", "weru"]})
        self.assertTrue(any("joins to 'ilaweru'" in str(f) for f in name_errors(doc)))

    def test_a_cluster_in_a_proposed_name_is_flagged(self):
        doc = names()
        doc["roots"]["airawa"].append({"form": "dak", "gloss": "x", "provenance": "weksa-placeholder"})
        doc["names"].append({"form": "Goradak", "language": "airawa", "morphemes": ["gora", "dak"]})
        self.assertEqual(name_errors(doc), [])  # open join, final consonant ok
        doc["names"].append({"form": "Gordak", "language": "airawa", "morphemes": ["gor", "dak"]})
        self.assertTrue(any("adjacent consonants 'rd'" in str(f) for f in name_errors(doc)))

    def test_unknown_language_flagged(self):
        doc = names()
        doc["names"].append({"form": "Kasu", "language": "klingon", "morphemes": ["kasu"]})
        self.assertTrue(any("language" in str(f) for f in name_errors(doc)))


if __name__ == "__main__":
    unittest.main()

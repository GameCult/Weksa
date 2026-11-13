# Project Gutenberg terms: bulk access and header stripping

Read 2026-11-13 for follow-up `gutenberg-terms` (campaign `weksa-lowering`),
ahead of cut `pd-importer`. Pages: gutenberg.org `policy/permission.html`,
`policy/terms_of_use.html`, `policy/robot_access.html`, `policy/license.html`.
The text came through a page-to-markdown fetch with a small model answering
over it, so the short quotes below are not verbatim-verified. `pd-importer`'s
Imagination re-reads the four pages before it fixes a fetch path.

## Findings

- **Automated access to the main site is refused.** Terms of Use: "This website
  is intended for human users only"; "Any perceived use of automated tools to
  access this website will result in a temporary or permanent block". OPDS
  feeds: no more requests than a browser user would make; heavy requesters are
  blocked by IP.
- **Bulk has two sanctioned paths.** Terms of Use: take bulk downloads "from
  one of our mirrors, not from the main site". The robot-access page names the
  `robot/harvest` wget endpoint (`-w 2`, filterable by file type or language)
  and recommends a private mirror for a full local copy; it asks that books be
  re-checked for change rather than re-downloaded. Catalog data are offered as
  machine-readable offline catalogs, "granted to the public domain".
- **Header and licence stripping.** Licence section 1.C: for a work not
  protected by US copyright, with the user in the US, the Foundation does "not
  claim a right to prevent you from copying, distributing, performing,
  displaying or creating derivative works", "as long as all references to
  Project Gutenberg are removed". Section 1.E.2 repeats that such a derived
  work may be used with no fee. The trademark is registered; non-commercial use
  of the name needs no permission, commercial use does. 1.E.4 (do not detach
  the licence) binds a copy that keeps the Project Gutenberg name, not a derived
  work with every reference removed.
- **No statement on machine learning or AI use** on any of the four pages.
- **US public domain only.** The Foundation makes no claim about copyright
  outside the US (1.D). A work free in the US may not be free elsewhere, and
  Gutenberg's own catalog is not a per-work clearance.

## What this decides for `pd-importer`

- Fetch from a mirror or the `robot/harvest` endpoint with `-w 2`, or from an
  offline dump. Never crawl `gutenberg.org` pages or OPDS from the loop host.
- Strip the whole header and footer, every Project Gutenberg reference and the
  licence text from the stored text; keep provenance as the ebook number and
  edition in the item's source field, which is Weksa's own record and not a use
  of the mark. This is the path `docs/targets/dao-de-jing-test-bed.md` already
  takes. The remaining open question there, what counts as a "reference", stays
  open: the safe reading is that the ebook number in a provenance field is a
  catalogue identifier, not a reference inside the work, but nothing read here
  rules on it.
- Per-work check stays with the importer: the work's author and first
  publication date against US public-domain rules, recorded on the item. The
  Foundation's catalogue does not supply it. A work Gutenberg posts with the
  copyright holder's permission (1.E.3) is out of scope.
- No importer code in this note.

# Project Gutenberg terms: bulk access and header stripping

Read 2026-11-13 for follow-up `gutenberg-terms` (campaign `weksa-lowering`),
ahead of cut `pd-importer`. Pages: gutenberg.org `policy/permission.html`,
`policy/terms_of_use.html`, `policy/robot_access.html`, `policy/license.html`.
The quotes were checked against the live HTML of each page by a second reader.
`pd-importer`'s Imagination re-reads the pages before it fixes a fetch path,
since they change.

## Findings

- **Automated access to the main site is refused.** Terms of Use: "This website
  is intended for human users only"; "Any perceived use of automated tools to
  access this website will result in a temporary or permanent block". OPDS
  feeds: no more requests than a browser user would make; heavy requesters are
  blocked by IP.
- **Bulk has two sanctioned paths.** Terms of Use: take bulk downloads "from
  one of our mirrors, not from the main site". The robot-access page names the
  `robot/harvest` wget endpoint (`-w 2`, filterable by file type or language)
  and recommends a private mirror for a full local copy; it suggests checking
  periodically for changes. Catalog data are offered as
  machine-readable offline catalogs, "granted to the public domain".
- **Header and licence stripping.** Licence section 1.C: for a work not
  protected by US copyright, with the user in the US, the Foundation does "not
  claim a right to prevent you from copying, distributing, performing,
  displaying or creating derivative works", "as long as all references to
  Project Gutenberg are removed", the condition holding for a user in the US.
  Section 1.E opens "Unless you have removed all references to Project
  Gutenberg", and 1.E.1 to 1.E.7 (including 1.E.4, do not detach the licence)
  attach to a work on which the phrase "Project Gutenberg" appears or with which
  it is associated, so a text with every reference removed is outside 1.E.
  1.E.2 is not that grant: it says a PG eBook derived from texts not restricted
  by US copyright may be copied and distributed in the US without fee. The
  trademark is registered; non-commercial use of the name needs no permission,
  commercial use does.
- **No statement on machine learning or AI use** on any of the four pages.
- **US public domain only.** The Foundation makes no claim about copyright
  outside the US (1.D). A work free in the US may not be free elsewhere, and
  Gutenberg's own catalog is not a per-work clearance.

## What this decides for `pd-importer`

- Fetch from a mirror or an offline dump. The Terms of Use footnote counts "many
  books" as more than about 100 a day and sends those to mirrors, while
  `robot/harvest` is a `www.gutenberg.org` URL: use it (with `-w 2`) only for a
  handful of works. Never crawl `gutenberg.org` pages or OPDS from the loop host.
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

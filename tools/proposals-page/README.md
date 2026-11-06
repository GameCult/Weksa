# Proposals page

A static page for humans who feed Weksa ideas, so they need not review forge pull requests.

- `proposals.json`: the content, one entry per proposal (plain-language summary, evidence vs invention, stance, open questions). Hand-maintained; update it when a PR changes or is ruled on.
- `generate.py`: `python3 tools/proposals-page/generate.py [in.json] [out/index.html]`. Standard library only; output has no scripts.
- Published copy: `/srv/minds-web/weksa/proposals-<short-sha>/` on minds.gamecult.org.

Only public material belongs in `proposals.json`. The page is what the JSON says; nothing is fetched.

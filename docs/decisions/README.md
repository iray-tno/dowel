# Decisions

Design questions that were settled after enough investigation that redoing the
investigation would be wasteful — and that constrain what contributors may
change.

A record belongs here when all three hold:

- the answer is not obvious from the code, and the code alone would read as an
  arbitrary choice
- getting it wrong again is cheap and likely
- the reasoning depends on facts about someone else's system (React Native,
  the ARIA specification, a browser) that should be re-checked when that system
  moves

Each record states the decision, the evidence with file and line references so
it can be re-checked, what was rejected and why, and what would make it worth
reopening.

`proposal.md` is the design document — where Hozo is going. These are the
smaller questions that came up on the way, and their answers.

| | |
| --- | --- |
| [001](001-disabled-and-focus.md) | `disabled` means one thing, and it is not focusable |
| [002](002-what-hozo-abstracts.md) | What Hozo abstracts, and what it carries |

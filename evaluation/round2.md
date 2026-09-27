# Round 2: review yield and safe suggestions

User-selected outcome: agents should find and suggest more and better fixes;
human prompting to implement refactors is acceptable. The contract is to repair
the observed misleading resolver result, compare complete production reviews
with and without the revised tool, and independently validate actionable
suggestions and their resulting patches. A tie or loss remains in the record.

This is the second development/evaluation round under the project policy's
three-round fuse. It is exploratory: the projects were used in round 1 and the
resolver was improved using that evidence. It is not held-out confirmation.

Inputs remain the pinned PostHog and Alien Signals projects in the original
protocol, using fresh isolated checkouts and fresh GPT-6 Sol / medium agents.
Both conditions receive the same production-review task. Only the assisted
condition receives the new frozen analyzer. The review must cover production
modules, not stop after a selected repair. There is no finding quota or invented
time/token budget. Existing tests and local documentation define behavior.

Agents first write suggestions and executable evidence without modifying
production source. The evaluator adjudicates every suggestion. Confirmed
behavior defects require executable evidence against the original plus a
documented or clearly established expected behavior. Refactors require a
specific present maintenance problem, a concrete proposed simplification, and
characterization coverage for affected behavior. Style preferences and reducing
an analyzer score alone do not qualify. Refactors and bugs are separate counts;
duplicate symptoms or the same refactoring mechanism are not inflated by file
count. Unsupported candidates and false leads remain visible.

After adjudication, agents receive the same follow-up form asking them to
implement their verified proposals, preserve APIs, record before/after evidence,
and run the original suite. The evaluator independently replays tests and
patches. A safe patch requires its new regressions/characterizations and the
original suite to pass plus source review for changed behavior. No upstream
contribution is authorized.

Primary observations: verified defects, justified refactor proposals, validated
patches, unsupported claims, and introduced regressions per project/condition.
Report coverage and concrete incremental findings, not an invented combined
quality score. Report any practical advantage as an observed result for these
reviews, with stochastic variation, unblinded adjudication, and project reuse
explicit. Do not keep changing criteria until a favored condition wins.

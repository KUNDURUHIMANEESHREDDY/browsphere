# Structured public train verification (v13.1)

Baseline: IRCTC returned four schedules for the correct route and date, including two evening departures, but the model verifier returned insufficient evidence. The old benchmark also depended on literal result-page wording absent from the actual capture. This targeted fix works on the merged main checkout; the separate Phase 8 worktree is unchanged.

The native verifier parses the unique result heading, explicit ordered cities and date/year, station alternatives observed in route filters, five-digit train identities, each card's scheduled local departure and arrival clocks/dates, and journey duration. It validates that elapsed duration agrees with both arrival date and time, rejects incomplete/duplicate or mismatched records and checks that every parsed schedule is in the retained exact source. It requires an acknowledged public search and the same secure origin. Only matching departure-window records are reported. The verifier does not interpret refresh buttons as availability or perform bookings. It supports the observed IRCTC-style layout and explicit lower-bound India-time constraints, not arbitrary railway interfaces, arbitrary date/time constraints, or zero-result searches.

The independent JavaScript oracle uses a separate card parser and local-time arithmetic. It requires exact source quotes, all matching train records, no extraneous nonmatching records, and an explicit seat-availability limitation. This replaces the old literal phrase predicate, not the original task objective. The original completed/verified and local model action requirements remain. Compare previous and new scores with this benchmark revision noted.

Validation:

    cargo test -p browsphere-core --offline -- --test-threads=2
    node --test tests/e2e/train-result-oracle.test.cjs
    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --case level-1 --train-date 2026-11-25

A sanitized public schedule capture from the earlier real-site benchmark is retained in fixtures/public-train-results-v13.txt for positive and negative tests. It is historical test data, not current train availability. No account or passenger data is included.

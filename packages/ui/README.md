# UI contracts

Framework-neutral view-model shapes for the stream command center, strategy
workspace, trade ticket, and positions workspace. This package contains no
wallet, database, collector, or execution implementation. Unlike the other
legacy TypeScript backend packages, this remains a frontend-only boundary.

The web skeleton still uses a local empty token view model. Live data will
replace it through an explicit mapping from Rust API contracts into this
boundary rather than exposing backend domain records or PostgreSQL rows
directly. The Discovery projection contains approved rows only; pending and
rejected candidates use compact screening-summary and rejection-log
projections.

Paper and Live tickets are a discriminated union. Paper tickets can be recorded
without wallet confirmation, while Live tickets retain signing and submission
states. Position projections and the outer command-center model carry the same
explicit mode boundary, so contradictory mode combinations do not type-check.

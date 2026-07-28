# UI contracts

Framework-neutral view-model shapes for the stream command center, strategy
workspace, trade ticket, and held-position tray. This package contains no wallet or
execution implementation.

The web skeleton still uses a local empty token view model. Live data will
replace it through an explicit API-to-view projection into this boundary rather
than exposing backend domain records directly.

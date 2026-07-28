# PnL engine (deferred TypeScript scaffold)

This package records the earlier TypeScript calculation sketch. Paper trading
and portfolio work are deferred; their implementation will use Rust domain and
persistence boundaries.

Realized, mark-to-market, and executable-exit PnL remain separate calculations,
and values always carry their pricing basis and freshness timestamp. Do not add
new backend implementation here.

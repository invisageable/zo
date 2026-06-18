This program declares two local variables. `imu` binds an immutable value — once `language` holds `"zo"`, it can never change. `mut` binds a mutable value, so `version` can be reassigned with `=` or updated in place with a compound operator like `+=`.

Each binding names its type after the `:` — `str` for text, `int` for a whole number — and the value on the right must match it.

> *Reach for `imu` by default and only use `mut` when a value truly has to change. Immutable-by-default keeps data flow easy to follow.*

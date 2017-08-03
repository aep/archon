not usable yet, stay tuned.

if you insist:

```
cargo run --release pack .    foobar.ctxz
cargo run --release unpack    foobar.ctxz /tmp/unpacked
cargo run --release push . file:///tmp/store/foobar
cargo run --release store-init file:///tmp/store/
cargo run --release push foobar.ctxz file:///tmp/store/foobar
cargo run --release pull file:///tmp/store/foobar /tmp/pulled
```

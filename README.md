# Elite Trade Finder

Real-time trade route finder for Elite Dangerous. Subscribes directly to EDDN, finds single-hop / loop / rare-chain routes, and alerts on hot routes faster than aggregators.

**Status:** In development.

## Dev setup

```
bun install
bun run tauri dev
```

Requires Rust 1.78+, Bun, and Tauri prerequisites for your OS.

## Linux: AppImage launch quirks

On bleeding-edge Arch / CachyOS, the AppImage's bundled `libwayland-client.so` can ABI-drift against system Mesa and produce `EGL_BAD_PARAMETER` at startup (white window). Workaround:

```
LD_PRELOAD=/usr/lib/libwayland-client.so ./Elite.Trade.Finder_X.Y.Z_amd64.AppImage
```

Cleaner option: use the Arch-built `.rpm` from the release page (files with `-arch` suffix) — it dynamically links to your system libs instead of bundled ones. Extract with `bsdtar -xvf *.rpm` and run `./usr/bin/elite-trade-finder`.

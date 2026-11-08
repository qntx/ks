# ks

Personal local secret vault with an age-compatible on-disk format.

`ks` keeps secrets in per-entry [age](https://age-encryption.org)-encrypted
files in a local directory that can be synced between your own devices with
plain git. Every file can always be recovered with the official `age` CLI —
no dependency on `ks` itself.

> **Status:** being rewritten from scratch towards 0.8.0. The 0.7 code, file
> formats and CLI were removed without migration.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

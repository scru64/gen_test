# SCRU64 Generator Tester

[![GitHub tag](https://img.shields.io/github/v/tag/scru64/gen_test)](https://github.com/scru64/gen_test)
[![License](https://img.shields.io/github/license/scru64/gen_test)](https://github.com/scru64/gen_test/blob/main/LICENSE)

A command-line SCRU64 tester that tests if a generator generates monotonically
ordered IDs, sets up-to-date timestamps, and so on.

## Usage

```bash
any-command-that-prints-identifiers-infinitely | scru64-test
```

## Installation

[Install Rust](https://www.rust-lang.org/tools/install) and build from source:

```bash
cargo install --git https://github.com/scru64/gen_test.git
```

## License

Copyright 2023 LiosK

Licensed under the Apache License, Version 2.0.

## See also

- [SCRU64 Specification](https://github.com/scru64/spec)

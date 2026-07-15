# Cross-component tests

- `fixtures/`: synthetic repositories and event payloads
  - `schema-invalid/`: reviewed negative public-contract examples
- `integration/`: storage, API, worker, and CLI integration tests
- `golden/`: reviewed versioned output used for compatibility tests

The public JSON contract goldens are validated with:

```sh
cargo test -p assay-cli --test schema_contracts
```

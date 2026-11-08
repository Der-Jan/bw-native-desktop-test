Pinned upstream: bitwarden/sdk-internal a13b4c4e1fa3685280241f417920fd19d7ef24a6.

The native feature exposes the existing Noise provider and transport/session traits
for native Rust adapters. No cryptographic or protocol behavior changes.
NoEncryptionCryptoProvider remains gated behind tests/test-support. Cargo metadata
is normalized to explicit pinned dependencies for this workspace.

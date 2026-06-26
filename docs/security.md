# Security Model

My AI Bag treats AI coding setup files as sensitive by default.

## Current Rules

- Scanning is local.
- Pack preview output does not include file contents.
- Secret-looking files are detected by name, such as `auth`, `token`, `credential`, `session`, `.env`, and similar terms.
- CLI export encrypts the archive with Argon2id key derivation and XChaCha20-Poly1305 authenticated encryption.
- The UI does not export yet; it previews what would be included.

## Encryption Details

The prototype archive format stores an encrypted JSON payload inside a small envelope. The passphrase is processed with Argon2id using a per-archive random salt to produce a 256-bit key. The payload is encrypted with XChaCha20-Poly1305 using a per-archive random nonce, so tampering should fail decryption instead of silently producing altered files.

This is a reasonable prototype choice, but it is not a final audited backup format.

## Current Limitations

- The archive format is a prototype and may change.
- There is no restore flow yet.
- There is no hosted sync service.
- There is no per-file approval UI yet.
- Filename heuristics can miss tool-specific secrets or classify harmless files as sensitive.

## Handling `.aibag` Files

Treat an `.aibag` file like a password manager export. It may contain auth files, tokens, settings, and private skills from selected tools.

Use long passphrases, keep exports local, and delete test archives when you are done with them.

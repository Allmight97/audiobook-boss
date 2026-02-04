# Audible Import Prototype Notes

## Goal
- Provide a minimal in-app path to decrypt Audible `.aax` downloads into `.m4b` files and load them into the existing Audiobook Boss workflow.

## Minimal workflow
- Select one or more `.aax` files.
- Provide Audible activation bytes (16 hex characters / 8 bytes).
- Decrypt to temporary `.m4b` files and load them into the file list for muxing/shrinking.
- Remove original `.aax` downloads unless the user opts in to keeping them.

## Metadata & security primitives
- Preserve embedded metadata by stream-copying and retaining container/stream tags.
- Require input path validation and activation byte sanity checks before decryption.

## UX impact
- Uses a dedicated modal to avoid interrupting the primary import workflow.
- Keeps the main file list and processing pipeline unchanged by returning standard `FileListInfo`.

## Libation reference
- Direct repository inspection is pending; the environment returned a 403 when attempting to clone Libation.

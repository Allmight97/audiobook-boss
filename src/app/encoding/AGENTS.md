# Encoding Configuration

## Scope

- The engine owns audio choices: the defaults new titles start from, each
  title's own choice, the edit rules, and capability facts
  (`crates/abb-engine/src/session/AGENTS.md`). This owner shows them in the
  encoder panel's terms and sends edits.
- The Solid panel lives in `src/ui/encoderPanel`. It renders this owner and
  dispatches `select`; it does not keep a second encoder store.

## Public API Strip

- Import `createEncodingOwner` and owner types from `src/app/encoding`.
- `index.ts` is the export surface. `owner.ts`, `project.ts`, and `hints.ts`
  are private.

## What Stays Here

- `project.ts` turns an engine choice and its facts into the panel view:
  labels, option lists, which options are disabled, and hint text. It holds
  no acceptance rule; an option is disabled because the engine's facts say
  so.
- `editFor` turns a control's value into a typed `AudioEdit`; a value no
  control offers sends nothing. The engine decides whether the edit applies.
- `hints.ts` words what Auto resolves to from the sources' facts. Defaults
  describe future imports, so their hints name no source.
- `selectionView` marks fields whose values differ across the selected titles.

## Testing

- `encoding.test.ts` covers the intents each control sends and the panel's
  terms for an engine choice.
- Audio choice rules are proved in `session/audio_choice_tests.rs`.
- Panel interactions live in `src/ui/__tests__/encoderPanel-*.test.tsx`.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Adding an edit rule here instead of in the engine.

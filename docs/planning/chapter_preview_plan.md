## Chapter Planning & Preview – Design Outline

### Goal
Let users see and adjust the chapter structure before processing (single or multi-file). Support:
- Viewing detected chapters (from source Timed Text / menu streams).
- Synthesizing chapters per input file when none exist.
- Renaming/reordering/removing chapters before encoding.

### UX Scope
- Input side panel or modal:
  - Show list of chapters (title, start time). For multi-file with no chapters, show one per file using filename + duration estimate.
  - Allow reordering (drag/drop), renaming titles, optional delete/merge.
  - Show a count and total duration preview.
- Output expectation: The final chapter list is what the user sees here.

### Data/Logic Changes
1) **Backend**
   - Expose a “chapter plan” structure via a new command:
     - `build_chapter_plan(files: Vec<Path>, metadata?: AudiobookMetadata)` -> returns `ChapterPlan { items: Vec<ChapterItem { title, start_ms, end_ms, source_file }>, synthesized: bool }`.
   - Use ffmpeg-next to extract chapters from each input (as we do in mux path); when none exist, synthesize one per file using duration and sanitized filename.
   - Provide a command to accept an edited plan:
     - `apply_chapter_plan(plan: ChapterPlan)` stored alongside processing context; encoder uses this plan instead of re-deriving.
   - During mux, if a plan is present, write chapters from the plan (override passthrough); otherwise fallback to passthrough/synthesis.

2) **Frontend**
   - Fetch plan after files are loaded/validated.
   - Display list with drag/drop and inline edit; persist changes client-side and send `apply_chapter_plan` before processing.
   - If no edits, the default plan is passthrough/synthesized as returned.

### Constraints & Standards
- No tag rewrites post-mux; chapters must be written by ffmpeg-next during header setup.
- Keep functions < 55 LOC; avoid growing existing large files—place plan logic in a dedicated module (e.g., `audio/chapters/plan.rs`).
- Maintain progress contract; emit a warning if a provided plan is empty.

### Incremental Steps
1) Backend: add chapter plan builder and override support in encoder setup.
2) Frontend: read-only display of computed plan.
3) Frontend: allow reorder/rename; send edited plan.
4) Polish: validation (non-empty titles), UI warnings, persistence per session.

### Risks / Mitigations
- **Chapter loss**: avoid Lofty rewrites; rely on mux-time ffmpeg-next only.
- **Plan drift**: ensure edited plan is the sole source when provided; skip passthrough in that case.
- **Performance**: chapter extraction is cheap; reuse existing input probing.


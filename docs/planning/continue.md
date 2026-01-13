⏺ Good, the skill content is now concrete. The plan is comprehensive. Let me verify I haven't missed anything by reading the current AGENTS.md one more time to ensure the line references are accurate.

⏺ Bash(wc -l AGENTS.md && head -50 AGENTS.md)
  ⎿       313 AGENTS.md            
     # Agent Role and purpose
     … +52 lines (ctrl+o to expand)

⏺ Good — 313 lines total. The plan's line references are close but may be slightly off due to the user's earlier edits (removing Rust-docs MCP section). The plan uses approximate line ranges which is fine for execution.

  The plan is complete. Let me do a final review to ensure it addresses all the user's requirements:

  Checklist:
  - ✅ lib-research skill with Context7 + btca combined
  - ✅ Trigger terms defined in description
  - ✅ Support file for btca resources
  - ✅ Full split: root + src-tauri/AGENTS.md + src/AGENTS.md
  - ✅ Skill location clarified (.codex/skills/, project-specific)
  - ✅ Concrete content (not just placeholders)
  - ✅ Verification steps
  - ✅ Rollback plan
                                                                                                                                                                   
⏺ User approved Claude's plan
  ⎿  Plan saved to: ~/.claude/plans/eager-meandering-squid.md · /plan to edit
     Plan: AGENTS.md Restructuring + Library Research Skill

     Overview

     Split the monolithic AGENTS.md (~320 lines) into a focused hierarchy per the agents.md spec, and extract Context7 + btca tooling into a dedicated skill.

     Files to Create/Modify
     ┌─────────────────────────────────────────────────────────┬────────┬────────────────────────────────────────────────────────────────────────┐
     │                          File                           │ Action │                                Purpose                                 │
     ├─────────────────────────────────────────────────────────┼────────┼────────────────────────────────────────────────────────────────────────┤
     │ AGENTS.md                                               │ Modify │ Trim to ~135 lines (role, principles, workflow, architecture pointers) │
     ├─────────────────────────────────────────────────────────┼────────┼────────────────────────────────────────────────────────────────────────┤
     │ src-tauri/AGENTS.md                                     │ Create │ Rust backend conventions (~90 lines)                                   │
     ├─────────────────────────────────────────────────────────┼────────┼────────────────────────────────────────────────────────────────────────┤
     │ src/AGENTS.md                                           │ Create │ TypeScript/UI conventions (~55 lines)                                  │
     ├─────────────────────────────────────────────────────────┼────────┼────────────────────────────────────────────────────────────────────────┤
     │ .codex/skills/lib-research/SKILL.md                     │ Create │ Context7 + btca combined skill                                         │
     ├─────────────────────────────────────────────────────────┼────────┼────────────────────────────────────────────────────────────────────────┤
     │ .codex/skills/lib-research/references/btca-resources.md │ Create │ btca resource table (support file)                                     │
     └─────────────────────────────────────────────────────────┴────────┴────────────────────────────────────────────────────────────────────────┘
     ---
     Phase 1: Create the lib-research Skill

     Skill Location

     Canonical path: .codex/skills/lib-research/
     - Symlinked via .claude/skills → ../.codex/skills
     - Project-specific (not global) because btca resources are configured for this repo

     Skill Structure

     .codex/skills/lib-research/
     ├── SKILL.md                      # Main skill file
     └── references/
         └── btca-resources.md         # btca resource table

     SKILL.md Full Content

     ---
     name: lib-research
     description: External library documentation and source code research. Use when verifying API contracts, understanding library internals, or researching how a
     dependency implements something. Triggers: Context7, btca, external docs, library source, API patterns, "how does X work".
     ---

     # Library Research

     Tools for researching external library documentation and source code.

     ## When to Use Which Tool

     | Need | Tool | Why |
     |------|------|-----|
     | API signatures, method params | **Context7** | Curated docs, fast |
     | "How does X implement Y?" | **btca** | Searches actual source code |
     | Verify a method exists | **Context7** | Authoritative docs |
     | Understand internal patterns | **btca** | See real implementation |
     | Quick API lookup | **Context7** | Faster response |
     | Debug unexpected behavior | **btca** | Source is truth |

     ## Context7 MCP

     Two-step process for authoritative library documentation.

     ### Step 1: Resolve Library ID
     mcp__context7-mcp__resolve-library-id
     Resolves library name to Context7 ID (e.g., "tauri" → `/tauri-apps/tauri`).

     ### Step 2: Query Docs
     mcp__context7-mcp__query-docs
     Fetch docs with a focused query.

     **Tips:**
     - Prefer library versions with high benchmark scores and snippet counts
     - Use for deep verification of API contracts
     - Prevents method signature hallucinations

     ## btca (Better Context)

     Queries actual source code from cloned repositories.

     ### Setup
     - **Config**: `btca.config.jsonc` in project root
     - **Auth**: Anthropic OAuth via opencode (`opencode auth list` to verify)
     - **Model**: Claude Haiku 4.5 (fast, non-reasoning)

     ### Commands
     ```bash
     # Quick query
     btca ask -r <resource> -q "<question>"

     # Interactive TUI
     btca chat -r <resource>

     # List resources
     btca config resources list

     Available Resources

     See references/btca-resources.md for the full resource table.

     Quick reference:
     - tauri — Command handlers, event emission
     - ffmpeg-next — Audio encoding, decoding, resampling
     - mp4ameta — M4B/MP4 metadata, Tag API
     - tokio — spawn_blocking, task cancellation
     - serde — Derive macros, field attributes
     - vitest — Mocking, async tests
     - typescript — Type inference, utility types

     Examples

     Verify Tauri Event Emission Pattern

     # Context7 for API
     mcp__context7-mcp__resolve-library-id("tauri")
     mcp__context7-mcp__query-docs("/tauri-apps/tauri", "emit events to frontend")

     # btca for implementation
     btca ask -r tauri -q "How do commands emit events to the frontend window?"

     Understand Serde Field Attributes

     btca ask -r serde -q "How do I skip serializing a field?"

     Check Tokio spawn_blocking Behavior

     btca ask -r tokio -q "How does spawn_blocking handle thread pool exhaustion?"

     ### Support File: references/btca-resources.md

     ```markdown
     # btca Resources

     Configured resources for this project. See `btca.config.jsonc` for full configuration.

     | Resource | Repository | Use Case |
     |----------|------------|----------|
     | `tauri` | tauri-apps/tauri (dev branch) | Command handlers, event emission, window management |
     | `ffmpeg-next` | zmwangx/rust-ffmpeg | Audio encoding, decoding, resampling, frame/packet handling |
     | `mp4ameta` | Saecki/rust-mp4ameta | M4B/MP4 metadata, Tag API, FreeformIdent atoms |
     | `tokio` | tokio-rs/tokio | spawn_blocking, task cancellation, Semaphore patterns |
     | `vitest` | vitest-dev/vitest | Mocking (vi.fn), async tests, JSDOM integration |
     | `serde` | serde-rs/serde | Derive macros, field attributes, enum serialization |
     | `typescript` | microsoft/TypeScript | Type inference, utility types, strict mode behavior |

     ## Adding New Resources

     ```bash
     btca config resources add -n <name> -t git -u <repo-url> -b <branch>

     Then update this file and btca.config.jsonc.

     ---

     ## Phase 2: Create `src-tauri/AGENTS.md`

     Move Rust-specific content from root AGENTS.md:

     ### Content to Move

     | Section | Current Lines | Notes |
     |---------|---------------|-------|
     | Architecture Fundamentals | 101-110 | Single engine, JobRegistry, path security, progress |
     | Critical Flows | 112-115 | Import, processing pipelines |
     | Integration Touchpoints | 117-121 | Commands, processor selection, progress reporter |
     | Architectural Invariants | 123-128 | Encoder, logic location, sanitization, target |
     | Interface Boundaries | 130-134 | Command surface, contract guard |
     | Code Guidelines: Rust | 200-207 | Clippy, Result patterns, pub visibility, errors |
     | Testing: Rust | 289-295 | External testing rule, test locations |

     ### New Structure

     ```markdown
     # Rust Backend Guidelines

     Inherits principles from root AGENTS.md.

     ## Architecture Fundamentals
     [moved content]

     ## Critical Flows
     [moved content]

     ## Integration Touchpoints
     [moved content]

     ## Architectural Invariants
     [moved content]

     ## Interface Boundaries
     [moved content]

     ## Code Conventions
     [moved content]

     ## Testing
     [moved content]

     ---
     Phase 3: Create src/AGENTS.md

     Move TypeScript/UI-specific content from root AGENTS.md:

     Content to Move
     ┌─────────────────────────────┬───────────────┬────────────────────────────────────────────────┐
     │           Section           │ Current Lines │                     Notes                      │
     ├─────────────────────────────┼───────────────┼────────────────────────────────────────────────┤
     │ Code Guidelines: TypeScript │ 193-198       │ Strict mode, naming, class patterns            │
     ├─────────────────────────────┼───────────────┼────────────────────────────────────────────────┤
     │ Frontend Testability        │ 229-233       │ Unique IDs, semantic HTML, agent-ready         │
     ├─────────────────────────────┼───────────────┼────────────────────────────────────────────────┤
     │ UI Layout and Spacing       │ 235-267       │ Spacing tokens, layout patterns, anti-patterns │
     ├─────────────────────────────┼───────────────┼────────────────────────────────────────────────┤
     │ Testing: TypeScript         │ 297-301       │ Colocated testing rule, locations              │
     └─────────────────────────────┴───────────────┴────────────────────────────────────────────────┘
     New Structure

     # TypeScript Frontend Guidelines

     Inherits principles from root AGENTS.md.

     ## Code Conventions
     [moved content]

     ## Frontend Testability
     [moved content]

     ## UI Layout and Spacing
     [moved content]

     ## Testing
     [moved content]

     ---
     Phase 4: Trim Root AGENTS.md

     Content to Keep (~135 lines)
     ┌─────────────────────────┬─────────┬───────────────────────────────────────────┐
     │         Section         │  Lines  │                  Action                   │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Agent Role and Purpose  │ 1-23    │ Keep                                      │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Engineering Principles  │ 24-36   │ Keep                                      │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Workflow Dynamics       │ 39-46   │ Keep                                      │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Checks & Gates          │ 48-65   │ Keep (references both Rust and TS)        │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Version & Changelog     │ 67-91   │ Keep                                      │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Essential Reading       │ 94-99   │ Update to include new files               │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Code Style & Guidelines │ 209-227 │ Keep (general LOC limits, exceptions)     │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Security & Validation   │ 271-279 │ Keep (cross-cutting)                      │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Testing & Verification  │ 283-307 │ Trim to strategy only; details in subdirs │
     ├─────────────────────────┼─────────┼───────────────────────────────────────────┤
     │ Build & Run Commands    │ 310-320 │ Keep                                      │
     └─────────────────────────┴─────────┴───────────────────────────────────────────┘
     Content to Remove
     ┌───────────────────────────────────┬─────────┬───────────────────────┐
     │              Section              │  Lines  │      Destination      │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ Tools & Workflow (Context7, btca) │ 138-184 │ → lib-research skill  │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ Architecture sections             │ 101-134 │ → src-tauri/AGENTS.md │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ Rust Code Guidelines              │ 200-207 │ → src-tauri/AGENTS.md │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ TypeScript Code Guidelines        │ 193-198 │ → src/AGENTS.md       │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ Frontend Testability              │ 229-233 │ → src/AGENTS.md       │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ UI Layout and Spacing             │ 235-267 │ → src/AGENTS.md       │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ Rust Testing                      │ 289-295 │ → src-tauri/AGENTS.md │
     ├───────────────────────────────────┼─────────┼───────────────────────┤
     │ TypeScript Testing                │ 297-301 │ → src/AGENTS.md       │
     └───────────────────────────────────┴─────────┴───────────────────────┘
     Updates to Essential Reading

     ## Essential Reading (in order)

     1. `AGENTS.md` (this file) — principles, workflow, shared conventions
     2. `src-tauri/AGENTS.md` — Rust architecture and backend patterns
     3. `src/AGENTS.md` — TypeScript/UI patterns and spacing
     4. `README.md` — human-facing overview
     5. `docs/external-apis/*.md` — ffmpeg-next, tauri, path handling

     **For external library research**: Load the `lib-research` skill.

     ---
     Phase 5: Update Cross-References

     In Root AGENTS.md

     - Remove Tools & Workflow section entirely (moved to skill)
     - Add note in "When working on the following areas" to include lib-research skill

     In src-tauri/AGENTS.md

     - Add header: "Inherits principles from root AGENTS.md"
     - Reference docs/external-apis/*.md for detailed API patterns

     In src/AGENTS.md

     - Add header: "Inherits principles from root AGENTS.md"
     - Reference root for shared conventions

     ---
     Verification

     After implementation:

     1. Skill loads correctly:
     # Check skill is registered (in Claude Code)
     /lib-research
     2. btca still works:
     btca ask -r serde -q "What is the skip attribute?"
     3. Context7 still accessible:
       - Verify MCP tools still available in Claude Code session
     4. AGENTS.md files parse correctly:
       - Open each file, verify markdown renders
       - Check no broken internal links
     5. No orphaned content:
       - Verify all moved content appears in new location
       - Verify no duplicate content

     ---
     Rollback

     If issues arise:
     - All changes are additive (new files) except root AGENTS.md
     - Git history preserves original AGENTS.md
     - Skill can be deleted by removing .codex/skills/lib-research/

     ---
     Summary
     ┌────────────────────────────┬────────────┬────────────────────┐
     │           Metric           │   Before   │       After        │
     ├────────────────────────────┼────────────┼────────────────────┤
     │ Root AGENTS.md             │ ~320 lines │ ~135 lines         │
     ├────────────────────────────┼────────────┼────────────────────┤
     │ Subdirectory AGENTS.md     │ 0          │ 2 files            │
     ├────────────────────────────┼────────────┼────────────────────┤
     │ Skills                     │ 8          │ 9 (+lib-research)  │
     ├────────────────────────────┼────────────┼────────────────────┤
     │ Tools section in AGENTS.md │ 50 lines   │ 0 (moved to skill) │
     └────────────────────────────┴────────────┴────────────────────┘

⏺ Plan approved. Let me set up the todo list and begin execution.

⏺ Bash(mkdir -p .codex/skills/lib-research/references)
  ⎿  (No content)

⏺ Write(.codex/skills/lib-research/SKILL.md)
  ⎿  Wrote 106 lines to .codex/skills/lib-research/SKILL.md                                                                                                         
      1 ---
      2 name: lib-research
      3 description: External library documentation and source code research. Use when verifying API contracts, understanding library internals, or researching how 
        a dependency implements something. Triggers: Context7, btca, external docs, library source, API patterns, "how does X work".
      4 ---
      5 # Library Research
      6 Tools for researching external library documentation and source code.
      7 ## When to Use Which Tool
     … +96 lines (ctrl+o to expand)

⏺ Write(.codex/skills/lib-research/references/btca-resources.md)
  ⎿  Wrote 22 lines to .codex/skills/lib-research/references/btca-resources.md                               
      1 # btca Resources
      2 Configured resources for this project. See `btca.config.jsonc` for full configuration.
      3 | Resource | Repository | Use Case |
      4 |----------|------------|----------|
      5 | `tauri` | tauri-apps/tauri (dev branch) | Command handlers, event emission, window management |
      6 | `ffmpeg-next` | zmwangx/rust-ffmpeg | Audio encoding, decoding, resampling, frame/packet handling |
      7 | `mp4ameta` | Saecki/rust-mp4ameta | M4B/MP4 metadata, Tag API, FreeformIdent atoms |
      8 | `tokio` | tokio-rs/tokio | spawn_blocking, task cancellation, Semaphore patterns |
     … +12 lines (ctrl+o to expand)

⏺ Now let me read the current AGENTS.md to extract the exact content for the subdirectory files.

⏺ Read(AGENTS.md)
  ⎿  Read 314 lines
  ⎿  You've hit your limit · resets 4pm (America/Los_Angeles)
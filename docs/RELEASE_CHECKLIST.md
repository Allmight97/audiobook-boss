# Audiobook Boss - Release Checklist

**Goal**: Public launch, Q1 2026
**Distribution**: Direct download (GitHub Releases) + Homebrew cask
**Notarization**: Deferred (users right-click > Open)
**Focus**: Ship a working product, not a perfect one
**Execution mode**: Full-stack migration complete (Svelte + Tailwind, Feb 2026). This checklist is now the primary release lane.

---

## Track A: Business/Legal (parallel track, not blocking release)

### LLC Formation
> General guidance - consult accountant/attorney for your situation.

- [ ] Choose state (usually your home state)
- [ ] Check name availability in state's business registry
- [ ] File Articles of Organization (~$50-500, online via Secretary of State)
- [ ] Get EIN from IRS (free, 5 min at irs.gov)
- [ ] Create Operating Agreement (even single-member LLCs need one)
- [ ] Open business bank account
- [ ] Set up basic accounting (Wave, QuickBooks, or spreadsheet)
- [ ] Understand tax implications (single-member LLC = pass-through by default)

---

## Track B: Technical - Must-Fix Issues

**Process for each issue:**
1. Read the issue, verify it's still valid
2. If stale/already fixed → close it, move on
3. If valid → fix it, test it, close it

### Security (must-fix)
- [x] **#144** - Symlink extension bypass in path validation (PR #214)
- [x] **#146** - Sanitize filesystem paths in user-facing error messages (PR #214)

### Core Functionality (must-fix)
- [x] **#52** - Audio quality distortion — safety warning added, defaults to auto encoder (PR #214). Deep codec research deferred (native AAC still lower quality than FDK/Apple).

### UX Clarity (must-fix)
- [x] **#141** - Metadata UX: auto-preserve on file switch, single Save All action, progress + status feedback (PR #214)
- [x] **#107** - Batch queue visibility + per-file status (closed 2026-01-23)

### QA Verification (must-fix)
- [x] **#136** - Verify bulk metadata save and multi-select UX (PR #214)

---

## Track C: Release Process

### Pre-Release
- [ ] All must-fix issues resolved
- [ ] Run release checks: `scripts/checks.sh release` (runs Standard tier + `cargo build --release -p audiobook-boss`)
- [ ] Manual smoke test (create M4B, edit metadata, batch process)
- [ ] Optional perf suite (non-blocking): run `bun run perf` (or `bun run perf:all` for synthetic + real) and review `scripts/perf/results/latest.md` for trend deltas

### Release
- [ ] Update CHANGELOG.md with user-facing changes
- [ ] Run `scripts/release.sh` (bumps version, builds, commits, tags)
- [ ] Push tag: `git push origin main --tags`
- [ ] Create GitHub Release with DMG artifact

### Homebrew (after GitHub Release)
- [ ] Create Homebrew cask formula
- [ ] Submit PR to homebrew-cask repo
- [ ] Wait for merge

---

## Track D: Launch Prep

### Documentation Updates
- [ ] Update README.md for public audience (installation, usage)
- [ ] Add CONTRIBUTING.md if accepting contributions (optional)

### Announcement
- [ ] Prepare launch materials (screenshots, description)
- [ ] Identify announcement venues:
  - [ ] r/audiobooks
  - [ ] r/selfhosted (if applicable)
  - [ ] Audiobookshelf community
  - [ ] Personal social media
- [ ] Draft announcement post

---

## Changelog & Release Notes

**Keep it manual** - automation adds complexity you don't need.

**Workflow** (already in place):
1. As you work, note user-facing changes
2. Run `scripts/release.sh`
3. Script prompts you to update CHANGELOG.md first
4. Write from user perspective ("Add X" not "Refactor Y")
5. Script bumps version, builds, commits, tags

**Format**:
```markdown
## [Unreleased]
### Added
- New feature

### Changed
- Modified behavior

### Fixed
- Bug fix
```

---

## Deferred / Parking Lot (not release-blocking)

These are valid but not blocking release. Migration is complete; these are post-launch improvements.
- Performance optimization (#180, #145)
- Tech debt refactoring (#147, #78, #108)
- Save/Revert/Clear UX (#115)
- Window persistence (#117)
- Output naming options (#140)
- All other open issues

---

## What Success Looks Like

- [ ] App downloadable from GitHub Releases
- [ ] README explains how to install and use it
- [ ] No known security issues
- [ ] Audio output sounds good
- [ ] Users can see what's happening (batch status, metadata feedback)
- [ ] LLC paperwork filed (can be in progress)
- [ ] At least one public announcement made

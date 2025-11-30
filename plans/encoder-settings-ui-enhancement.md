# Encoder Settings UI Enhancement Plan

**Created:** 2025-11-29
**Status:** Aligned - Ready for Implementation
**Branch:** Feature branch → PR to `new_encoder`

---

## Core Principle

> Maximize audiobook sound quality in the smallest file size possible, while preserving source characteristics.

---

## Scope

### In Scope
- Pill-style inline UI layout for encoder settings
- Smart defaults inferred from shrink.sh
- Encoder-specific toggles with concise descriptions
- Estimated bitrate display for all modes
- Encoder detection visibility ("FDK detected ✓")
- Profile as display-only (shows what encoder uses)
- PR workflow for review

### Out of Scope
- Preset buttons (just good defaults + user control)
- User-selectable profile (display-only for now)
- `fdk_source` path detection
- Forced sample rate or channel changes

---

## Defaults (from shrink.sh)

| Setting | FDK AAC | Apple AAC | Native AAC |
|---------|---------|-----------|------------|
| Profile (display) | HE-AAC v1 | AAC-LC | AAC-LC |
| Bitrate Mode | VBR | CVBR | CBR |
| Quality/Target | VBR 3 | 64 kbps | 96 kbps |
| **Estimated** | ~60 kbps | ~64 kbps | 96 kbps |
| Channels | Auto (preserve) | Auto | Auto |
| Sample Rate | Auto (preserve) | Auto | Auto |

### Encoder-Specific Toggles

**FDK AAC:**
```
☑ Afterburner    Better quality, +10% encode time
```

**Native AAC:**
```
☑ Twoloop    Improves quality at low bitrates
```

**Apple AAC:**
```
(No configurable options - hardware encoder)
```

---

## UI Layout (Pill-Style)

```
┌─────────────────────────────────────────────────────────────────────┐
│ Audio Encoder Settings                              Estimated: ~60 MB│
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Encoder              Profile          Bitrate Mode    Quality      │
│ ┌───────────────┐   ┌───────────┐    ┌───────────┐  ┌───────────┐  │
│ │ Auto (FDK)  ▼ │   │ HE-AAC v1 │    │ VBR     ▼ │  │ 3       ▼ │  │
│ └───────────────┘   └───────────┘    └───────────┘  └───────────┘  │
│  ✓ FDK detected      (display only)                  Est: ~60 kbps │
│                                                                     │
│  Sample Rate          Channels                                      │
│ ┌───────────────┐   ┌───────────────┐                              │
│ │ Auto        ▼ │   │ Auto        ▼ │                              │
│ └───────────────┘   └───────────────┘                              │
│                                                                     │
│ ── Encoder Options ─────────────────────────────────────────────── │
│  ☑ Afterburner    Better quality, +10% encode time                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Dynamic Behavior

1. **Encoder dropdown** - Shows availability status
   - "Auto (FDK)" when FDK available
   - "Auto (Apple)" when only Apple available
   - Unavailable encoders shown but disabled

2. **Profile display** - Read-only, updates based on encoder
   - FDK → "HE-AAC v1"
   - Apple → "AAC-LC"
   - Native → "AAC-LC"

3. **Bitrate Mode dropdown** - Locked per encoder
   - FDK → VBR only
   - Apple → CVBR only
   - Native → CBR only

4. **Quality/Bitrate control** - Changes based on mode
   - VBR: Quality dropdown (1-5)
   - CVBR/CBR: Bitrate dropdown (48k-128k)

5. **Estimated bitrate** - Always shown
   - VBR 1: ~32 kbps
   - VBR 2: ~48 kbps
   - VBR 3: ~60 kbps
   - VBR 4: ~72 kbps
   - VBR 5: ~96 kbps
   - CVBR/CBR: Shows target value

6. **Encoder Options section** - Shows/hides based on encoder
   - FDK: Afterburner toggle
   - Native: Twoloop toggle (or just indicator since always ON)
   - Apple: Section hidden

---

## Implementation

### Files to Modify

```
src/
├── ui/
│   └── encoderPanel/
│       ├── logic.ts          # Update defaults, add toggle logic
│       └── dom.ts            # Add new element references
├── types/
│   └── encoder.ts            # Update default values
└── index.html                # New pill-style layout, toggles
```

### Changes

#### 1. Fix Defaults (`src/types/encoder.ts`)
```typescript
// Change flavor default to 'auto'
flavor: 'auto',

// Enable VBR with quality 3 for FDK
vbr: { enabled: true, level: 3 },
```

#### 2. Update HTML Layout (`index.html`)
- Restructure encoder panel to pill-style inline layout
- Add profile display element (read-only)
- Add estimated bitrate display
- Add encoder options section with toggles
- Add `data-testid` attributes for all controls

#### 3. Update Logic (`src/ui/encoderPanel/logic.ts`)
- Apply encoder-specific defaults when encoder changes
- Update profile display based on encoder
- Show/hide encoder options based on selection
- Calculate and display estimated bitrate
- Simplify availability hint to single line

#### 4. Add Toggle Descriptions
Each encoder-specific toggle includes:
- Checkbox control
- Label
- Concise description (≤10 words)

---

## Acceptance Criteria

### Functional
- [ ] Auto encoder resolves: FDK → Apple → Native
- [ ] Defaults match shrink.sh exactly
- [ ] Changing encoder updates all related controls
- [ ] Profile displays correctly per encoder (read-only)
- [ ] Estimated bitrate shown for all modes
- [ ] Encoder-specific toggles appear/hide appropriately
- [ ] Toggle descriptions are concise and helpful
- [ ] FDK detection status visible

### Technical
- [ ] All controls have `data-testid` for testing
- [ ] Semantic HTML (proper form elements)
- [ ] `scripts/quick-checks.sh` passes
- [ ] PR created (not direct commit to `new_encoder`)

---

## Workflow

1. Create feature branch from `new_encoder`
2. Implement changes
3. Run `scripts/quick-checks.sh`
4. Open PR to `new_encoder` for review
5. Address feedback
6. Merge

---

## Future Considerations (Not This PR)

- User-selectable profile (if requested)
- Custom FDK path configuration
- Additional encoder-specific options
- Preset save/load functionality

# Encoder Implementation Plan (Updated)

**Project goal:** High‑quality, size‑efficient audiobook encoding to a single `.m4b` with legal distribution. Prioritize HE‑AAC at \~64–80 kbps when available; otherwise deliver solid AAC‑LC.

**Final product decisions (locked):**

- **Shipping model:** Bundle a clean **LGPL FFmpeg** (no `--enable-nonfree`, no `--enable-gpl`). *Optionally* use a **user‑installed external FFmpeg** for FDK jobs when detected.
- **Default encoders:**
  - **macOS:** `aac_at` (AudioToolbox). Use **HE‑AAC v1** for mono audiobooks at 64–80 kbps.
  - **Windows/Linux:** native `aac` (LC only).
- **FDK usage:** Supported **only via user’s external FFmpeg** (detected at runtime). Our bundled libs remain redistributable. App exposes a toggle to opt‑in when present.
- **No threads UI** for audio. (Remove from contracts/specs.)

---

## 1. Capability Matrix (Cheat Sheet)

| Encoder                  | Profiles                   | HE‑AAC v1            | HE‑AAC v2                     | VBR                              | Afterburner       | Notes for mono audiobooks                                                                                                                 |
| ------------------------ | -------------------------- | -------------------- | ----------------------------- | -------------------------------- | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **Apple **``** (macOS)** | LC, HE(v1), HE(v2), LD/ELD | ✅ (`profile=4`)      | ✅ (`profile=28`, stereo only) | Quality scale **0–14** (0=best)  | ❌                 | Use **HE v1** at **64–80 kbps**; keep mono (HEv2 unnecessary for mono).                                                                   |
| **Fraunhofer **``        | LC, HE(v1), HE(v2), LD/ELD | ✅ (`profile=aac_he`) | ✅ (`aac_he_v2`, stereo only)  | **VBR 1–5** (5=best)             | ✅ (default **1**) | Best perceived quality at 64–80 kbps; **keep Afterburner on**.                                                                            |
| **Native **``            | LC (incl. MPEG‑2 LC, LTP)  | ❌                    | ❌                             | **VBR via **``**/**`` (optional) | ❌                 | Prefer **80 kbps mono @ 44.1 kHz**; at **64 kbps** enable **“Optimize LC ≤64k”** → **32 kHz** to reduce artifacts; leave **cutoff auto**. |

**General:** HE‑AAC v2 implies **stereo** (Parametric Stereo). For mono speech, **HE‑AAC v1** is the right pick.

---

## 2. UX / UI Spec

**Top‑level controls**

- **Encoder** (radio): `Auto (recommended)` | `Apple AAC (macOS)` | `Use external FFmpeg (FDK) if available` | `Native AAC`
- **Bitrate** (dropdown): 56, 64, 72, 80, 88, 96 kbps (default **64 kbps**)
- **Channels**: 1 (Mono, default) | 2 (Stereo)
- **Profile** (contextual):
  - ``** or FDK**: `LC | HE (v1) | HE (v2)`
  - **Native**: hidden (LC only)
- **Advanced (accordion)**
  - **FDK only:** `Afterburner` (toggle, **On** default) ; **VBR** toggle + level **1–5** (disabled by default)
  - **aac\_at only:** `VBR` toggle + **quality 0–14** (0=best), default **off**
  - **Native only:** `Optimize LC at ≤64 kbps (32 kHz)` toggle (default **On** when `bitrate <= 64`), small inline note under Bitrate when it triggers

**Inline logic / validation**

- If **Profile = HE v2** then force **Channels=2** and show note: “HE‑AAC v2 requires stereo; for mono use HE v1.”
- If **Native** selected, hide Profile.
- If **External FFmpeg (FDK)** selected but not detected, show “FDK not detected” status with link: **How to install FFmpeg+FDK**.
- For **Native + ≤64 kbps**, show an info chip: “Optimizing LC at low bitrate → 32 kHz. Toggle in Advanced.”

**Tooltips (exact copy)**

- **Afterburner (FDK only):** “Fraunhofer’s post‑process polish. On = slightly better quality at low bitrates; CPU ↑. No size change in CBR.”
- **VBR (Apple AAC):** “Variable bitrate. **0 = highest quality**, 14 = lowest. Size varies; quality ↑ at lower numbers.”
- **VBR (FDK):** “Variable bitrate **1 (lowest) → 5 (highest)**. Size varies; quality ↑ at higher levels.”
- **Optimize LC ≤64k (Native):** “Use **32 kHz** at very low bitrates to reduce artifacts. Turn off to keep source sample rate.”

---

## 3. Runtime Selection & Detection

**Encoder selection order**

- **Auto (macOS):** prefer `aac_at`; if user selects **External FFmpeg (FDK)** and detected, use FDK; else Native.
- **Auto (Windows/Linux):** Native; if user selects External FDK and detected, use FDK.

**FDK detection (external FFmpeg path)**

- Discovery: search `PATH` for `ffmpeg` (and allow manual path override). Run `ffmpeg -hide_banner -v 0 -encoders` and look for `libfdk_aac` in the encoder list.
- Store detection result and full path in settings; re‑probe on path change or version change.
- For jobs routed to “external FFmpeg”, shell out with a **sanitized** command line; capture stderr to surface progress/errors.

---

## 4. API / Types

**TypeScript (payload → backend)**

```ts
export type EncoderFlavor = 'auto' | 'aac_at' | 'external_fdk' | 'native_aac';
export type AacProfile = 'lc' | 'he' | 'he_v2';

export interface EncoderSettingsV2 {
  flavor: EncoderFlavor;
  bitrateKbps: 56|64|72|80|88|96;
  channels: 1|2;
  profile?: AacProfile;           // hidden/ignored for native
  vbr?: {                          // optional, per-encoder meaning
    enabled: boolean;
    level?: number;               // FDK: 1–5; aac_at: 0–14 (0 best)
  };
  fdkAfterburner?: boolean;       // FDK only
  optimizeLcLowBitrate?: boolean; // Native only; suggests 32 kHz at ≤64 kbps
  externalFfmpegPath?: string;    // optional absolute path override
}
```

**Rust (serde mirrors)**

- `enum EncoderFlavor { Auto, AacAt, ExternalFdk, NativeAac }`
- `enum AacProfile { Lc, He, HeV2 }`
- `struct EncoderSettingsV2 { … }` with validation:
  - If `flavor == NativeAac` ⇒ `profile = None` (force LC).
  - If `profile == HeV2` ⇒ `channels == 2` else validation error.
  - If `flavor != ExternalFdk` ⇒ ignore `fdkAfterburner` and any FDK‑VBR.
  - If `flavor != AacAt` ⇒ ignore `aac_at` VBR quality (i.e., `vbr.level` 0–14).

---

## 5. Backend Mapping (ffmpeg‑next in‑process; plus external CLI path)

**Common pre‑encode**

- Ensure **downmix to mono** when `channels==1` in the resampler stage.
- If **Native + optimize at ≤64 kbps** is active and bitrate ≤64 ⇒ set output `sample_rate=32000`; otherwise pass through source rate.

**In‑process encoders**

- `` (macOS only):

  - Profiles: `he v1 → profile=4`, `he v2 → profile=28` (stereo only). `lc` ⇒ omit `profile`.
  - **CBR:** set `b` to `bitrateKbps*1000`.
  - **VBR (Advanced):** set mode to VBR and map `vbr.level` 0–14 (0 best) to `global_quality` as per AudioToolbox mapping.

- **Native **``:

  - **LC only.** Set `b` to CBR. Leave `cutoff` **unset** (auto). Keep default coder **twoloop**.
  - Optional Native VBR (future): set `global_quality` and clear `b` when `vbr.enabled`.

**External FFmpeg (FDK) path**

- Build command line per job when `flavor == ExternalFdk` **and** detection succeeded:
  - `-c:a libfdk_aac` + `-profile:a` (`aac_he` or `aac_he_v2` when selected; else LC)
  - `-b:a <kbps>k` for CBR **or** `-vbr <1..5>` when VBR enabled
  - `-afterburner <0|1>`
  - channel/layout and sample rate flags from settings
- On failure (binary missing or `libfdk_aac` absent), fall back to `aac_at` (macOS) or Native and surface a non‑blocking warning to the user.

**INFO logging (one line per job)**

```
encoder=<aac_at|libfdk_aac|aac(native)|external-missing> profile=<lc|he|he_v2|none> \
bitrate=<kbps> ch=<1|2> sr=<Hz> vbr=<off|level> afterburner=<0|1> notes=[ignored:…]
```

---

## 6. Tests

**Unit**

- HE v2 + Mono ⇒ validation error.
- Native + profile specified ⇒ coerced to LC; log `ignored: profile`.
- External FDK selected + detection false ⇒ fallback encoder chosen per platform with warning message queued.
- Native + ≤64 kbps + optimize enabled ⇒ `sample_rate=32000`.

**Integration**

- macOS runner: Auto ⇒ selects `aac_at`; HE v1 mono at 64 kbps produces expected profile bits.
- External FFmpeg detector: parses `-encoders` output containing/omitting `libfdk_aac`.

**Goldens / Snapshots**

- CLI string builder for external FDK matches expected tokens.
- INFO summary log matches schema.

---

## 7. Docs & Help

- **In‑app help card:** “Why you might see HE options on macOS but not Windows/Linux.”
- **How to install FFmpeg+FDK** article link (platform‑specific). Include detection steps and a “Verify” button that runs the probe.

---

## 8. Roadmap / Future

- Add **quality presets** (Max Savings / Balanced / Max Quality) once advanced controls are stable.
- Consider **native LC VBR** option behind Advanced for users who prefer quality over size predictability.
- Optional: per‑chapter parallel encode scheduler.

---

# Implementation Tasks (atomized; junior‑friendly)

### Phase A — Types, Validation, Detection

1. **Types:** Add `EncoderFlavor`, `AacProfile`, `EncoderSettingsV2` in frontend & backend (serde).
2. **Validation:** Implement backend validators for profile↔channels and per‑flavor ignored fields.
3. **External FFmpeg detector:**
   - Implement `which ffmpeg` path discovery + manual override.
   - Spawn `ffmpeg -hide_banner -v 0 -encoders` and set `has_fdk = output.contains("libfdk_aac")`.
   - Persist `{ path, has_fdk, lastCheckedVersion }`.

### Phase B — UI

4. Build **Encoder** panel with radio buttons and Advanced accordion.
5. Contextual show/hide for **Profile** and **Advanced** options per flavor.
6. Inline notes: HE v2 forces stereo; Native low‑bitrate optimization notice.

### Phase C — Backend mapping

7. **In‑process:** Implement `aac_at` & Native mapping as specified; ensure mono downmix and sample‑rate logic.
8. **External FDK job:** Build CLI, execute, and map progress/errors to UI.
9. Add INFO **summary log** per job; include ignored options.

### Phase D — Tests

10. Write unit tests for validators & detector parser.
11. Integration test on macOS for Auto ⇒ `aac_at` path.
12. Snapshot test for external FDK CLI builder.

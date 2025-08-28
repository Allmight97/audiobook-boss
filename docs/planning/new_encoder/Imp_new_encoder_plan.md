# Advanced Encoder Options – Unified Source of Truth (AAC Integration)

> This document **replaces** `initial_design.md` and `active_encoder_plan.md`. It consolidates high‑level decisions, user‑experience requirements, and detailed technical steps for adding advanced AAC encoder options to **audiobook‑boss**. Use this as the authoritative reference for development, testing, and documentation going forward.

---

## 1. Goals and guiding decisions

* **Quality vs legality.** Deliver high‑quality, size‑efficient audiobooks while remaining within redistributable licenses. The app will **bundle a clean LGPL FFmpeg** (no `--enable-gpl` or `--enable-nonfree`).

  * Background: FDK AAC requires **non‑free** licensing; it will **only** be supported via a **user‑provided external FFmpeg** binary. See FFmpeg’s docs for `libfdk_aac`.
* **Default encoders.** On **macOS**, prefer the Apple AAC encoder (`aac_at`) via **AudioToolbox**; fall back to the **native AAC** encoder if unavailable. On **Windows/Linux**, default to the **native AAC** encoder.
* **User choice.** Provide an **Advanced** panel allowing users to select between **Auto**, **Apple AAC**, **Native AAC**, and **External FFmpeg (FDK)** encoders. Expose related settings **contextually** (profiles, bitrates, platform‑specific notes). VBR and FDK are placeholders only (disabled in this phase). Threads are out of scope.
* **Incremental rollout.** Integrate **Apple AAC** support first (macOS), verify stability, then add **FDK AAC** detection and invocation. Each phase should be a **separate pull request**.

---

## 2. Capability matrix

Use this table to inform **validation logic** and **UI presentation**.

| Encoder                           | Available profiles           | HE‑AAC v1            | HE‑AAC v2                    | VBR                               | Afterburner   | Notes                                                                                           |
| --------------------------------- | ---------------------------- | -------------------- | ---------------------------- | --------------------------------- | ------------- | ----------------------------------------------------------------------------------------------- |
| **Apple AAC (`aac_at`)**          | LC, HE(v1), HE(v2), LD/ELD   | ✅ (profile=4)        | ✅ (profile=28, stereo only)  | Quality scale **0–14** (0 = best) | ❌             | macOS only; prefer **HE v1** at **64–80 kbps** for mono audiobooks                              |
| **Fraunhofer FDK (`libfdk_aac`)** | LC, HE(v1), HE(v2), LD/ELD   | ✅ (`profile=aac_he`) | ✅ (`aac_he_v2`, stereo only) | Levels **1–5** (5 = best)         | ✅ (default 1) | Highest quality at low bitrates; **only via external FFmpeg**                                   |
| **Native AAC**                    | LC (MPEG‑4 & MPEG‑2 LC, LTP) | ❌                    | ❌                            | `-q:a` (optional)                 | ❌             | Prefer **80 kbps mono @ 44.1 kHz**. At **≤ 64 kbps**, default to **32 kHz** to reduce artifacts |

**General rule:** HE‑AAC v2 **requires stereo**; for mono content, select **HE‑AAC v1**.

---

## 3. UI and UX specification

The **Advanced Encoder** panel should offer the following controls:

* **Encoder selection (radio buttons):** `Auto (recommended)` | `Apple AAC (macOS)` | `Use external FFmpeg (FDK)` | `Native AAC`.
* **Bitrate:** Dropdown with `64, 72, 80, 88, 96 kbps` (default **64 kbps**).
* **Channels:** `1 (Mono)` (default) or `2 (Stereo)`.
* **Profile selector:** Shown only for **Apple AAC** and **External FDK**; options `LC`, `HE (v1)`, `HE (v2)`. Hidden for **Native AAC**.
* **Advanced accordion:**

  * **Apple AAC:** VBR control reserved — currently disabled/hidden (backend ignores VBR).
  * **External FDK:** Placeholder only — controls disabled (Afterburner/VBR reserved for future work).
  * **Native AAC:** **Optimize LC ≤ 64 kbps** toggle (On when `bitrate ≤ 64`); shows an info chip about downsampling to **32 kHz**.

**Validation & notes:**

* If `profile = HE v2`, **force** `channels = 2` and show a helper note.
* If **External FDK** is selected but **not detected**, show **“FDK not detected”** with installation instructions.
* Use concise tooltips explaining **VBR** and **Afterburner** effects.

---

## 4. Data types & contracts

Adopt these **new types** and **payload structures**.

### TypeScript (frontend)

```ts
export type EncoderFlavor = 'auto' | 'aac_at' | 'external_fdk' | 'native_aac';
export type AacProfile = 'lc' | 'he' | 'he_v2';

export interface EncoderSettingsV2 {
  flavor: EncoderFlavor;
  bitrateKbps: 64|72|80|88|96;
  channels: 1|2;
  profile?: AacProfile;                 // hidden/ignored for native
  vbr?: { enabled: boolean; level?: number; };  // Reserved; disabled now; backend ignores
  fdkAfterburner?: boolean;             // Reserved; disabled now
  optimizeLcLowBitrate?: boolean;       // Native only; suggests 32 kHz at ≤ 64 kbps
  externalFfmpegPath?: string;          // Absolute path override (future FDK)
}
```

### Rust (backend mirrors)

```rust
enum EncoderFlavor { Auto, AacAt, ExternalFdk, NativeAac }

enum AacProfile { Lc, He, HeV2 }

struct VbrSetting {
    enabled: bool,
    level: Option<u8>, // Reserved; disabled now
}

struct EncoderSettingsV2 {
    flavor: EncoderFlavor,
    bitrate_kbps: u16,
    channels: u8,
    profile: Option<AacProfile>,
    vbr: Option<VbrSetting>, // Reserved; disabled now
    fdk_afterburner: bool,   // Reserved; disabled now
    optimize_lc_low_bitrate: bool,
    external_ffmpeg_path: Option<String>, // Future FDK only
}
```

### Validation rules

* If `flavor == NativeAac`, **force** `profile = None` and ignore VBR/afterburner.
* If `profile == HeV2`, **require** `channels == 2` (reject mono).
* Ignore FDK‑specific fields unless `flavor == ExternalFdk`.
* If **Apple AAC** is selected on a non‑macOS platform, **fall back** to Native AAC and notify the user.

---

## 5. Runtime selection & detection

* **Auto mode:** On macOS, prefer **`aac_at`** when available; otherwise **Native AAC**. On Windows/Linux, choose **Native AAC**. Auto mode never selects FDK.

* **FDK detection (future):** Search `PATH` for an `ffmpeg` binary or allow the user to browse to one. Execute `ffmpeg -hide_banner -v 0 -encoders` and parse output for `libfdk_aac`. Cache the detection result and display status; **re‑probe** when the binary path changes.

* **Secure invocation (external FFmpeg):** Treat the binary as **untrusted**.

  * Validate that the path refers to an **executable**; optionally inspect `ffmpeg -version`.
  * Build command arguments as an **array** (not a shell string) to avoid injection.
  * Execute with **minimal privileges** and a controlled working directory; capture and surface logs for progress/error handling.

* **Apple AAC integration:** Compile `ffmpeg-sys-next` with the **`build-audiotoolbox`** feature on macOS. When the `aac_at` encoder is selected, set the **profile** using `av_opt_set_int(ctx, "profile", ...)` (values: `0` for LC, `4` for HE‑v1, `28` for HE‑v2). Use CBR for this phase; VBR mapping is reserved for future work.

---

## 6. Phased implementation milestones (PR‑by‑PR)
- TOC: Mark each phase as complete as we go. User will create a new PR end of each phase.
  - [ ] Phase 1 – Types & Scaffolding
  - [ ] Phase 2 – Command upgrade & IPC plumbing
  - [ ] Phase 3 – Apple AAC integration
  - [ ] Phase 4 – External FDK support
  - [ ] Phase 5 – UI polish & documentation
  - [ ] Phase 6 – Testing

**Phase 1 – Types & Scaffolding**

* Introduce the new `EncoderFlavor`, `AacProfile`, and `EncoderSettingsV2` types in Rust and TypeScript.
* Add backend validation helpers (bitrate whitelist, HE‑v2 stereo enforcement, ignoring unsupported options).
* Expose a `validate_encoder_settings` Tauri command for the UI to pre‑check payloads.
* Implement fallback detection for `aac_at` using `ffmpeg-next` name lookup.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

**Phase 2 – Command upgrade & IPC plumbing**

* Add a new **v2 processing command** that accepts `EncoderSettingsV2` and routes into the existing pipeline; keep the v1 command intact initially.
* Update the frontend to assemble and send the v2 payload; maintain progress/cancellation listeners.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

**Phase 3 – Apple AAC integration**

* Compile `ffmpeg-sys-next` with `build-audiotoolbox` on macOS.
* When `flavor == AacAt`, set profile via libav options; **fallback to native** if `aac_at` is unavailable.
* UI: keep VBR controls disabled/hidden; FDK controls remain disabled placeholders.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

**Phase 4 – External FDK support (future)**

* Implement external FFmpeg detection logic and path storage. Provide a file picker in the UI.
* When `flavor == ExternalFdk` and FDK is detected, construct a command such as:

  ```bash
  ffmpeg -i input.wav -c:a libfdk_aac -profile:a aac_he -b:a 64k -afterburner 1 -y output.m4b
  ```

  Choose `aac_he`/`aac_he_v2` per user settings. VBR/afterburner can be added later. Ensure the command is built **securely**.

---

## 8. Future Work — Enablement Guide (VBR and FDK, disabled now)

Status: VBR and FDK are intentionally disabled in this phase. Placeholders exist in types and UI. Backend ignores VBR/FDK fields.

Search tokens to locate all relevant code paths quickly:

- FEATURE_TOGGLE:VBR, VBR_DISABLED_MARKER
- FEATURE_TOGGLE:FDK, FDK_PLACEHOLDER

File map:

- Frontend types: `src/types/encoder.ts`
- Frontend panel: `src/ui/encoderPanel/*` (feature flags, DOM, logic, state)
- Backend types/validation: `src-tauri/src/audio/settings_encoder.rs`
- Backend mapping: `src-tauri/src/audio/processor/encoder.rs`, `streams.rs`

Checklist — VBR (staged):

1) Frontend
   - Unhide VBR controls when `ENABLE_VBR === true`.
   - Publish VBR settings in `EncoderSettingsV2`.
2) Backend (aac_at first)
   - Map quality 0–14 to `global_quality` and enable VBR mode for `aac_at`.
   - For native AAC (optional later): set `global_quality` and clear `bit_rate`.
3) Validation & tests
   - Update validators to accept VBR inputs; extend unit tests.
4) Observability
   - Include `vbr=<off|level>` in encoder summary log.

Checklist — FDK (separate PR; off by default):

1) Detection
   - PATH search and manual path override; probe `-encoders` for `libfdk_aac`.
2) Secure invocation
   - Build argv array; sandbox working dir; capture logs; progress mapping.
3) UI gating
   - Enable FDK controls only when detected and explicitly selected.
4) Tests & docs
   - Integration tests for detection and CLI builder; update help docs.
* Integrate progress reporting and error handling for the external process.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

**Phase 5 – UI polish & documentation**

* Complete the Advanced Encoder panel with all controls, disabled states and tooltips.
* Ensure **HE‑AAC v2 forces stereo** and surfaces a helper message.
* Provide a help link explaining how to install FFmpeg with FDK support.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

**Phase 6 – Testing**

* Unit tests for validation logic (HE‑v2 stereo enforcement, default resolution of profiles, ignoring unsupported flags).
* Integration tests verifying that each encoder flavour triggers the expected behaviour (native AAC, `aac_at`, external FDK) and that logs contain correct summary lines.
* Update documentation (`external-apis/ffmpeg-next.md`, UI help text) accordingly.
* [ ] Collaborate with user on testing appropriate for the phase.
* [ ] (USER)When phase is complete, create a new PR for review and merge.

---

## 7. Open questions & risks

* **Afterburner availability:** Only applicable to FDK. Keep the option hidden/disabled until FDK is detected.
* **HE‑AAC profile flags:** Confirm integer constants for `FF_PROFILE_AAC_HE` and `FF_PROFILE_AAC_HE_V2` in FFmpeg headers.
* **Cross‑platform edges:** Ensure `aac_at` is never selected on non‑macOS platforms and that native AAC falls back gracefully.
* **Security:** Running a user‑supplied binary is inherently risky; consider additional sandboxing or code‑signing verification where feasible.

import type { AudiobookMetadata } from "../types/metadata";
import {
  getCurrentCoverArt,
  getHasCustomCoverArt,
  isCoverArtRemovalRequested,
  setCoverArt,
} from "./coverArt";

export type MetadataFormMode = "single" | "multi";

type FieldConfig = {
  inputId: string;
  actionId: string;
  key: keyof AudiobookMetadata;
  mapToAlbum?: boolean;
  isNumber?: boolean;
  unconditional?: boolean;
};

type FieldAction = "keep" | "blank";

const FIELD_CONFIGS: FieldConfig[] = [
  { inputId: "meta-title", actionId: "meta-title-action", key: "title", mapToAlbum: true },
  { inputId: "meta-author", actionId: "meta-author-action", key: "artist" },
  { inputId: "meta-narrator", actionId: "meta-narrator-action", key: "composer" },
  { inputId: "meta-year", actionId: "meta-year-action", key: "date", isNumber: true },
  { inputId: "meta-genre", actionId: "meta-genre-action", key: "genre" },
  { inputId: "meta-series", actionId: "meta-series-action", key: "series", unconditional: true },
  {
    inputId: "meta-series-part",
    actionId: "meta-series-part-action",
    key: "series_part",
    unconditional: true,
  },
  {
    inputId: "meta-description",
    actionId: "meta-description-action",
    key: "description",
    unconditional: true,
  },
];

const MIXED_PLACEHOLDER = "Mixed values";

function getMetadataForm(): HTMLElement | null {
  return document.getElementById("metadata-form");
}

function getSelectionCountEl(): HTMLElement | null {
  return document.getElementById("metadata-selection-count");
}

function getInputElement(
  id: string
): HTMLInputElement | HTMLTextAreaElement | null {
  const el = document.getElementById(id);
  if (!el) return null;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    return el;
  }
  return null;
}

function getActionElement(id: string): HTMLSelectElement | null {
  const el = document.getElementById(id);
  if (el instanceof HTMLSelectElement) {
    return el;
  }
  return null;
}

function cacheDefaultPlaceholder(input: HTMLInputElement | HTMLTextAreaElement): void {
  if (!input.dataset.defaultPlaceholder) {
    input.dataset.defaultPlaceholder = input.placeholder || "";
  }
}

function setMixedPlaceholder(
  input: HTMLInputElement | HTMLTextAreaElement,
  mixed: boolean
): void {
  cacheDefaultPlaceholder(input);
  if (mixed) {
    input.placeholder = MIXED_PLACEHOLDER;
    input.dataset.mixed = "true";
  } else {
    input.placeholder = input.dataset.defaultPlaceholder || "";
    delete input.dataset.mixed;
  }
}

function markDirty(input: HTMLInputElement | HTMLTextAreaElement): void {
  input.dataset.dirty = "true";
  input.classList.add("dirty-field");
}

function isDirty(input: HTMLInputElement | HTMLTextAreaElement): boolean {
  return input.dataset.dirty === "true";
}

function getFieldAction(actionId: string): FieldAction {
  const action = getActionElement(actionId);
  return (action?.value as FieldAction) || "keep";
}

function setFieldAction(actionId: string, value: FieldAction): void {
  const action = getActionElement(actionId);
  if (action) {
    action.value = value;
  }
}

function isMultiSelectMode(): boolean {
  const form = getMetadataForm();
  return form?.dataset.multiSelect === "true";
}

export function setMetadataFormMode(
  mode: MetadataFormMode,
  selectionCount?: number
): void {
  const form = getMetadataForm();
  if (form) {
    form.dataset.multiSelect = mode === "multi" ? "true" : "false";
  }

  const countEl = getSelectionCountEl();
  if (!countEl) return;

  if (mode === "multi" && selectionCount && selectionCount > 1) {
    countEl.textContent = `${selectionCount} files selected`;
    countEl.hidden = false;
  } else {
    countEl.textContent = "";
    countEl.hidden = true;
  }
}

export function initMetadataFormEvents(): void {
  const form = getMetadataForm();
  if (!form) return;

  const inputs = form.querySelectorAll<
    HTMLInputElement | HTMLTextAreaElement
  >("input[id^='meta-'], textarea[id^='meta-']");

  inputs.forEach((input) => {
    const handleInput = () => {
      markDirty(input);
      if (!isMultiSelectMode()) return;

      const config = FIELD_CONFIGS.find((field) => field.inputId === input.id);
      if (!config) return;

      const actionValue: FieldAction = input.value.trim() ? "keep" : "blank";
      setFieldAction(config.actionId, actionValue);
    };

    input.addEventListener("input", handleInput);
    input.addEventListener("change", handleInput);
  });

  const actionSelects = form.querySelectorAll<HTMLSelectElement>(
    ".meta-apply-select"
  );
  actionSelects.forEach((select) => {
    select.addEventListener("change", () => {
      const config = FIELD_CONFIGS.find((field) => field.actionId === select.id);
      if (!config) return;
      const input = getInputElement(config.inputId);
      if (!input) return;

      if (select.value === "blank") {
        input.value = "";
        markDirty(input);
      }
    });
  });
}

export function resetDirtyState(): void {
  const form = getMetadataForm();
  if (!form) return;

  const inputs = form.querySelectorAll<
    HTMLInputElement | HTMLTextAreaElement
  >("input[id^='meta-'], textarea[id^='meta-']");
  inputs.forEach((input) => {
    delete input.dataset.dirty;
    input.classList.remove("dirty-field");
  });

  const actionSelects = form.querySelectorAll<HTMLSelectElement>(
    ".meta-apply-select"
  );
  actionSelects.forEach((select) => {
    select.value = "keep";
  });
}

export function populateMetadataFormSingle(
  metadata: Partial<AudiobookMetadata>
): void {
  setMetadataFormMode("single");

  FIELD_CONFIGS.forEach((field) => {
    const input = getInputElement(field.inputId);
    if (!input) return;

    let value = "";
    if (field.isNumber) {
      const date = metadata.date;
      if (typeof date === "number" && date > 0) {
        value = date.toString();
      }
    } else {
      const raw = metadata[field.key];
      if (typeof raw === "string") {
        value = raw;
      }
    }

    input.value = value;
    setMixedPlaceholder(input, false);
  });

  if (!getHasCustomCoverArt()) {
    setCoverArt(metadata.cover_art || null);
  }

  resetDirtyState();
}

export function populateMetadataFormMulti(
  metadataList: Partial<AudiobookMetadata>[],
  selectionCount: number
): void {
  setMetadataFormMode("multi", selectionCount);

  const hasMetadata = metadataList.length > 0;

  FIELD_CONFIGS.forEach((field) => {
    const input = getInputElement(field.inputId);
    if (!input) return;

    if (!hasMetadata) {
      input.value = "";
      setMixedPlaceholder(input, false);
      return;
    }

    const values = metadataList.map((metadata) => {
      if (field.isNumber) {
        const date = metadata.date;
        return typeof date === "number" && date > 0 ? date.toString() : "";
      }
      const raw = metadata[field.key];
      return typeof raw === "string" ? raw.trim() : "";
    });

    const uniqueValues = new Set(values);
    if (uniqueValues.size === 1) {
      const value = values[0] ?? "";
      input.value = value;
      setMixedPlaceholder(input, false);
    } else {
      input.value = "";
      setMixedPlaceholder(input, true);
    }
  });

  if (!getHasCustomCoverArt()) {
    setCoverArt(null);
  }

  resetDirtyState();
}

export function readMetadataForm(options?: {
  mode?: MetadataFormMode;
  onlyDirty?: boolean;
}): Partial<AudiobookMetadata> {
  const mode = options?.mode ?? "single";
  const onlyDirty = options?.onlyDirty ?? false;
  const metadata: Partial<AudiobookMetadata> = {};
  const setMetadataValue = <K extends keyof AudiobookMetadata>(
    key: K,
    value: AudiobookMetadata[K]
  ): void => {
    metadata[key] = value;
  };

  FIELD_CONFIGS.forEach((field) => {
    const input = getInputElement(field.inputId);
    if (!input) return;

    const raw = input.value.trim();
    const dirty = isDirty(input);

    if (mode === "multi") {
      const action = getFieldAction(field.actionId);

      if (action === "blank") {
        if (field.isNumber) {
          setMetadataValue(
            field.key,
            0 as AudiobookMetadata[typeof field.key]
          );
        } else {
          setMetadataValue(
            field.key,
            "" as AudiobookMetadata[typeof field.key]
          );
          if (field.mapToAlbum && field.key === "title") {
            metadata.album = "";
          }
        }
        return;
      }

      if (!dirty) return;

      if (field.isNumber) {
        if (!raw) {
          setMetadataValue(
            field.key,
            0 as AudiobookMetadata[typeof field.key]
          );
          return;
        }
        const parsed = parseInt(raw, 10);
        if (!Number.isNaN(parsed)) {
          setMetadataValue(
            field.key,
            parsed as AudiobookMetadata[typeof field.key]
          );
        }
        return;
      }

      setMetadataValue(
        field.key,
        raw as AudiobookMetadata[typeof field.key]
      );
      if (field.mapToAlbum && field.key === "title") {
        metadata.album = raw;
      }
      return;
    }

    if (onlyDirty && !dirty) return;

    if (field.isNumber) {
      if (raw) {
        const parsed = parseInt(raw, 10);
        if (!Number.isNaN(parsed)) {
          setMetadataValue(
            field.key,
            parsed as AudiobookMetadata[typeof field.key]
          );
        }
      } else if (dirty) {
        setMetadataValue(
          field.key,
          0 as AudiobookMetadata[typeof field.key]
        );
      }
      return;
    }

    const shouldInclude =
      raw || dirty || (field.unconditional && !onlyDirty);

    if (!shouldInclude) return;

    setMetadataValue(
      field.key,
      raw as AudiobookMetadata[typeof field.key]
    );
    if (field.mapToAlbum && field.key === "title") {
      metadata.album = raw;
    }
  });

  if (mode === "single") {
    if (isCoverArtRemovalRequested()) {
      metadata.cover_art = [];
    } else {
      const coverBytes = getCurrentCoverArt();
      if (coverBytes && coverBytes.length > 0) {
        metadata.cover_art = coverBytes;
      }
    }
  }

  return metadata;
}

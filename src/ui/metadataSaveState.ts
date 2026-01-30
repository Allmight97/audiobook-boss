let metadataSaveInProgress = false;

export function setMetadataSaveInProgress(inProgress: boolean): void {
  metadataSaveInProgress = inProgress;

  const applyButton = document.getElementById(
    "metadata-apply-btn"
  ) as HTMLButtonElement | null;
  if (applyButton) {
    applyButton.disabled = inProgress;
  }

  const saveButton = document.getElementById(
    "metadata-save-btn"
  ) as HTMLButtonElement | null;
  if (saveButton) {
    saveButton.disabled = inProgress;
  }
}

export function isMetadataSaveInProgress(): boolean {
  return metadataSaveInProgress;
}

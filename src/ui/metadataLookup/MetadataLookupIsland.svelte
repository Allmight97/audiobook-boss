<div
  id="metadata-lookup-modal"
  class="metadata-lookup-modal"
  data-testid="metadata-lookup-modal"
  aria-hidden="true"
>
  <div
    class="metadata-lookup-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="metadata-lookup-title"
  >
    <div class="metadata-lookup-header">
      <h3 id="metadata-lookup-title">Find Metadata Online</h3>
      <button
        id="metadata-lookup-close"
        class="btn-pill btn-pill-secondary"
        data-testid="metadata-lookup-close"
        type="button"
      >
        Close
      </button>
    </div>
    <div class="metadata-lookup-body">
      <div class="metadata-lookup-controls">
        <div class="metadata-lookup-field">
          <label for="metadata-lookup-query">Search</label>
          <input
            id="metadata-lookup-query"
            type="text"
            placeholder="Title, author, or ASIN (e.g., B01234ABCD)"
            data-testid="metadata-lookup-query"
          />
        </div>
        <div class="metadata-lookup-field">
          <label for="metadata-lookup-source">Source</label>
          <select
            id="metadata-lookup-source"
            data-testid="metadata-lookup-source"
          >
            <option value="audnexus">Audnexus</option>
          </select>
        </div>
        <div class="metadata-lookup-field">
          <label for="metadata-lookup-apply-mode">Apply</label>
          <select
            id="metadata-lookup-apply-mode"
            data-testid="metadata-lookup-apply-mode"
          ></select>
        </div>
        <div class="metadata-lookup-field metadata-lookup-field-toggle">
          <label class="checkbox-label text-xs mb-0">
            <input
              type="checkbox"
              id="metadata-lookup-cover-toggle"
              data-testid="metadata-lookup-cover-toggle"
            />
            <span class="option-label">Replace cover art</span>
          </label>
        </div>
        <div class="metadata-lookup-field metadata-lookup-field-button">
          <button
            id="metadata-lookup-search-btn"
            class="btn-pill btn-pill-primary"
            data-testid="metadata-lookup-search-btn"
            type="button"
          >
            Search
          </button>
        </div>
        <div class="metadata-lookup-field metadata-lookup-field-button">
          <button
            id="metadata-lookup-skip-btn"
            class="btn-pill btn-pill-secondary"
            data-testid="metadata-lookup-skip-btn"
            type="button"
          >
            Skip
          </button>
        </div>
      </div>
      <div
        id="metadata-lookup-context"
        class="metadata-lookup-context text-xs muted-text"
      ></div>
      <div
        id="metadata-lookup-status"
        class="metadata-lookup-status text-xs"
      ></div>
      <div
        id="metadata-lookup-results"
        class="metadata-lookup-results"
      ></div>
    </div>
  </div>
</div>

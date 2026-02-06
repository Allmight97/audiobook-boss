const ALLOWED_REGIONS = new Set(["au", "ca", "de", "es", "fr", "in", "it", "jp", "us", "uk"]);

const SYNTHETIC_WORKLOAD = {
  queryCount: 200,
  loops: 100,
  itemCount: 120,
};

const REAL_WORKLOAD = {
  requests: [
    "https://api.audible.com/1.0/catalog/products?response_groups=contributors,product_desc,product_attrs,product_extended_attrs,media,product_details,series&products_sort_by=Relevance&num_results=3&image_sizes=500,1024&keywords=project%20hail%20mary",
    "https://api.audnex.us/books/B08G9PRS1K?region=us",
  ],
  timeoutMs: 12000,
};

function extractAsin(query) {
  let current = "";
  for (const ch of query) {
    if (/[a-z0-9]/i.test(ch)) {
      current += ch.toUpperCase();
    } else {
      if (current.length === 10) return current;
      current = "";
    }
  }
  return current.length === 10 ? current : null;
}

function extractRegionOverride(query) {
  for (let i = 0; i <= query.length - 4; i += 1) {
    if (query[i] !== "[" || query[i + 3] !== "]") continue;
    const region = query.slice(i + 1, i + 3).toLowerCase();
    if (ALLOWED_REGIONS.has(region)) {
      return region;
    }
  }
  return null;
}

function stripRegionOverrides(query) {
  return query
    .replace(/\[(au|ca|de|es|fr|in|it|jp|us|uk)\]/gi, " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

function parseYear(value) {
  if (!value || value.length < 4) return null;
  const parsed = Number.parseInt(value.slice(0, 4), 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeHttpsUrl(raw) {
  if (!raw) return null;
  try {
    const parsed = new URL(raw);
    return parsed.protocol === "https:" ? parsed.toString() : null;
  } catch {
    return null;
  }
}

function sampleQueries(count) {
  return Array.from({ length: count }, (_, index) => {
    const region = index % 2 === 0 ? "[us]" : "[uk]";
    const asin = `B0${String(index).padStart(8, "0")}`;
    return `${region} Project Book ${index} ${asin}`;
  });
}

function sampleAudibleItems(count) {
  return Array.from({ length: count }, (_, index) => ({
    asin: `B0${String(index).padStart(8, "0")}`,
    title: `Synthetic Title ${index}`,
    subtitle: index % 3 === 0 ? `Subtitle ${index}` : null,
    authors: [{ name: `Author ${index % 17}` }],
    narrators: [{ name: `Narrator ${index % 13}` }],
    release_date: `20${(10 + (index % 15)).toString().padStart(2, "0")}-01-01`,
    runtime_length_min: 420 + index,
    publisher_summary: `Summary ${index}`,
    merchandising_summary: null,
    product_images: {
      500: "https://images.example.com/cover-500.jpg",
      1024: "https://images.example.com/cover-1024.jpg",
    },
  }));
}

function mapAudibleItem(item) {
  const baseTitle = (item.title ?? item.asin).trim();
  const title = item.subtitle?.trim() ? `${baseTitle}: ${item.subtitle.trim()}` : baseTitle;

  return {
    source: "audnexus",
    source_id: item.asin,
    title,
    authors: (item.authors ?? []).map((author) => author.name),
    narrators: (item.narrators ?? []).map((narrator) => narrator.name),
    description: item.publisher_summary ?? item.merchandising_summary,
    published_year: parseYear(item.release_date),
    duration_seconds: item.runtime_length_min ? Math.round(item.runtime_length_min * 60) : null,
    cover_url:
      normalizeHttpsUrl(item.product_images?.["1024"]) ??
      normalizeHttpsUrl(item.product_images?.["500"]),
  };
}

function runSyntheticMetadataBench() {
  const queries = sampleQueries(SYNTHETIC_WORKLOAD.queryCount);
  const items = sampleAudibleItems(SYNTHETIC_WORKLOAD.itemCount);

  let checksum = 0;
  const start = performance.now();

  for (let loop = 0; loop < SYNTHETIC_WORKLOAD.loops; loop += 1) {
    for (const query of queries) {
      const asin = extractAsin(query);
      const region = extractRegionOverride(query) ?? "us";
      const cleaned = stripRegionOverrides(query);
      checksum += (asin?.length ?? 0) + region.length + cleaned.length;
    }

    for (const item of items) {
      const mapped = mapAudibleItem(item);
      checksum += mapped.title.length + (mapped.published_year ?? 0);
    }
  }

  const elapsedMs = performance.now() - start;
  return {
    value: elapsedMs,
    details: {
      mode: "synthetic",
      query_count: SYNTHETIC_WORKLOAD.queryCount,
      item_count: SYNTHETIC_WORKLOAD.itemCount,
      loops: SYNTHETIC_WORKLOAD.loops,
      operations:
        SYNTHETIC_WORKLOAD.loops * (SYNTHETIC_WORKLOAD.queryCount + SYNTHETIC_WORKLOAD.itemCount),
      checksum,
      elapsed_ms: Number(elapsedMs.toFixed(3)),
    },
  };
}

async function fetchWithTiming(url, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  const started = performance.now();
  try {
    const response = await fetch(url, {
      signal: controller.signal,
      headers: {
        "user-agent": "audiobook-boss-perf/1.0",
      },
    });

    const text = await response.text();
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    return {
      ok: true,
      elapsedMs: performance.now() - started,
      bytes: text.length,
    };
  } catch (error) {
    return {
      ok: false,
      elapsedMs: performance.now() - started,
      error: error instanceof Error ? error.message : String(error),
    };
  } finally {
    clearTimeout(timer);
  }
}

async function runRealMetadataBench() {
  const allowNetwork = process.env.ABB_PERF_ALLOW_NETWORK === "1";
  if (!allowNetwork) {
    const fallback = runSyntheticMetadataBench();
    return {
      ...fallback,
      details: {
        ...fallback.details,
        mode: "fixture-fallback",
        fallback_reason: "Set ABB_PERF_ALLOW_NETWORK=1 to enable network requests.",
      },
    };
  }

  const samples = [];
  for (const url of REAL_WORKLOAD.requests) {
    const sample = await fetchWithTiming(url, REAL_WORKLOAD.timeoutMs);
    if (sample.ok) {
      samples.push(sample);
      continue;
    }

    const fallback = runSyntheticMetadataBench();
    return {
      ...fallback,
      details: {
        ...fallback.details,
        mode: "fixture-fallback",
        fallback_reason: `Network request failed for ${url}: ${sample.error}`,
      },
    };
  }

  const elapsed = samples.map((sample) => sample.elapsedMs);
  const avgElapsed = elapsed.reduce((sum, value) => sum + value, 0) / elapsed.length;

  return {
    value: avgElapsed,
    details: {
      mode: "real",
      requests: REAL_WORKLOAD.requests.length,
      request_latencies_ms: elapsed.map((value) => Number(value.toFixed(3))),
      bytes: samples.reduce((sum, sample) => sum + (sample.bytes ?? 0), 0),
      elapsed_avg_ms: Number(avgElapsed.toFixed(3)),
    },
  };
}

export const benchmark = {
  name: "metadata-lookup-latency",
  description: "Metadata lookup pipeline latency (synthetic) and optional network probes (real).",
  userImpact: "Book metadata resolves near-instantly when adding files to the library",
  phase: 1,
  metricType: "duration_ms",
  direction: "lower_is_better",
  warmupRuns: 1,
  async run({ mode }) {
    if (mode === "real") {
      return runRealMetadataBench();
    }
    return runSyntheticMetadataBench();
  },
};

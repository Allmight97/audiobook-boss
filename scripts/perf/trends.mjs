import { sparklineAscii, formatDelta } from "./shared/stats.mjs";

function keyFor(row) {
  return `${row.bench_name}::${row.mode}`;
}

function formatNumber(value, digits = 3) {
  if (!Number.isFinite(value)) return "n/a";
  return value.toFixed(digits);
}

function statusEmoji(status) {
  if (status === "warn") return "WARN";
  if (status === "improved") return "IMPROVED";
  if (status === "ok") return "OK";
  if (status === "missing") return "MISSING_BASELINE";
  if (status === "skipped") return "SKIPPED";
  return status?.toUpperCase() ?? "UNKNOWN";
}

function buildSparklineRows(historyRows, latestRows) {
  const grouped = new Map();

  for (const row of historyRows) {
    const key = keyFor(row);
    if (!grouped.has(key)) {
      grouped.set(key, []);
    }
    grouped.get(key).push(row);
  }

  for (const rows of grouped.values()) {
    rows.sort((a, b) => a.timestamp.localeCompare(b.timestamp));
  }

  return latestRows.map((row) => {
    const key = keyFor(row);
    const history = grouped.get(key) ?? [];
    const values = history
      .map((entry) => entry.median)
      .filter((value) => Number.isFinite(value));
    const recentValues = values.slice(-12);

    return {
      bench_name: row.bench_name,
      mode: row.mode,
      points: recentValues.length,
      sparkline: sparklineAscii(recentValues),
      recent: recentValues.map((value) => Number(value.toFixed(3))),
    };
  });
}

export function buildLatestMarkdown({ summary, latestRows, historyRows }) {
  const lines = [];
  lines.push("# Performance Results");
  lines.push("");
  lines.push(`- Timestamp: ${summary.timestamp}`);
  lines.push(`- Mode: ${summary.mode}`);
  lines.push(`- Git: ${summary.git_branch} (${summary.git_sha})`);
  lines.push(`- Host: ${summary.host_os} | ${summary.cpu_info}`);
  lines.push(
    `- Runs: ${summary.runs} (warmup ${summary.warmup_runs ?? "benchmark defaults"})`
  );
  lines.push("");

  lines.push("## Latest Benchmarks");
  lines.push("");
  lines.push("| Bench | Metric | Median | P95 | Delta vs Baseline | Status |");
  lines.push("| --- | --- | ---: | ---: | ---: | --- |");

  for (const row of latestRows) {
    lines.push(
      `| ${row.bench_name} | ${row.metric_type} | ${formatNumber(row.median)} | ${formatNumber(row.p95)} | ${formatDelta(row.delta_pct)} | ${statusEmoji(row.status)} |`
    );
  }

  const sparkRows = buildSparklineRows(historyRows, latestRows);

  lines.push("");
  lines.push("## Trend Snapshot (Last 12)");
  lines.push("");
  lines.push("| Bench | Mode | Points | Trend | Recent Medians |");
  lines.push("| --- | --- | ---: | --- | --- |");

  for (const row of sparkRows) {
    lines.push(
      `| ${row.bench_name} | ${row.mode} | ${row.points} | ${row.sparkline || "n/a"} | ${row.recent.length > 0 ? row.recent.join(", ") : "n/a"} |`
    );
  }

  lines.push("");
  lines.push("## Notes");
  lines.push("");
  lines.push("- `warn` indicates >15% regression versus baseline in the wrong direction.");
  lines.push("- `improved` indicates >15% improvement versus baseline.");
  lines.push("- `missing` means no baseline entry exists for that bench/mode.");

  return `${lines.join("\n")}\n`;
}

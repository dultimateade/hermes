#!/usr/bin/env python3
"""
Benchmark regression checking script for Hermes contracts.
Runs criterion benchmarks, extracts timing estimates, compares against
configured absolute thresholds and relative regression limits, and generates
markdown/JSON reports.
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(
        description="Run and check contract benchmarks against regression thresholds."
    )
    parser.add_argument(
        "--contract",
        default="analytics",
        help="Contract package name to benchmark (default: analytics)",
    )
    parser.add_argument(
        "--thresholds",
        default="contracts/analytics/benches/thresholds.json",
        help="Path to thresholds JSON configuration file",
    )
    parser.add_argument(
        "--baseline",
        default=None,
        help="Baseline name to compare against (passed to cargo bench -- --baseline <name>)",
    )
    parser.add_argument(
        "--save-baseline",
        default=None,
        help="Save current benchmark as a named baseline (passed to cargo bench -- --save-baseline <name>)",
    )
    parser.add_argument(
        "--skip-run",
        action="store_true",
        help="Skip executing cargo bench, only evaluate existing target/criterion results",
    )
    parser.add_argument(
        "--report-md",
        default="benchmark_report.md",
        help="Path to write markdown summary report",
    )
    parser.add_argument(
        "--report-json",
        default="benchmark_results.json",
        help="Path to write JSON benchmark summary",
    )
    parser.add_argument(
        "--baseline-json",
        default="benchmarks/baseline.json",
        help="Path to baseline JSON file for regression comparison",
    )
    parser.add_argument(
        "--max-regression-pct",
        type=float,
        default=10.0,
        help="Maximum allowed regression percentage vs baseline (default: 10.0)",
    )
    return parser.parse_args()


def format_duration(ns: float) -> str:
    """Format nanoseconds into human-readable string."""
    if ns < 1_000:
        return f"{ns:.2f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1_000:.2f} µs"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    else:
        return f"{ns / 1_000_000_000:.2f} s"


def run_cargo_bench(contract: str, baseline: str = None, save_baseline: str = None):
    """Execute cargo bench for the specified contract."""
    cmd = ["cargo", "bench", "-p", contract]
    extra_args = []
    if baseline:
        extra_args.extend(["--baseline", baseline])
    if save_baseline:
        extra_args.extend(["--save-baseline", save_baseline])

    if extra_args:
        cmd.extend(["--", *extra_args])

    print(f"==> Running: {' '.join(cmd)}")
    res = subprocess.run(cmd)
    if res.returncode != 0:
        print(f"Error: Benchmark command failed with exit code {res.returncode}", file=sys.stderr)
        sys.exit(res.returncode)


def load_thresholds(thresholds_path: str) -> dict:
    """Load benchmark thresholds from JSON file."""
    path = Path(thresholds_path)
    if not path.exists():
        print(f"Warning: Thresholds file '{thresholds_path}' not found. Using default thresholds.", file=sys.stderr)
        return {"thresholds": {}, "default_max_regression_pct": 20.0}
    
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def discover_entrypoints(contract: str) -> list:
    """Discover state-changing public entrypoints for a contract.

    Parses the contract's Rust sources for `pub fn` definitions inside
    `#[contractimpl]` blocks and returns their names. Falls back to an empty
    list if the contract sources cannot be located.
    """
    contract_dir = Path("contracts") / contract
    src_dir = contract_dir / "src"
    if not src_dir.exists():
        print(
            f"Warning: Could not locate sources for contract '{contract}' at '{src_dir}'. "
            "Skipping entrypoint coverage check.",
            file=sys.stderr,
        )
        return []

    entrypoints = []
    for rs_file in sorted(src_dir.rglob("*.rs")):
        try:
            content = rs_file.read_text(encoding="utf-8")
        except Exception as e:
            print(f"Warning: Failed reading {rs_file}: {e}", file=sys.stderr)
            continue

        in_impl = False
        depth = 0
        for line in content.splitlines():
            stripped = line.strip()
            if not in_impl:
                if stripped.startswith("#[contractimpl]"):
                    in_impl = True
                    depth = 0
                continue

            depth += line.count("{") - line.count("}")
            if depth <= 0 and "{" in line and "}" in line:
                # Single-line impl block or closing brace on same line.
                pass

            if stripped.startswith("pub fn "):
                name = stripped[len("pub fn "):].split("(")[0].strip()
                if name and name not in entrypoints:
                    entrypoints.append(name)

            if depth <= 0 and "}" in line:
                in_impl = False

    return entrypoints


def check_entrypoint_coverage(contract: str, config: dict) -> list:
    """Return entrypoints that are missing from the thresholds configuration."""
    thresholds = config.get("thresholds", {})
    entrypoints = discover_entrypoints(contract)
    missing = [ep for ep in entrypoints if ep not in thresholds]
    return missing


def find_benchmark_results(criterion_dir: Path) -> dict:
    """Traverse criterion output directory to collect benchmark metrics."""
    results = {}
    if not criterion_dir.exists():
        return results

    for estimates_file in criterion_dir.rglob("estimates.json"):
        if "new" in estimates_file.parts:
            # Benchmark name is typically parent parent directory name
            bench_dir = estimates_file.parent.parent
            bench_name = bench_dir.name
            
            try:
                with open(estimates_file, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    mean_estimate = data.get("mean", {}).get("point_estimate")
                    median_estimate = data.get("median", {}).get("point_estimate")
                    std_dev = data.get("std_dev", {}).get("point_estimate")

                    change_file = bench_dir / "change" / "estimates.json"
                    change_pct = None
                    if change_file.exists():
                        try:
                            with open(change_file, "r", encoding="utf-8") as cf:
                                cdata = json.load(cf)
                                change_pct = cdata.get("mean", {}).get("point_estimate")
                                if change_pct is not None:
                                    change_pct = change_pct * 100.0
                        except Exception:
                            pass

                    results[bench_name] = {
                        "name": bench_name,
                        "mean_ns": mean_estimate,
                        "median_ns": median_estimate,
                        "std_dev_ns": std_dev,
                        "change_pct": change_pct,
                    }
            except Exception as e:
                print(f"Warning: Failed reading {estimates_file}: {e}", file=sys.stderr)

    return results


def load_baseline(baseline_path: str) -> dict:
    """Load baseline benchmark data from JSON file.

    Returns the ``benchmarks`` map from the baseline file, or an empty dict if
    the file is missing or malformed.
    """
    path = Path(baseline_path)
    if not path.exists():
        print(
            f"Warning: Baseline file '{baseline_path}' not found. Skipping baseline comparison.",
            file=sys.stderr,
        )
        return {}

    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        print(f"Warning: Failed reading baseline '{baseline_path}': {e}", file=sys.stderr)
        return {}

    benchmarks = data.get("benchmarks", {})
    if not isinstance(benchmarks, dict):
        print(
            f"Warning: Baseline '{baseline_path}' has no valid 'benchmarks' map. "
            "Skipping baseline comparison.",
            file=sys.stderr,
        )
        return {}

    return benchmarks


def compare_to_baseline(
    benchmarks: dict, baseline: dict, default_max_regression_pct: float
) -> tuple:
    """Compare current benchmark results against the baseline.

    Returns a tuple of (comparisons, has_regressions) where comparisons is a
    list of per-entrypoint dicts describing the comparison outcome.
    """
    comparisons = []
    has_regressions = False

    for name, base in baseline.items():
        base_mean = base.get("mean_ns") if isinstance(base, dict) else None
        current = benchmarks.get(name)
        current_mean = current.get("mean_ns") if current else None

        max_reg_pct = default_max_regression_pct
        if isinstance(base, dict) and base.get("max_regression_pct") is not None:
            max_reg_pct = base["max_regression_pct"]

        if base_mean is None or current_mean is None:
            comparisons.append({
                "name": name,
                "baseline_ns": base_mean,
                "current_ns": current_mean,
                "change_pct": None,
                "max_reg_pct": max_reg_pct,
                "status": "SKIPPED",
                "reason": "Missing baseline or current measurement",
            })
            continue

        if base_mean > 0:
            change_pct = ((current_mean - base_mean) / base_mean) * 100.0
        else:
            change_pct = 0.0 if current_mean == 0 else float("inf")

        if change_pct > max_reg_pct:
            status = "FAILED"
            has_regressions = True
            reason = (
                f"Regression of +{change_pct:.2f}% exceeded baseline limit of "
                f"+{max_reg_pct:.2f}%"
            )
        else:
            status = "PASSED"
            reason = f"Within baseline limit of +{max_reg_pct:.2f}%"

        comparisons.append({
            "name": name,
            "baseline_ns": base_mean,
            "current_ns": current_mean,
            "change_pct": change_pct,
            "max_reg_pct": max_reg_pct,
            "status": status,
            "reason": reason,
        })

    return comparisons, has_regressions


def evaluate_benchmarks(benchmarks: dict, config: dict) -> tuple:
    """Evaluate benchmark results against thresholds."""
    thresholds = config.get("thresholds", {})
    default_regression_pct = config.get("default_max_regression_pct", 20.0)

    evaluations = []
    has_regressions = False

    for name, data in benchmarks.items():
        mean_ns = data.get("mean_ns")
        change_pct = data.get("change_pct")

        thresh = thresholds.get(name, {})
        max_time_ns = thresh.get("max_time_ns")
        max_reg_pct = thresh.get("max_regression_pct", default_regression_pct)

        status = "PASSED"
        reasons = []

        if max_time_ns is not None and mean_ns is not None:
            if mean_ns > max_time_ns:
                status = "FAILED"
                reasons.append(
                    f"Mean time {format_duration(mean_ns)} exceeded max threshold {format_duration(max_time_ns)}"
                )

        if change_pct is not None and max_reg_pct is not None:
            if change_pct > max_reg_pct:
                status = "FAILED"
                reasons.append(
                    f"Regression of +{change_pct:.2f}% exceeded limit of +{max_reg_pct:.2f}%"
                )

        if status == "FAILED":
            has_regressions = True

        evaluations.append({
            "name": name,
            "mean_ns": mean_ns,
            "max_time_ns": max_time_ns,
            "change_pct": change_pct,
            "max_reg_pct": max_reg_pct,
            "status": status,
            "reasons": reasons,
        })

    return evaluations, has_regressions


def generate_reports(
    evaluations: list,
    has_regressions: bool,
    report_md_path: str,
    report_json_path: str,
    contract: str,
    baseline_comparisons: list = None,
):
    """Write markdown and JSON reports."""
    if baseline_comparisons is None:
        baseline_comparisons = []

    # Write JSON report
    with open(report_json_path, "w", encoding="utf-8") as f:
        json.dump({
            "contract": contract,
            "has_regressions": has_regressions,
            "results": evaluations,
            "baseline_comparisons": baseline_comparisons,
        }, f, indent=2)

    # Build Markdown report
    lines = [
        f"# ⚡ Benchmark Regression Report: `{contract}`",
        "",
        f"**Overall Status**: {'❌ REGRESSION DETECTED' if has_regressions else '✅ ALL THRESHOLDS PASSED'}",
        "",
        "| Benchmark | Measured (Mean) | Max Limit | Relative Change | Status |",
        "| :--- | :--- | :--- | :--- | :--- |",
    ]

    for item in evaluations:
        name = item["name"]
        mean_str = format_duration(item["mean_ns"]) if item["mean_ns"] else "N/A"
        limit_str = format_duration(item["max_time_ns"]) if item["max_time_ns"] else "None"
        change_str = f"{item['change_pct']:+.2f}%" if item["change_pct"] is not None else "N/A"
        status_icon = "✅ PASS" if item["status"] == "PASSED" else "❌ FAIL"

        lines.append(f"| `{name}` | {mean_str} | {limit_str} | {change_str} | {status_icon} |")

    if baseline_comparisons:
        lines.append("")
        lines.append("### 📊 Baseline Comparison")
        lines.append("")
        lines.append("| Entrypoint | Baseline (Mean) | Current (Mean) | Change | Limit | Status |")
        lines.append("| :--- | :--- | :--- | :--- | :--- | :--- |")
        for item in baseline_comparisons:
            base_str = (
                format_duration(item["baseline_ns"])
                if item["baseline_ns"] is not None
                else "N/A"
            )
            cur_str = (
                format_duration(item["current_ns"])
                if item["current_ns"] is not None
                else "N/A"
            )
            change_str = (
                f"{item['change_pct']:+.2f}%"
                if item["change_pct"] is not None
                else "N/A"
            )
            limit_str = f"+{item['max_reg_pct']:.2f}%"
            status_icon = "✅ PASS" if item["status"] == "PASSED" else (
                "❌ FAIL" if item["status"] == "FAILED" else "⚠️ SKIP"
            )
            lines.append(
                f"| `{item['name']}` | {base_str} | {cur_str} | {change_str} | "
                f"{limit_str} | {status_icon} |"
            )

    if has_regressions:
        lines.append("")
        lines.append("### ⚠️ Regression Details")
        for item in evaluations:
            if item["status"] == "FAILED":
                lines.append(f"- **`{item['name']}`**:")
                for r in item["reasons"]:
                    lines.append(f"  - {r}")
        for item in baseline_comparisons:
            if item["status"] == "FAILED":
                lines.append(f"- **`{item['name']}`** (baseline):")
                lines.append(f"  - {item['reason']}")

    md_content = "\n".join(lines) + "\n"

    with open(report_md_path, "w", encoding="utf-8") as f:
        f.write(md_content)

    # Print to stdout
    print("\n" + md_content)

    # If running inside GitHub Actions, append to GITHUB_STEP_SUMMARY
    github_step_summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if github_step_summary:
        try:
            with open(github_step_summary, "a", encoding="utf-8") as f:
                f.write(md_content)
            print("Successfully wrote benchmark summary to $GITHUB_STEP_SUMMARY")
        except Exception as e:
            print(f"Warning: Failed to write to GITHUB_STEP_SUMMARY: {e}", file=sys.stderr)


def main():
    args = parse_args()

    if not args.skip_run:
        run_cargo_bench(args.contract, args.baseline, args.save_baseline)

    criterion_dir = Path("target") / "criterion"
    benchmarks = find_benchmark_results(criterion_dir)

    if not benchmarks:
        print("Warning: No benchmark results found in target/criterion.", file=sys.stderr)

    config = load_thresholds(args.thresholds)
    evaluations, has_regressions = evaluate_benchmarks(benchmarks, config)

    baseline = load_baseline(args.baseline_json)
    baseline_comparisons, baseline_regressions = compare_to_baseline(
        benchmarks, baseline, args.max_regression_pct
    )
    for item in baseline_comparisons:
        if item["status"] == "PASSED":
            print(
                f"✅ PASS {item['name']}: {item['change_pct']:+.2f}% "
                f"(limit +{item['max_reg_pct']:.2f}%)"
            )
        elif item["status"] == "FAILED":
            print(
                f"❌ FAIL {item['name']}: {item['change_pct']:+.2f}% "
                f"(limit +{item['max_reg_pct']:.2f}%)",
                file=sys.stderr,
            )
        else:
            print(f"⚠️ SKIP {item['name']}: {item['reason']}", file=sys.stderr)

    if baseline_regressions:
        has_regressions = True

    missing_entrypoints = check_entrypoint_coverage(args.contract, config)
    if missing_entrypoints:
        print(
            "\n❌ The following state-changing entrypoints are missing from "
            f"'{args.thresholds}':",
            file=sys.stderr,
        )
        for ep in missing_entrypoints:
            print(f"  - {ep}", file=sys.stderr)
        print(
            "Add threshold entries for these entrypoints so regressions are caught.",
            file=sys.stderr,
        )
        has_regressions = True

    generate_reports(
        evaluations,
        has_regressions,
        args.report_md,
        args.report_json,
        args.contract,
        baseline_comparisons,
    )

    if has_regressions:
        print("\n❌ Performance regression detected! CI benchmark check failed.", file=sys.stderr)
        sys.exit(1)
    else:
        print("\n✅ All benchmark regression thresholds satisfied.", file=sys.stderr)
        sys.exit(0)


if __name__ == "__main__":
    main()

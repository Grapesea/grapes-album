//! Acceptance harness for the deterministic Time/Tone classifiers.
//!
//! Run against an indexed Grapes workspace:
//!
//! ```text
//! cargo run --manifest-path crates/grapes-core/Cargo.toml \
//!   --example derived_classify -- "D:\Photos" --reconcile --csv report.csv
//! ```
//!
//! Add `--apply` to persist the derived rows. Without it the command only reads
//! the workspace and prints what it would classify, which is the safer way to
//! inspect a real library first.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use grapes_core::Workspace;

struct Options {
    workspace: PathBuf,
    apply: bool,
    reconcile: bool,
    limit: Option<usize>,
    csv: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    let mut workspace = if options.reconcile {
        Workspace::open_and_reconcile(&options.workspace)
    } else {
        Workspace::open(&options.workspace)
    }
    .map_err(|error| format!("打开工作区失败：{error}"))?;

    let snapshot = workspace.snapshot().map_err(|error| error.to_string())?;
    let mut assets = snapshot.assets.iter().collect::<Vec<_>>();
    if let Some(limit) = options.limit {
        assets.truncate(limit);
    }

    let mut csv_rows = vec!["path,time,time_confidence,tone,tone_confidence,evidence".to_owned()];
    let mut time_labeled = 0usize;
    let mut time_abstained = 0usize;
    let mut tone_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut saturation_abstained = 0usize;
    let mut manual_time = 0usize;
    let mut manual_tone = 0usize;

    for asset in &assets {
        let suggestions = workspace
            .derived_classifications(&asset.id)
            .map_err(|error| format!("{}：{error}", asset.relative_path))?;
        let time = suggestions
            .iter()
            .find(|item| item.dimension_id == "dimension:time");
        let tones = suggestions
            .iter()
            .filter(|item| item.dimension_id == "dimension:tone")
            .collect::<Vec<_>>();

        if asset
            .classifications
            .iter()
            .any(|item| item.dimension_id == "dimension:time" && item.source != "derived")
        {
            manual_time += 1;
        }
        if asset
            .classifications
            .iter()
            .any(|item| item.dimension_id == "dimension:tone" && item.source != "derived")
        {
            manual_tone += 1;
        }

        if time.is_some() {
            time_labeled += 1;
        } else {
            time_abstained += 1;
        }
        for tone in &tones {
            *tone_counts.entry(tone.value_name.to_owned()).or_default() += 1;
        }
        if !tones.iter().any(|item| {
            item.value_id.ends_with("high-saturation") || item.value_id.ends_with("low-saturation")
        }) {
            saturation_abstained += 1;
        }

        println!("{}", asset.relative_path);
        match time {
            Some(time) => println!(
                "  Time  {:<12} {:.2}  {}  {}",
                time.value_name, time.confidence, time.method, time.evidence
            ),
            None => println!("  Time  (弃权：缺少时间、GPS 或光带不稳定)"),
        }
        if tones.is_empty() {
            println!("  Tone  (弃权：没有可用缩略图)");
        } else {
            let names = tones
                .iter()
                .map(|item| format!("{} {:.2}", item.value_name, item.confidence))
                .collect::<Vec<_>>()
                .join(" + ");
            println!("  Tone  {names}");
            println!("        {}", tones[0].evidence);
        }

        let time_name = time.map(|item| item.value_name).unwrap_or("");
        let time_confidence = time
            .map(|item| format!("{:.2}", item.confidence))
            .unwrap_or_default();
        let time_evidence = time.map(|item| item.evidence.as_str()).unwrap_or("");
        let tone_name = tones
            .iter()
            .map(|item| item.value_name)
            .collect::<Vec<_>>()
            .join("|");
        let tone_confidence = tones
            .iter()
            .map(|item| format!("{:.2}", item.confidence))
            .collect::<Vec<_>>()
            .join("|");
        csv_rows.push(format!(
            "{},{},{},{},{},{}",
            escape(&asset.relative_path),
            escape(time_name),
            time_confidence,
            escape(&tone_name),
            tone_confidence,
            escape(time_evidence)
        ));
    }

    println!();
    println!("相片总数：{}", assets.len());
    println!(
        "Time：已判定 {}（{:.0}%），弃权 {}",
        time_labeled,
        percentage(time_labeled, assets.len()),
        time_abstained
    );
    println!("Time：人工锁定 {} 张（apply 会保留人工值）", manual_time);
    println!(
        "Tone：{}",
        tone_counts
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join("，")
    );
    println!(
        "Tone 饱和度：已判定 {}，未判定 {}",
        assets.len().saturating_sub(saturation_abstained),
        saturation_abstained
    );
    println!("Tone：人工锁定 {} 张（apply 会保留人工值）", manual_tone);

    if let Some(path) = &options.csv {
        fs::write(path, csv_rows.join("\n"))
            .map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
        println!("CSV 已写入 {}", path.display());
    }

    if options.apply {
        let mut applied = 0usize;
        for asset in &assets {
            workspace
                .apply_derived_classifications(&asset.id)
                .map_err(|error| format!("{}：{error}", asset.relative_path))?;
            applied += 1;
        }
        let after = workspace.snapshot().map_err(|error| error.to_string())?;
        let derived_rows = after
            .assets
            .iter()
            .flat_map(|asset| &asset.classifications)
            .filter(|item| item.source == "derived")
            .count();
        println!();
        println!("已对 {applied} 张相片执行 apply，当前派生分类行数：{derived_rows}");
    } else {
        println!();
        println!("本次为只读检查；加 --apply 才会写入派生分类。");
    }

    Ok(())
}

fn parse_options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut workspace = None;
    let mut apply = false;
    let mut reconcile = false;
    let mut limit = None;
    let mut csv = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--apply" => apply = true,
            "--reconcile" => reconcile = true,
            "--limit" => {
                let value = args.next().ok_or("--limit 需要数字")?;
                limit = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit 需要数字".to_owned())?,
                );
            }
            "--csv" => {
                let value = args.next().ok_or("--csv 需要路径")?;
                csv = Some(PathBuf::from(value));
            }
            "-h" | "--help" => return Err(usage()),
            other if other.starts_with('-') => {
                return Err(format!("未知参数 {other}\n\n{}", usage()))
            }
            other => {
                if workspace.is_some() {
                    return Err("只能指定一个工作区路径".to_owned());
                }
                workspace = Some(PathBuf::from(other));
            }
        }
    }
    Ok(Options {
        workspace: workspace.ok_or_else(usage)?,
        apply,
        reconcile,
        limit,
        csv,
    })
}

fn usage() -> String {
    "用法：cargo run --example derived_classify -- <工作区路径> [--reconcile] [--apply] [--limit N] [--csv 文件]\n\
     \n\
     --reconcile  先扫描文件夹并刷新索引/缩略图（新工作区需要）\n\
     --apply      写入派生分类；缺省只读预览\n\
     --limit N    只检查前 N 张相片\n\
     --csv 文件   把结果导出为 CSV"
        .to_owned()
}

fn percentage(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

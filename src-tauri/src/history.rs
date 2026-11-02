use crate::bench::BenchResult;
use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub ts: u64,
    pub game: String,
    pub process: String,
    /// antes | depois | manual | ab-antes | ab-depois
    pub label: String,
    /// Ajustes ativos no momento da medição.
    pub tweaks: Vec<String>,
    pub result: BenchResult,
}

fn path() -> Result<std::path::PathBuf, String> {
    Ok(crate::tweaks::data_dir()?.join("bench_history.json"))
}

pub fn list() -> Vec<Entry> {
    let mut v: Vec<Entry> = path()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    v.sort_by(|a, b| b.id.cmp(&a.id));
    v
}

fn save(v: &[Entry]) -> Result<(), String> {
    let json = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    std::fs::write(path()?, json).map_err(|e| e.to_string())
}

pub fn add(game: &str, process: &str, label: &str, result: BenchResult) -> Result<Entry, String> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let tweaks = crate::tweaks::list()
        .map(|l| l.into_iter().filter(|t| t.applied()).map(|t| t.id().to_string()).collect())
        .unwrap_or_default();
    let entry = Entry {
        id: ms,
        ts: ms / 1000,
        game: game.into(),
        process: process.into(),
        label: label.into(),
        tweaks,
        result,
    };
    let mut all = list();
    all.insert(0, entry.clone());
    all.truncate(MAX_ENTRIES);
    save(&all)?;
    Ok(entry)
}

pub fn delete(id: u64) -> Result<(), String> {
    let all: Vec<Entry> = list().into_iter().filter(|e| e.id != id).collect();
    save(&all)
}

pub fn clear() -> Result<(), String> {
    save(&[])
}

// ---------------------------------------------------------------- teste A/B

#[derive(Serialize, Debug, PartialEq)]
pub struct Verdict {
    pub keep: bool,
    pub gain_pct: f64,
    pub low_gain_pct: f64,
    /// Variação entre as duas medições iguais (ruído do próprio teste), em %.
    pub noise_pct: f64,
    pub reason: String,
}

fn mean(v: impl Iterator<Item = f64>) -> f64 {
    let (s, n) = v.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    if n == 0 { 0.0 } else { s / n as f64 }
}

fn spread_pct(runs: &[BenchResult]) -> f64 {
    if runs.len() < 2 {
        return 0.0;
    }
    let (min, max) = runs.iter().fold((f64::MAX, 0.0f64), |(lo, hi), r| (lo.min(r.avg_fps), hi.max(r.avg_fps)));
    let m = mean(runs.iter().map(|r| r.avg_fps));
    if m <= 0.0 { 0.0 } else { (max - min) / m * 100.0 }
}

/// Decide se um ajuste vale a pena. Só mantém se o ganho de FPS médio passar do ruído medido entre as
/// corridas "antes" (e de 3%) e se o 1% low não piorar além de 3%.
pub fn verdict(before: &[BenchResult], after: &[BenchResult]) -> Verdict {
    let (a, b) = (mean(before.iter().map(|r| r.avg_fps)), mean(after.iter().map(|r| r.avg_fps)));
    let (la, lb) = (mean(before.iter().map(|r| r.low_1pct_fps)), mean(after.iter().map(|r| r.low_1pct_fps)));
    let gain = if a > 0.0 { (b - a) / a * 100.0 } else { 0.0 };
    let low_gain = if la > 0.0 { (lb - la) / la * 100.0 } else { 0.0 };
    let noise = spread_pct(before).max(spread_pct(after));
    let needed = (noise * 1.5).max(3.0);
    let (keep, reason) = if gain >= needed && low_gain >= -3.0 {
        (true, format!("Ganho de {gain:.1}% acima do ruído ({noise:.1}%)."))
    } else if gain >= needed {
        (false, format!("FPS médio subiu {gain:.1}%, mas o 1% low piorou {:.1}%.", -low_gain))
    } else {
        (false, format!("Ganho de {gain:.1}% não passa do mínimo de {needed:.1}% (ruído do teste: {noise:.1}%)."))
    };
    Verdict { keep, gain_pct: gain, low_gain_pct: low_gain, noise_pct: noise, reason }
}

// ------------------------------------------------------------------ relatório

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn chart(series: &[f32], color: &str, ymax: f32) -> String {
    if series.len() < 2 {
        return String::new();
    }
    let pts: Vec<String> = series
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f32 / (series.len() - 1) as f32 * 600.0;
            let y = 120.0 - (v / ymax).min(1.0) * 110.0;
            format!("{x:.1},{y:.1}")
        })
        .collect();
    format!("<polyline fill=\"none\" stroke=\"{color}\" stroke-width=\"1.4\" points=\"{}\"/>", pts.join(" "))
}

/// Página HTML única (sem dependências) com a comparação das medições escolhidas.
pub fn report_html(entries: &[Entry]) -> String {
    let colors = ["#ff3b4a", "#4ea1ff", "#7dd87d", "#f5b301"];
    let ymax = entries.iter().flat_map(|e| e.result.series.iter().cloned()).fold(16.7f32, f32::max).max(1.0);
    let mut rows = String::new();
    let mut lines = String::new();
    let mut legend = String::new();
    for (i, e) in entries.iter().enumerate() {
        let c = colors[i % colors.len()];
        rows += &format!(
            "<tr><td><span class=\"dot\" style=\"background:{c}\"></span>{}</td><td>{}</td><td>{:.1}</td><td>{:.1}</td><td>{:.1}</td><td>{}</td></tr>",
            esc(&e.label),
            esc(&e.game),
            e.result.avg_fps,
            e.result.low_1pct_fps,
            e.result.max_frametime_ms,
            e.tweaks.len()
        );
        lines += &chart(&e.result.series, c, ymax);
        legend += &format!("<span><i style=\"background:{c}\"></i>{} · {}</span>", esc(&e.label), esc(&e.game));
    }
    let delta = if entries.len() >= 2 {
        let (a, b) = (&entries[entries.len() - 1].result, &entries[0].result);
        format!(
            "<p class=\"delta\">FPS médio: {:+.1}% · 1% low: {:+.1}% (da medição mais antiga para a mais recente)</p>",
            (b.avg_fps - a.avg_fps) / a.avg_fps * 100.0,
            (b.low_1pct_fps - a.low_1pct_fps) / a.low_1pct_fps * 100.0
        )
    } else {
        String::new()
    };
    format!(
        "<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Relatório Nexus Optimizer</title>\
<style>body{{font:15px Segoe UI,sans-serif;background:#0a0a0a;color:#eee;max-width:760px;margin:30px auto;padding:0 18px}}\
h1{{color:#ff3b4a;margin-bottom:0}}small{{color:#888}}table{{width:100%;border-collapse:collapse;margin:18px 0}}\
th,td{{text-align:left;padding:8px;border-bottom:1px solid #2a1417}}th{{color:#888;font-weight:500}}\
.dot{{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:8px}}\
svg{{width:100%;background:#111;border:1px solid #2a1417;border-radius:10px}}.legend span{{margin-right:16px;color:#bbb}}\
.legend i{{display:inline-block;width:14px;height:3px;margin-right:6px;vertical-align:middle}}.delta{{color:#ff8d97}}</style>\
<h1>Nexus Optimizer</h1><small>Relatório de benchmark</small>{delta}\
<table><tr><th>Medição</th><th>Jogo</th><th>FPS médio</th><th>1% low</th><th>Maior frametime (ms)</th><th>Ajustes ativos</th></tr>{rows}</table>\
<h3>Frametime (ms, menor é melhor)</h3><svg viewBox=\"0 0 600 130\">{lines}</svg><div class=\"legend\">{legend}</div>\
<p><small>Uma única medição varia de uma corrida para outra. Repita a mesma cena antes de concluir algo.</small></p></html>"
    )
}

pub fn write_report(entries: &[Entry]) -> Result<std::path::PathBuf, String> {
    let home = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(home).join("Documents").join("Nexus Reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("relatorio-{}.html", crate::activity::now()));
    std::fs::write(&file, report_html(entries)).map_err(|e| e.to_string())?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(avg: f64, low: f64) -> BenchResult {
        BenchResult { frames: 1000, avg_fps: avg, low_1pct_fps: low, max_frametime_ms: 30.0, series: vec![10.0, 12.0, 30.0] }
    }

    #[test]
    fn verdict_keeps_real_gain_and_rejects_noise() {
        let v = verdict(&[r(100.0, 70.0), r(101.0, 71.0)], &[r(112.0, 74.0), r(113.0, 75.0)]);
        assert!(v.keep, "{}", v.reason);
        // ganho de 1% com ruído de 1%: não passa do mínimo de 3%
        assert!(!verdict(&[r(100.0, 70.0), r(101.0, 70.0)], &[r(101.0, 70.0), r(102.0, 70.0)]).keep);
        // FPS sobe mas o 1% low piora muito
        let bad_low = verdict(&[r(100.0, 70.0), r(100.5, 70.0)], &[r(115.0, 55.0), r(115.5, 55.0)]);
        assert!(!bad_low.keep && bad_low.reason.contains("1% low"));
        // ruído alto exige ganho maior
        assert!(!verdict(&[r(100.0, 70.0), r(120.0, 70.0)], &[r(115.0, 70.0), r(116.0, 70.0)]).keep);
    }

    #[test]
    fn report_escapes_html_and_has_chart() {
        let e = Entry { id: 1, ts: 1, game: "<b>x</b>".into(), process: "p".into(), label: "antes".into(), tweaks: vec![], result: r(60.0, 50.0) };
        let html = report_html(&[e.clone(), e]);
        assert!(html.contains("&lt;b&gt;x") && !html.contains("<b>x</b>"));
        assert!(html.contains("<polyline"));
    }
}

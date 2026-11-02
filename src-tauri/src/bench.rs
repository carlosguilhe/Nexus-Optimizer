use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BenchResult {
    pub frames: usize,
    pub avg_fps: f64,
    pub low_1pct_fps: f64,
    pub max_frametime_ms: f64,
    /// Frametimes (ms) reduzidos a no máximo SERIES_POINTS pontos (maior valor de cada trecho), para o gráfico.
    #[serde(default)]
    pub series: Vec<f32>,
}

const SERIES_POINTS: usize = 240;

/// Reduz a série mantendo os picos: cada ponto é o maior frametime do seu trecho.
pub fn downsample(frametimes: &[f64], points: usize) -> Vec<f32> {
    if frametimes.len() <= points {
        return frametimes.iter().map(|v| *v as f32).collect();
    }
    let chunk = (frametimes.len() as f64 / points as f64).ceil() as usize;
    frametimes
        .chunks(chunk)
        .map(|c| c.iter().cloned().fold(0.0, f64::max) as f32)
        .collect()
}

/// Calcula as métricas a partir dos frametimes (ms). "1% low" = FPS médio dos 1% frames mais lentos.
pub fn stats(frametimes: &[f64]) -> Option<BenchResult> {
    if frametimes.is_empty() {
        return None;
    }
    let total: f64 = frametimes.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut sorted = frametimes.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let worst = ((sorted.len() as f64 * 0.01).ceil() as usize).clamp(1, sorted.len());
    let worst_mean = sorted[sorted.len() - worst..].iter().sum::<f64>() / worst as f64;
    Some(BenchResult {
        frames: frametimes.len(),
        avg_fps: frametimes.len() as f64 / (total / 1000.0),
        low_1pct_fps: 1000.0 / worst_mean,
        max_frametime_ms: *sorted.last().unwrap(),
        series: downsample(frametimes, SERIES_POINTS),
    })
}

/// Lê o CSV do PresentMon e devolve os frametimes (coluna MsBetweenPresents ou FrameTime).
pub fn parse_csv(csv: &str) -> Result<Vec<f64>, String> {
    let mut lines = csv.lines();
    let header = lines.next().ok_or("CSV vazio")?;
    let col = header
        .split(',')
        .position(|h| matches!(h.trim(), "MsBetweenPresents" | "FrameTime"))
        .ok_or("coluna de frametime não encontrada no CSV do PresentMon")?;
    Ok(lines
        .filter_map(|l| l.split(',').nth(col)?.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .collect())
}

fn presentmon_path() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("NEXUS_PRESENTMON") {
        return Ok(PathBuf::from(p));
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.join("resources"));
        }
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.starts_with("presentmon") && n.ends_with(".exe") {
                    return Ok(e.path());
                }
            }
        }
    }
    Err("PresentMon não encontrado. Coloque PresentMon-*.exe em src-tauri/resources ou defina NEXUS_PRESENTMON.".into())
}

/// Mede o processo informado durante `seconds` segundos. Exige o jogo aberto e (normalmente) administrador.
pub fn run(process_name: &str, seconds: u32) -> Result<BenchResult, String> {
    let exe = presentmon_path()?;
    let out = std::env::temp_dir().join("nexus_presentmon.csv");
    let _ = std::fs::remove_file(&out);
    let status = Command::new(exe)
        .args([
            "--process_name",
            process_name,
            "--output_file",
            &out.to_string_lossy(),
            "--timed",
            &seconds.to_string(),
            "--terminate_after_timed",
            "--stop_existing_session",
            "--no_console_stats",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.exists() {
        // A saída do PresentMon vem em UTF-16: remove os NULs para ficar legível.
        let text = |b: &[u8]| String::from_utf8_lossy(b).replace('\0', "");
        let all = format!("{} {}", text(&status.stdout), text(&status.stderr));
        if all.to_lowercase().contains("access denied") {
            return Err("O PresentMon precisa de administrador. Feche o app e abra o Nexus Optimizer como administrador.".into());
        }
        return Err(format!(
            "PresentMon não gerou dados (o jogo está aberto e com esse nome de executável?): {}",
            all.trim()
        ));
    }
    let csv = std::fs::read_to_string(&out).map_err(|e| e.to_string())?;
    stats(&parse_csv(&csv)?).ok_or_else(|| "nenhum frame capturado — o jogo estava renderizando?".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_keeps_spikes() {
        let mut v = vec![10.0; 1000];
        v[503] = 90.0;
        let d = downsample(&v, 100);
        assert!(d.len() <= 100 && d.iter().cloned().fold(0.0, f32::max) == 90.0);
        assert_eq!(downsample(&[1.0, 2.0], 100).len(), 2);
    }

    #[test]
    fn constant_60fps() {
        let r = stats(&vec![1000.0 / 60.0; 600]).unwrap();
        assert!((r.avg_fps - 60.0).abs() < 1e-6);
        assert!((r.low_1pct_fps - 60.0).abs() < 1e-6);
    }

    #[test]
    fn stutter_lowers_1pct_low() {
        let mut v = vec![10.0; 990]; // 100 fps
        v.extend(vec![50.0; 10]); // 1% de frames a 20 fps
        let r = stats(&v).unwrap();
        assert!(r.avg_fps < 100.0 && r.avg_fps > 95.0);
        assert!((r.low_1pct_fps - 20.0).abs() < 1e-6);
        assert_eq!(r.max_frametime_ms, 50.0);
    }

    #[test]
    fn parses_csv_columns() {
        let csv = "Application,ProcessID,MsBetweenPresents,Other\ng.exe,1,16.6,x\ng.exe,1,bad,x\ng.exe,1,8.3,x\n";
        assert_eq!(parse_csv(csv).unwrap(), vec![16.6, 8.3]);
        assert!(parse_csv("a,b\n1,2\n").is_err());
        assert!(stats(&[]).is_none());
    }
}

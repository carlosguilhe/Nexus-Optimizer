use futures_util::StreamExt;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;

/// Servidores de download em ordem de preferência. Se um responder com erro ou 429 (limite de requisições),
/// o teste passa para o próximo sem martelar o servidor.
const DOWN_SOURCES: &[(&str, &str)] = &[
    ("Cloudflare", "https://speed.cloudflare.com/__down?bytes=25000000"),
    ("CacheFly", "https://cachefly.cachefly.net/100mb.test"),
];
const UP_URL: &str = "https://speed.cloudflare.com/__up";
const PING_HOST: &str = "speed.cloudflare.com:443";
const PING_SAMPLES: usize = 15;
const PHASE_SECS: f64 = 8.0;
const WARMUP_SECS: f64 = 1.5; // ignora o início lento (TCP slow start) no valor final
const STREAMS: usize = 4;
const CHUNK: usize = 64 * 1024;
const CHUNKS_PER_REQUEST: usize = 128; // 8 MB por requisição de upload (menos requisições = menos limite)

#[derive(Serialize, Clone)]
struct Progress {
    phase: &'static str,
    value: f64,
    progress: f64,
}

#[derive(Serialize)]
pub struct SpeedResult {
    pub ping_ms: f64,
    pub jitter_ms: f64,
    pub down_mbps: f64,
    pub up_mbps: f64,
    pub server: String,
}

type Emit<'a> = dyn Fn(&'static str, f64, f64) + Sync + 'a;

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("NexusOptimizer/0.1")
        .build()
        .map_err(|e| e.to_string())
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 { sorted[n / 2] } else { (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0 }
}

/// Latência de rede = tempo para abrir uma conexão TCP (SYN → SYN-ACK). Não inclui TLS nem HTTP,
/// por isso fica próxima do ping dos jogos. Usa a mediana; jitter = variação média entre amostras.
async fn measure_latency(emit: &Emit<'_>) -> Result<(f64, f64), String> {
    let addr = tokio::net::lookup_host(PING_HOST)
        .await
        .map_err(|e| format!("sem conexão com a internet (DNS): {e}"))?
        .next()
        .ok_or("não consegui resolver o servidor de teste")?;
    let mut times: Vec<f64> = Vec::new();
    for i in 0..PING_SAMPLES {
        let t = Instant::now();
        let ok = matches!(
            tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(addr)).await,
            Ok(Ok(_))
        );
        if ok {
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            let mut s = times.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            emit("ping", median(&s), (i + 1) as f64 / PING_SAMPLES as f64);
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    if times.len() < 3 {
        return Err("sem resposta do servidor de teste: verifique a conexão ou o firewall".into());
    }
    let jitter = times.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (times.len() - 1) as f64;
    let mut sorted = times;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Ok((median(&sorted), jitter))
}

/// Observa o contador de bytes por PHASE_SECS, emite a velocidade instantânea e devolve a média estável (Mbps).
async fn watch(emit: &Emit<'_>, phase: &'static str, bytes: &AtomicU64) -> f64 {
    let start = Instant::now();
    let (mut last_t, mut last_b) = (0.0f64, 0u64);
    let mut smooth = 0.0f64;
    let mut warm: Option<(f64, u64)> = None;
    loop {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let t = start.elapsed().as_secs_f64();
        let b = bytes.load(Ordering::Relaxed);
        let inst = (b - last_b) as f64 * 8.0 / 1e6 / (t - last_t).max(1e-3);
        smooth = if smooth == 0.0 { inst } else { smooth * 0.6 + inst * 0.4 };
        (last_t, last_b) = (t, b);
        if warm.is_none() && t >= WARMUP_SECS {
            warm = Some((t, b));
        }
        emit(phase, smooth, (t / PHASE_SECS).min(1.0));
        if t >= PHASE_SECS {
            return match warm {
                Some((wt, wb)) => (b - wb) as f64 * 8.0 / 1e6 / (t - wt).max(1e-3),
                None => 0.0,
            };
        }
    }
}

/// Espera progressiva após falhas: 0,8 s, 1,6 s, 3,2 s, ... até 6,4 s.
fn backoff(fails: u32) -> Duration {
    Duration::from_millis(400u64 << fails.min(4))
}

async fn run_phase(
    emit: &Emit<'_>,
    phase: &'static str,
    upload: bool,
    c: &reqwest::Client,
    source: &Arc<AtomicUsize>,
) -> Result<f64, String> {
    let bytes = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut tasks = Vec::new();
    for _ in 0..STREAMS {
        let (c, bytes, stop, source) = (c.clone(), bytes.clone(), stop.clone(), source.clone());
        tasks.push(tauri::async_runtime::spawn(async move {
            let mut fails = 0u32;
            while !stop.load(Ordering::Relaxed) {
                let idx = source.load(Ordering::Relaxed);
                let ok = if upload {
                    let counter = bytes.clone();
                    let body = futures_util::stream::iter((0..CHUNKS_PER_REQUEST).map(move |_| {
                        counter.fetch_add(CHUNK as u64, Ordering::Relaxed);
                        Ok::<_, std::io::Error>(vec![0u8; CHUNK])
                    }));
                    matches!(
                        c.post(UP_URL).body(reqwest::Body::wrap_stream(body)).send().await,
                        Ok(r) if r.status().is_success()
                    )
                } else {
                    let url = DOWN_SOURCES[idx % DOWN_SOURCES.len()].1;
                    match c.get(url).send().await {
                        // Só conta bytes de respostas 2xx: um 429 traz 1 byte de erro, não é velocidade.
                        Ok(resp) if resp.status().is_success() => {
                            let mut s = resp.bytes_stream();
                            while let Some(Ok(chunk)) = s.next().await {
                                bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                                if stop.load(Ordering::Relaxed) {
                                    break;
                                }
                            }
                            true
                        }
                        _ => {
                            // Servidor limitando ou fora do ar: passa ao próximo (uma vez, mesmo com 4 fluxos).
                            let _ = source.compare_exchange(idx, idx + 1, Ordering::Relaxed, Ordering::Relaxed);
                            false
                        }
                    }
                };
                if ok {
                    fails = 0;
                } else {
                    fails += 1;
                    tokio::time::sleep(backoff(fails)).await;
                }
            }
        }));
    }
    let mbps = watch(emit, phase, &bytes).await;
    stop.store(true, Ordering::Relaxed);
    for t in tasks {
        t.abort();
    }
    if bytes.load(Ordering::Relaxed) == 0 || mbps <= 0.0 {
        return Err(if upload {
            "upload indisponível: o servidor de teste está limitando as requisições. Tente de novo em alguns minutos.".into()
        } else {
            "download indisponível: os servidores de teste estão limitando as requisições ou sem conexão. Tente de novo em alguns minutos.".into()
        });
    }
    Ok(mbps)
}

async fn run_with(emit: &Emit<'_>) -> Result<SpeedResult, String> {
    let c = client()?;
    let source = Arc::new(AtomicUsize::new(0));
    let (ping_ms, jitter_ms) = measure_latency(emit).await?;
    let down_mbps = run_phase(emit, "download", false, &c, &source).await?;
    let server = DOWN_SOURCES[source.load(Ordering::Relaxed) % DOWN_SOURCES.len()].0.to_string();
    let up_mbps = run_phase(emit, "upload", true, &c, &source).await?;
    emit("done", 0.0, 1.0);
    Ok(SpeedResult { ping_ms, jitter_ms, down_mbps, up_mbps, server })
}

pub async fn run(app: AppHandle) -> Result<SpeedResult, String> {
    run_with(&|phase, value, progress| {
        let _ = app.emit("speedtest", Progress { phase, value, progress });
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_backoff() {
        assert_eq!(median(&[1.0, 2.0, 9.0]), 2.0);
        assert_eq!(median(&[1.0, 3.0]), 2.0);
        assert_eq!(backoff(1), Duration::from_millis(800));
        assert_eq!(backoff(99), Duration::from_millis(6400));
    }

    // Usa a internet de verdade (~30 s): rode com `cargo test real_speed_test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_speed_test() {
        let log = std::sync::Mutex::new(Vec::<(&'static str, f64)>::new());
        let r = tauri::async_runtime::block_on(run_with(&|p, v, _| log.lock().unwrap().push((p, v)))).unwrap();
        for (i, (p, v)) in log.lock().unwrap().iter().enumerate() {
            if i % 8 == 0 {
                println!("{p:9} {v:8.1}");
            }
        }
        println!(
            "FINAL ping {:.1} ms  jitter {:.1} ms  down {:.1} Mbps  up {:.1} Mbps  servidor {}",
            r.ping_ms, r.jitter_ms, r.down_mbps, r.up_mbps, r.server
        );
        assert!(r.ping_ms > 0.0 && r.down_mbps > 0.5 && r.up_mbps > 0.1);
    }
}

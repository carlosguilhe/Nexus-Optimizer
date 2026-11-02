use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::net::{TcpStream, UdpSocket};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize, Clone)]
pub struct Server {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub host: &'static str,
}

/// Endpoints reais que aceitam conexão na porta 443. Medem a rota até a região/serviço, não o servidor da partida.
pub const SERVERS: &[Server] = &[
    Server { id: "aws-sa", name: "São Paulo (AWS sa-east-1)", group: "Regiões da nuvem", host: "dynamodb.sa-east-1.amazonaws.com" },
    Server { id: "aws-use1", name: "Virgínia do Norte (AWS us-east-1)", group: "Regiões da nuvem", host: "dynamodb.us-east-1.amazonaws.com" },
    Server { id: "aws-use2", name: "Ohio (AWS us-east-2)", group: "Regiões da nuvem", host: "dynamodb.us-east-2.amazonaws.com" },
    Server { id: "aws-usw2", name: "Oregon (AWS us-west-2)", group: "Regiões da nuvem", host: "dynamodb.us-west-2.amazonaws.com" },
    Server { id: "aws-euw1", name: "Irlanda (AWS eu-west-1)", group: "Regiões da nuvem", host: "dynamodb.eu-west-1.amazonaws.com" },
    Server { id: "aws-euc1", name: "Frankfurt (AWS eu-central-1)", group: "Regiões da nuvem", host: "dynamodb.eu-central-1.amazonaws.com" },
    Server { id: "aws-apne1", name: "Tóquio (AWS ap-northeast-1)", group: "Regiões da nuvem", host: "dynamodb.ap-northeast-1.amazonaws.com" },
    Server { id: "riot-br", name: "Riot Games · Brasil", group: "Serviços de jogos", host: "br.api.riotgames.com" },
    Server { id: "riot-na", name: "Riot Games · América do Norte", group: "Serviços de jogos", host: "na.api.riotgames.com" },
    Server { id: "riot-euw", name: "Riot Games · Europa Oeste", group: "Serviços de jogos", host: "euw1.api.riotgames.com" },
    Server { id: "steam", name: "Steam / Valve", group: "Serviços de jogos", host: "api.steampowered.com" },
    Server { id: "epic", name: "Epic Games", group: "Serviços de jogos", host: "epicgames.com" },
    Server { id: "battlenet", name: "Battle.net", group: "Serviços de jogos", host: "battle.net" },
    Server { id: "ea", name: "EA", group: "Serviços de jogos", host: "www.ea.com" },
    Server { id: "ubisoft", name: "Ubisoft", group: "Serviços de jogos", host: "www.ubisoft.com" },
    Server { id: "xbox", name: "Xbox / Microsoft", group: "Serviços de jogos", host: "login.live.com" },
];

pub fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 64
        && !h.starts_with('-')
        && h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

/// Uma sondagem: tempo de abertura de conexão TCP (ms), ou None se falhou/estourou o tempo.
pub async fn tcp_ping(host: &str, port: u16) -> Option<f64> {
    if !valid_host(host) {
        return None;
    }
    let addr = tokio::net::lookup_host((host, port)).await.ok()?.next()?;
    let t = Instant::now();
    match tokio::time::timeout(Duration::from_millis(2000), TcpStream::connect(addr)).await {
        Ok(Ok(_)) => Some(t.elapsed().as_secs_f64() * 1000.0),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct ServerPing {
    pub id: &'static str,
    pub ms: Option<f64>,
    pub loss: u32,
}

/// Mediana de `n` sondagens para cada servidor, todos em paralelo.
pub async fn ping_servers(n: usize) -> Vec<ServerPing> {
    let tasks: Vec<_> = SERVERS
        .iter()
        .map(|s| {
            tauri::async_runtime::spawn(async move {
                let mut ok: Vec<f64> = Vec::new();
                for _ in 0..n {
                    if let Some(ms) = tcp_ping(s.host, 443).await {
                        ok.push(ms);
                    }
                    tokio::time::sleep(Duration::from_millis(80)).await;
                }
                ok.sort_by(|a, b| a.partial_cmp(b).unwrap());
                ServerPing { id: s.id, ms: ok.get(ok.len() / 2).copied(), loss: (n - ok.len()) as u32 }
            })
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(p) = t.await {
            out.push(p);
        }
    }
    out
}

// ------------------------------------------------------------------- DNS

pub fn dns_query(domain: &str, id: u16) -> Vec<u8> {
    let mut q = vec![(id >> 8) as u8, id as u8, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in domain.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]); // fim do nome, tipo A, classe IN
    q
}

/// Resposta válida = mesmo id, bit de resposta ligado e RCODE 0 (NOERROR).
pub fn dns_reply_ok(reply: &[u8], id: u16) -> bool {
    reply.len() >= 12 && reply[0] == (id >> 8) as u8 && reply[1] == id as u8 && reply[2] & 0x80 != 0 && reply[3] & 0x0F == 0
}

async fn resolve_time(server: &str, domain: &str, id: u16) -> Option<f64> {
    let sock = UdpSocket::bind("0.0.0.0:0").await.ok()?;
    sock.connect((server, 53)).await.ok()?;
    let t = Instant::now();
    sock.send(&dns_query(domain, id)).await.ok()?;
    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_millis(1500), sock.recv(&mut buf)).await.ok()?.ok()?;
    dns_reply_ok(&buf[..n], id).then(|| t.elapsed().as_secs_f64() * 1000.0)
}

#[derive(Serialize)]
pub struct DnsResult {
    pub name: String,
    pub ip: String,
    pub preset: Option<&'static str>,
    pub ms: Option<f64>,
    pub failures: u32,
}

const DOMAINS: &[&str] = &["google.com", "youtube.com", "cloudflare.com", "amazon.com", "wikipedia.org", "riotgames.com"];

/// Mede quanto cada servidor DNS demora para responder (mediana de 3 rodadas por domínio).
pub async fn dns_benchmark(system_ip: Option<String>) -> Vec<DnsResult> {
    let mut list: Vec<(String, String, Option<&'static str>)> = vec![
        ("Cloudflare".into(), "1.1.1.1".into(), Some("cloudflare")),
        ("Google".into(), "8.8.8.8".into(), Some("google")),
        ("Quad9".into(), "9.9.9.9".into(), Some("quad9")),
        ("OpenDNS".into(), "208.67.222.222".into(), Some("opendns")),
    ];
    if let Some(ip) = system_ip.filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok() && !list.iter().any(|l| &l.1 == ip)) {
        list.insert(0, ("Atual do sistema".into(), ip, None));
    }
    let tasks: Vec<_> = list
        .into_iter()
        .map(|(name, ip, preset)| {
            tauri::async_runtime::spawn(async move {
                let mut times = Vec::new();
                let mut failures = 0;
                for round in 0..3u16 {
                    for (i, d) in DOMAINS.iter().enumerate() {
                        match resolve_time(&ip, d, 0x4e00 + round * 16 + i as u16).await {
                            Some(ms) => times.push(ms),
                            None => failures += 1,
                        }
                    }
                }
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                DnsResult { name, ip, preset, ms: times.get(times.len() / 2).copied(), failures }
            })
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(r) = t.await {
            out.push(r);
        }
    }
    out.sort_by(|a, b| a.ms.unwrap_or(f64::MAX).partial_cmp(&b.ms.unwrap_or(f64::MAX)).unwrap());
    out
}

// ----------------------------------------------------------------- rota

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hop {
    pub hop: u32,
    pub ms: Option<f64>,
    pub ip: Option<String>,
}

/// Interpreta uma linha do `tracert` em qualquer idioma: número do salto, tempos ("<1 ms", "12 ms", "*") e IP final.
pub fn parse_hop(line: &str) -> Option<Hop> {
    let mut parts = line.split_whitespace();
    let hop: u32 = parts.next()?.parse().ok()?;
    let rest: Vec<&str> = parts.collect();
    let mut times = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let tok = rest[i];
        if tok == "*" {
            i += 1;
        } else if rest.get(i + 1).copied() == Some("ms") {
            if let Ok(v) = tok.trim_start_matches('<').parse::<f64>() {
                times.push(v);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let ip = rest.last().filter(|t| t.parse::<std::net::IpAddr>().is_ok()).map(|s| s.to_string());
    // Linha de salto sem tempo nem IP só vale se for um timeout (tem "*"); o resto é texto do tracert.
    if times.is_empty() && ip.is_none() && !rest.contains(&"*") {
        return None;
    }
    let ms = (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64);
    Some(Hop { hop, ms, ip })
}

/// Executa `tracert` e envia cada salto assim que aparece (evento "trace").
pub fn route_trace(app: &AppHandle, host: &str) -> Result<Vec<Hop>, String> {
    if !valid_host(host) {
        return Err("host inválido".into());
    }
    let mut child = Command::new("tracert")
        .args(["-d", "-h", "20", "-w", "1000", host])
        .stdout(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut hops = Vec::new();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).split(b'\n').flatten() {
            if let Some(h) = parse_hop(&String::from_utf8_lossy(&line)) {
                let _ = app.emit("trace", h.clone());
                hops.push(h);
            }
        }
    }
    let _ = child.wait();
    if hops.is_empty() {
        return Err("não foi possível traçar a rota (host inválido ou ICMP bloqueado)".into());
    }
    Ok(hops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_packet_is_well_formed() {
        let q = dns_query("google.com", 0x1234);
        assert_eq!(&q[..4], &[0x12, 0x34, 0x01, 0x00]);
        assert_eq!(&q[12..], &[6, b'g', b'o', b'o', b'g', b'l', b'e', 3, b'c', b'o', b'm', 0, 0, 1, 0, 1]);
        let mut reply = q.clone();
        reply[2] = 0x81;
        reply[3] = 0x80;
        assert!(dns_reply_ok(&reply, 0x1234));
        assert!(!dns_reply_ok(&reply, 0x9999), "id diferente");
        reply[3] = 0x83; // NXDOMAIN
        assert!(!dns_reply_ok(&reply, 0x1234));
    }

    #[test]
    fn tracert_lines_parse_in_portuguese_and_english() {
        let h = parse_hop("  2    <1 ms     2 ms     1 ms  192.168.0.1").unwrap();
        assert_eq!((h.hop, h.ip.as_deref()), (2, Some("192.168.0.1")));
        assert!((h.ms.unwrap() - (1.0 + 2.0 + 1.0) / 3.0).abs() < 1e-9);
        let t = parse_hop("  3     *        *        *     Esgotado o tempo limite do pedido.").unwrap();
        assert!(t.ms.is_none() && t.ip.is_none());
        assert!(parse_hop("Rastreando a rota para x").is_none());
        assert!(parse_hop("Trace complete.").is_none());
    }

    #[test]
    fn server_list_is_valid() {
        let mut ids: Vec<_> = SERVERS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), SERVERS.len());
        assert!(SERVERS.iter().all(|s| valid_host(s.host)));
    }

    // Usa a internet: `cargo test real_net -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_net() {
        tauri::async_runtime::block_on(async {
            for p in ping_servers(3).await {
                println!("{:12} {:?} perdas={}", p.id, p.ms.map(|m| m.round()), p.loss);
            }
            for d in dns_benchmark(None).await {
                println!("{:12} {:16} {:?} falhas={}", d.name, d.ip, d.ms.map(|m| (m * 10.0).round() / 10.0), d.failures);
            }
        });
    }
}

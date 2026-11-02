use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::shell::{is_newer, REPO};

const DETACHED_PROCESS: u32 = 0x0000_0008;
const MAX_INSTALLER_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Serialize, Clone)]
struct Progress {
    /// download | verify | install
    stage: &'static str,
    done: u64,
    total: u64,
}

/// Instalador e hash escolhidos entre os arquivos de uma release.
#[derive(Debug, PartialEq)]
struct Assets {
    installer_name: String,
    installer_url: String,
    hash_url: String,
}

fn release_prefix() -> String {
    format!("https://github.com/{REPO}/releases/download/")
}

/// Escolhe o instalador (`*_x64-setup.exe`) e o `.sha256` dele. Só aceita endereços de download da própria
/// release do repositório do Nexus: nenhum outro endereço vindo da resposta é usado.
fn pick_assets(release: &serde_json::Value) -> Result<Assets, String> {
    let list = release["assets"].as_array().ok_or("a release não tem arquivos")?;
    let url_of = |name: &str| -> Option<String> {
        list.iter()
            .find(|a| a["name"].as_str() == Some(name))
            .and_then(|a| a["browser_download_url"].as_str())
            .filter(|u| u.starts_with(&release_prefix()))
            .map(str::to_string)
    };
    let installer_name = list
        .iter()
        .filter_map(|a| a["name"].as_str())
        .find(|n| n.ends_with("_x64-setup.exe") && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ')))
        .ok_or("a release não tem o instalador do Windows")?
        .to_string();
    let installer_url = url_of(&installer_name).ok_or("endereço do instalador não reconhecido")?;
    let hash_url = url_of(&format!("{installer_name}.sha256")).ok_or("a release não tem o arquivo de hash (.sha256); atualização recusada")?;
    Ok(Assets { installer_name, installer_url, hash_url })
}

/// Primeiro valor de 64 dígitos hexadecimais de um arquivo `.sha256` ("hash  nome" ou só o hash).
fn parse_hash(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|t| t.trim_start_matches('*').to_lowercase())
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
}

fn emit(app: &AppHandle, stage: &'static str, done: u64, total: u64) {
    let _ = app.emit("update_progress", Progress { stage, done, total });
}

/// Baixa o instalador da última release, confere o SHA-256 e o executa; o app fecha em seguida.
pub async fn install(app: AppHandle) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent("NexusOptimizer")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("sem acesso ao GitHub: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("nenhuma versão publicada (HTTP {})", resp.status()));
    }
    let release: serde_json::Value = serde_json::from_slice(&resp.bytes().await.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let tag = release["tag_name"].as_str().ok_or("resposta inesperada do GitHub")?.to_string();
    if !is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        return Err("O Nexus já está na versão mais recente.".into());
    }
    let assets = pick_assets(&release)?;

    let expected = {
        let text = client
            .get(&assets.hash_url)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("não consegui baixar o hash: {e}"))?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        parse_hash(&text).ok_or("arquivo de hash inválido; atualização recusada")?
    };

    let dir = std::env::temp_dir().join("NexusUpdate");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&assets.installer_name);

    let resp = client
        .get(&assets.installer_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("falha ao baixar a atualização: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    if total > MAX_INSTALLER_BYTES {
        return Err("o instalador é maior do que o esperado; atualização recusada".into());
    }
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let (mut hasher, mut done) = (Sha256::new(), 0u64);
    let mut stream = resp.bytes_stream();
    let mut last_emit = std::time::Instant::now();
    emit(&app, "download", 0, total);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("a conexão caiu durante o download: {e}"))?;
        done += chunk.len() as u64;
        if done > MAX_INSTALLER_BYTES {
            return Err("o instalador é maior do que o esperado; atualização recusada".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        if last_emit.elapsed() > Duration::from_millis(120) {
            emit(&app, "download", done, total);
            last_emit = std::time::Instant::now();
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    emit(&app, "verify", done, done);
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        let _ = std::fs::remove_file(&path);
        return Err("O arquivo baixado não confere com o hash publicado (SHA-256). Atualização cancelada por segurança.".into());
    }

    emit(&app, "install", done, done);
    crate::activity::log("system", format!("Atualizando o Nexus para {tag}"));
    // /P = instalação passiva (só a barra de progresso), /R = reabre o app ao terminar.
    Command::new(&path)
        .args(["/P", "/R"])
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("não consegui iniciar o instalador: {e}"))?;
    // O instalador precisa do app fechado para trocar os arquivos.
    tokio::time::sleep(Duration::from_millis(600)).await;
    app.exit(0);
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(extra: serde_json::Value) -> serde_json::Value {
        let base = format!("https://github.com/{REPO}/releases/download/v9.9.9/");
        let mut assets = vec![
            json!({"name": "Nexus-Optimizer_9.9.9_x64-setup.exe", "browser_download_url": format!("{base}Nexus-Optimizer_9.9.9_x64-setup.exe")}),
            json!({"name": "Nexus-Optimizer_9.9.9_x64-setup.exe.sha256", "browser_download_url": format!("{base}Nexus-Optimizer_9.9.9_x64-setup.exe.sha256")}),
        ];
        if let Some(a) = extra.as_array() {
            assets.extend(a.clone());
        }
        json!({"tag_name": "v9.9.9", "assets": assets})
    }

    #[test]
    fn picks_installer_and_hash() {
        let a = pick_assets(&release(json!([]))).unwrap();
        assert_eq!(a.installer_name, "Nexus-Optimizer_9.9.9_x64-setup.exe");
        assert!(a.installer_url.starts_with(&release_prefix()) && a.hash_url.ends_with(".sha256"));
    }

    #[test]
    fn refuses_foreign_urls_and_missing_hash() {
        let foreign = json!({"assets": [
            {"name": "x_x64-setup.exe", "browser_download_url": "https://evil.example/x_x64-setup.exe"},
            {"name": "x_x64-setup.exe.sha256", "browser_download_url": "https://evil.example/x.sha256"},
        ]});
        assert!(pick_assets(&foreign).is_err());
        let no_hash = json!({"assets": [{"name": "a_x64-setup.exe", "browser_download_url": format!("{}v1/a_x64-setup.exe", release_prefix())}]});
        assert!(pick_assets(&no_hash).unwrap_err().contains("hash"));
        assert!(pick_assets(&json!({})).is_err());
    }

    #[test]
    fn parses_sha256_files() {
        let h = "720b8632ffd2426c99784a82b85cb2c373eb6face98443884596dcfb9d61940b";
        assert_eq!(parse_hash(&format!("{h}  Nexus-Optimizer_1.0.0_x64-setup.exe\n")).as_deref(), Some(h));
        assert_eq!(parse_hash(&h.to_uppercase()).as_deref(), Some(h));
        assert_eq!(parse_hash(&format!("{h} *setup.exe")).as_deref(), Some(h));
        assert!(parse_hash("não é hash").is_none());
        assert!(parse_hash(&h[..60]).is_none());
    }

    #[test]
    fn sha256_matches_known_value() {
        assert_eq!(format!("{:x}", Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}

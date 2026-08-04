//! Issue Eventide offline licenses (Ed25519 JWT).

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use eventide_license::{issue_license, verify_license, LicenseFile, LicenseRequest};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "eventide-license", about = "Eventide 产品许可证签发工具")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 根据客户导出的申请文件签发授权文件（推荐）
    Issue {
        /// 客户授权申请文件（.eventide-req.json）
        #[arg(long)]
        request: PathBuf,
        /// 有效天数
        #[arg(long, default_value_t = 365)]
        days: i64,
        /// 版本标记（默认 standard）
        #[arg(long, default_value = "standard")]
        edition: String,
        /// 覆盖申请文件中的客户名称
        #[arg(long)]
        sub: Option<String>,
        /// 输出授权文件路径（默认 stdout；建议 .eventide-lic.json）
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// 私钥 PEM 路径；也可用环境变量 EVENTIDE_LICENSE_PRIVATE_KEY
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// 手动签发（不推荐；请优先用 --request）
    IssueManual {
        #[arg(long)]
        sub: String,
        #[arg(long, default_value_t = 365)]
        days: i64,
        #[arg(long, default_value = "standard")]
        edition: String,
        #[arg(long)]
        install_id: Option<String>,
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// 校验授权文件或 JWT
    Verify {
        /// 授权文件路径或 JWT 字符串
        token: String,
        #[arg(long)]
        install_id: Option<String>,
    },
    /// 打印内置公钥 PEM
    PublicKey,
}

fn load_private_pem(key: Option<&PathBuf>) -> anyhow::Result<Vec<u8>> {
    if let Ok(pem) = std::env::var("EVENTIDE_LICENSE_PRIVATE_KEY") {
        let t = pem.trim();
        if !t.is_empty() {
            return Ok(t.as_bytes().to_vec());
        }
    }
    let path = key
        .cloned()
        .or_else(|| Some(PathBuf::from("keys/license_private.pem")))
        .unwrap();
    std::fs::read(&path).with_context(|| format!("读取私钥 {}", path.display()))
}

fn write_out(output: Option<&PathBuf>, content: &str) -> anyhow::Result<()> {
    if let Some(path) = output {
        std::fs::write(path, content).with_context(|| format!("写入 {}", path.display()))?;
        eprintln!("已写入 {}", path.display());
    } else {
        print!("{content}");
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Issue {
            request,
            days,
            edition,
            sub,
            output,
            key,
        } => {
            let raw = std::fs::read_to_string(&request)
                .with_context(|| format!("读取申请文件 {}", request.display()))?;
            let req = LicenseRequest::parse_json(&raw)?;
            let customer = sub
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or(req.customer.as_deref().map(str::trim).filter(|s| !s.is_empty()))
                .unwrap_or("");
            if customer.is_empty() {
                bail!("客户名称为空：请在申请文件填写 customer，或使用 --sub");
            }
            let pem = load_private_pem(key.as_ref())?;
            let jwt = issue_license(
                &pem,
                customer,
                days,
                &edition,
                Some(req.install_id.trim()),
            )?;
            let claims = verify_license(&jwt, Some(req.install_id.trim()), true)?;
            let file = LicenseFile::from_claims(jwt, &claims);
            let json = serde_json::to_string_pretty(&file)?;
            write_out(output.as_ref(), &format!("{json}\n"))?;
        }
        Cmd::IssueManual {
            sub,
            days,
            edition,
            install_id,
            output,
            key,
        } => {
            if sub.trim().is_empty() {
                bail!("--sub 不能为空");
            }
            let pem = load_private_pem(key.as_ref())?;
            let jwt = issue_license(
                &pem,
                &sub,
                days,
                &edition,
                install_id.as_deref(),
            )?;
            let claims = verify_license(&jwt, install_id.as_deref(), true)?;
            let file = LicenseFile::from_claims(jwt, &claims);
            let json = serde_json::to_string_pretty(&file)?;
            write_out(output.as_ref(), &format!("{json}\n"))?;
        }
        Cmd::Verify { token, install_id } => {
            let raw = if std::path::Path::new(&token).is_file() {
                std::fs::read_to_string(&token)?
            } else {
                token
            };
            let file = LicenseFile::parse_import(&raw)?;
            let claims =
                verify_license(&file.token, install_id.as_deref(), true)?;
            println!(
                "ok customer={} edition={} exp={} install_id={:?}",
                claims.sub,
                claims.edition,
                claims.expires_at().to_rfc3339(),
                claims.install_id
            );
        }
        Cmd::PublicKey => {
            print!("{}", eventide_license::PUBLIC_KEY_PEM);
        }
    }
    Ok(())
}

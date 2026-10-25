//! 更新包 ed25519 签名工具（发布流水线用，Windows/Linux 通用）。
//!
//! 用法：
//! - `cargo run --example asset_signer -- genkey installer/signing.key`
//!   首次生成签名密钥（32 字节种子 hex；**私钥不入库、离线备份**），
//!   同时打印公钥——公钥 hex 需同步写入 updater.rs 的 UPDATE_PUBKEY
//! - `cargo run --example asset_signer -- sign <文件> installer/signing.key`
//!   生成 `<文件>.sig`（64 字节签名 hex），发布时作为附件与安装包一起上传
#![allow(clippy::print_stderr)]

use ed25519_dalek::{Signer, SigningKey};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("genkey") => genkey(
            args.get(2).expect("用法: asset_signer genkey <keyfile>"),
        ),
        Some("sign") => sign(
            args.get(2).expect("用法: asset_signer sign <文件> <keyfile>"),
            args.get(3).expect("用法: asset_signer sign <文件> <keyfile>"),
        ),
        _ => {
            eprintln!("用法: asset_signer genkey <keyfile> | sign <文件> <keyfile>");
            std::process::exit(1);
        }
    }
}

/// 生成密钥：写 32 字节种子 hex 到 keyfile，stdout 打印公钥 hex
fn genkey(keyfile: &str) {
    use rand::RngCore;
    let mut seed = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    std::fs::write(keyfile, hex::encode(seed))
        .unwrap_or_else(|e| panic!("写入密钥文件失败: {e}"));
    println!("公钥（写入 updater.rs 的 UPDATE_PUBKEY）:");
    println!("{}", hex::encode(sk.verifying_key().to_bytes()));
}

/// 签名文件：<文件>.sig（64 字节签名 hex）
fn sign(file: &str, keyfile: &str) {
    let seed_hex = std::fs::read_to_string(keyfile)
        .unwrap_or_else(|e| panic!("读取密钥文件失败: {e}"))
        .trim()
        .to_string();
    let mut seed = [0u8; 32];
    hex::decode_to_slice(&seed_hex, &mut seed).expect("密钥文件格式错误（应为 64 位 hex）");
    let sk = SigningKey::from_bytes(&seed);
    let data = std::fs::read(file).unwrap_or_else(|e| panic!("读取待签名文件失败: {e}"));
    let sig = sk.sign(&data);
    let sig_path = format!("{file}.sig");
    std::fs::write(&sig_path, hex::encode(sig.to_bytes()))
        .unwrap_or_else(|e| panic!("写入签名失败: {e}"));
    println!("已生成签名: {sig_path}");
}

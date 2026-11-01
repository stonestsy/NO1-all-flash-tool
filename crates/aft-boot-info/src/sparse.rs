//! 逆向重建占位文件：`crates\aft-boot-info\src\sparse.rs`
//!
//! 证据来源：脱壳镜像 `All-FlashTool-v5.1.0.unpacked.exe` 中的 Rust panic 路径字符串。
//! 本文件为“可编译骨架”，真实函数签名与逻辑需结合反编译结果补全。
#![allow(dead_code, unused_imports)]

/// 占位函数（真实签名未知）。
pub fn placeholder() {}

/// 占位类型（供后续替换为真实结构体）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stub;

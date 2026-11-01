//! `aft-boot-info` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod ext4_prop;
pub mod lz4_prop;
pub mod offset_cache;
pub mod parser;
pub mod sparse;
pub mod super_parse;

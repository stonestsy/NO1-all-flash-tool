//! `hacc` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod common;
pub mod da;
pub mod preloader;

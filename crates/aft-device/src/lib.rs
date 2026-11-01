//! `aft-device` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod adb_files;
pub mod adb_server;
pub mod apk_install;
pub mod fastboot_batch;
pub mod fastboot_ops;
pub mod fdl;
pub mod lib_backup;
pub mod oneplus;
pub mod partition_ops;
pub mod reboot_ops;
pub mod usb_link;

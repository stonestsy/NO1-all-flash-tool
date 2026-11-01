//! `aft-edl` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod boot_slot;
pub mod cloud_da;
pub mod cloud_device_op;
pub mod cloud_loaders;
pub mod device_version;
pub mod lib_storage_info;
pub mod oplus_ocdt;
pub mod peek_super;
pub mod rawprogram_dialog;
pub mod session;
pub mod tools;
pub mod ufs_des_edl;
pub mod xml_parse;

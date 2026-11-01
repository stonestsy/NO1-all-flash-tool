//! `aft-storage` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod apk_cache;
pub mod cloud_proxy;
pub mod config_pack;
pub mod driver_download;
pub mod driver_install;
pub mod image_upload;
pub mod lk_runtime_pack;
pub mod loader_download;
pub mod pack_download_source;
pub mod seven_zip;
pub mod stream_download;
pub mod webdav_upload;

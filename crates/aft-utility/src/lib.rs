//! `aft-utility` —— 逆向重建骨架
//!
//! 该 crate 的存在与职责依据脱壳镜像中的模块路径、错误文案与接口字符串推断。
//! 当前为可编译占位实现（std-only）。

#![allow(dead_code, unused_imports)]

pub mod adb_lib_info;
pub mod android_relock;
pub mod da_patch;
pub mod lenovo_sn;
pub mod lenovo_unlock;
pub mod lk_patch;
pub mod lk_sign_unlock;
pub mod lun_extract;
pub mod modem_acc;
pub mod mtk_scatter_gpt;
pub mod nv_repair;
pub mod ocdt;
pub mod oneplus;
pub mod oplus_custom;
pub mod persist_acc;
pub mod ufs_des;
pub mod vab_sign;
pub mod xiaomi;
pub mod xiaomi_gen3_nv;
pub mod xiaomi_qc_gpt;
pub mod xiaomi_unlock;

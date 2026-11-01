# crate 职责映射（依据字符串与模块路径推断）

| crate | 推断职责 | 关键证据 |
|---|---|---|
| aft-auth | 授权/卡密/打包/传输 | pack.rs, pack_advanced.rs, transport.rs, lk_relay.rs；siyanbaozi/xiriacg 签名接口 |
| aft-security | 安全校验 | windows_guard.rs；安全中心/权限相关包名 |
| aft-crypto | 加密原语 | AES-GCM 认证错误 iv/ts/tag/nonce |
| aft-tls | TLS 配置 | 证书与 https 端点 |
| aft-edl | 高通 EDL/9008 | cloud_da.rs, cloud_device_op.rs, ufs_des_edl.rs, oplus_ocdt.rs |
| aft-device | ADB/Fastboot/USB | adb_server.rs, fastboot_ops.rs, usb_link.rs, fdl.rs |
| aft-rom | ROM/OTA/解锁 | miui_ota.rs, miui_root.rs, lenovo.rs, xiaomi.rs(由 utility) |
| aft-image | 镜像解析/解包 | payload.rs, sparse.rs(由 boot-info), super_parse.rs |
| aft-boot-info | boot/super/sparse 信息 | parser.rs, sparse.rs, super_parse.rs |
| aft-avb | AVB 签名/校验 | format.rs, tool.rs |
| aft-super-merge | super 合并 | disk.rs, native.rs, ofp_super.rs |
| aft-ofp-decrypt | OPPO OFP 解密 | crypto.rs, mtk.rs, qc.rs |
| aft-pan123 | 123 云盘 | lib.rs；123pan 接口簇 |
| aft-qcn | 基带 QCN | engine/*, imei.rs, diag.rs |
| aft-jailbreak | ROOT/越狱 | engine.rs, kernels.rs, payload.rs |
| aft-env-hide | 环境隐藏 | modules.rs, runner.rs, webdav.rs |
| aft-root-patch | ROOT 补丁 | patch/{magisk,ksu,apatch,apk,flash}.rs |
| aft-storage | 下载/驱动/缓存 | driver_download.rs, seven_zip.rs, webdav_upload.rs |
| aft-store | 应用商店/壁纸 | list.rs, install.rs, wallpaper.rs |
| aft-download | 下载队列 | manager.rs, queue.rs |
| aft-update | 版本更新 | lib.rs |
| aft-time-sync | 时间同步 | ntp.aliyun.com / ntp.tencent.com / cn.ntp.org.cn |
| aft-utility | 高级工具集合 | lenovo_*, xiaomi_*, ocdt.rs, ufs_des.rs, nv_repair.rs, mtk_scatter_gpt.rs |
| aft-mtk-config | MTK 配置回读 | readback.rs；aft-mtk-readback.json |
| hacc | 高通 DA/preloader | common/gfh/file.rs, da.rs, preloader/pl.rs |
| penumbra-mtk | MTK DA 协议 | auth/sla.rs, da/xflash, da/xml |
| wayget-core | 下载核心 | download.rs |

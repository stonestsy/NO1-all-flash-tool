# Tauri 命令签名（从前端反混淆代码恢复）

来源：`_work/deob/deobfuscated.js` 的命令封装对象；参数为前端发送的 args 键。

| 命令 | 前端方法 | 参数 |
|---|---|---|
| `app_version` | appVersion | — |
| `force_english_input` | forceEnglishInput | — |
| `restore_input_method` | restoreInputMethod | — |
| `health_check` | healthCheck | — |
| `get_login_env_meta` | getLoginEnvMeta | — |
| `login_bootstrap` | loginBootstrap | forceReconnect, skipTimeCalibrate |
| `calibrate_system_time_cmd` | calibrateSystemTime | — |
| `refresh_auth` | refreshAuth | — |
| `load_login_credentials` | loadLoginCredentials | — |
| `save_login_credentials` | saveLoginCredentials | password, username |
| `clear_login_credentials` | clearLoginCredentials | — |
| `load_login_wallpaper_cache` | loadLoginWallpaperCache | — |
| `sync_login_wallpaper` | syncLoginWallpaper | downloadTicket, exists, kind, updatedAt, url, username |
| `refresh_login_wallpaper` | refreshLoginWallpaper | username |
| `refresh_login_wallpaper_with_creds` | refreshLoginWallpaperWithCreds | loginKey, password, username |
| `login` | login | password, username |
| `login_by_key` | loginByKey | loginKey |
| `one_click_call` | oneClickCall | — |
| `logout` | logout | — |
| `app_shutdown_cleanup` | appShutdownCleanup | — |
| `app_exit` | appExit | — |
| `verify_pack_access` | verifyPackAccess | — |
| `config_pack_check` | configPackCheck | — |
| `refresh_tool_paths_cmd` | refreshToolPaths | — |
| `set_pack_download_source` | setPackDownloadSource | source |
| `get_pack_download_source` | getPackDownloadSource | — |
| `probe_pack_download_lines` | probePackDownloadLines | — |
| `cloud_gate` | cloudGate | op, path |
| `config_pack_sync` | configPackSync | — |
| `config_pack_cancel` | configPackCancel | — |
| `check_version_prelogin` | checkVersionPrelogin | — |
| `check_update` | checkUpdate | — |
| `download_app_update` | downloadAppUpdate | expectedSha256, url |
| `apply_app_update` | applyAppUpdate | installerPath |
| `server_status` | serverStatus | — |
| `server_offline_age_secs` | serverOfflineAgeSecs | — |
| `list_devices` | listDevices | — |
| `list_fastboot_devices` | listFastbootDevices | — |
| `adb_reboot` | adbReboot | mode, serial |
| `adb_device_info` | adbDeviceInfo | serial |
| `security_login_anomaly` | securityLoginAnomaly | — |
| `adb_read_device_detail` | adbReadDeviceDetail | serial |
| `adb_partitions` | adbPartitions | serial |
| `adb_erase_partitions` | adbErasePartitions | partitions, serial |
| `adb_readback_partitions` | adbReadbackPartitions | partitions, saveDir, serial |
| `adb_write_partitions` | adbWritePartitions | items, serial |
| `adb_qc_backup` | adbQcBackup | saveDir, serial |
| `adb_mtk_backup` | adbMtkBackup | saveDir, serial |
| `adb_install` | adbInstall | apk, serial |
| `adb_push` | adbPush | local, remote, serial |
| `adb_list_files` | adbListFiles | path, serial |
| `adb_pull_files` | adbPullFiles | localDir, remotePaths, serial |
| `adb_push_files` | adbPushFiles | localPaths, remoteDir, serial |
| `adb_drag_files_out` | adbDragFilesOut | remotePaths, serial |
| `mirror_enable` | mirrorEnable | — |
| `mirror_disable` | mirrorDisable | — |
| `mirror_keyevent` | mirrorKeyevent | code, serial |
| `love_danmaku_show` | loveDanmakuShow | — |
| `love_danmaku_consume` | loveDanmakuConsume | — |
| `love_danmaku_overlay_show` | loveDanmakuOverlayShow | — |
| `love_danmaku_finished` | loveDanmakuFinished | — |
| `love_danmaku_prepare` | loveDanmakuPrepare | — |
| `fastboot_reboot` | fastbootReboot | mode, serial |
| `fastboot_device_info_cmd` | fastbootDeviceInfo | serial |
| `fastboot_read_device_info_cmd` | fastbootReadDeviceInfo | serial |
| `fastboot_partitions_cmd` | fastbootPartitions | serial |
| `fastboot_vars` | fastbootVars | serial |
| `fastboot_flash` | fastbootFlash | image, partition, serial |
| `fastboot_erase` | fastbootErase | partition, serial |
| `fastboot_set_active` | fastbootSetActive | serial, slot |
| `fastboot_parse_script` | fastbootParseScript | scriptPath |
| `fastboot_parse_scripts` | fastbootParseScripts | scriptPaths |
| `fastboot_browse_scripts` | fastbootBrowseScripts | defaultDir |
| `fastboot_verify_model_cmd` | fastbootVerifyModel | args, serial |
| `fastboot_batch_write` | fastbootBatchWrite | options, serial, tasks |
| `fastboot_cancel` | fastbootCancel | — |
| `fastboot_clear_cancel` | fastbootClearCancel | — |
| `oneplus_scan_images` | oneplusScanImages | directory |
| `oneplus_fix_ftd_cmd` | oneplusFixFtd | partitions, serial |
| `oneplus_write_partitions_cmd` | oneplusWritePartitions | options, partitions, selected, serial |
| `mtk_connect` | mtkConnect | authPath, daPath, preloaderPath |
| `mtk_inspect_config` | mtkInspectConfig | path |
| `mtk_submit_signature` | mtkSubmitSignature | signature |
| `mtk_request_signature` | mtkRequestSignature | blob, kami |
| `mtk_cancel_auth` | mtkCancelAuth | — |
| `mtk_read_partition_table` | mtkReadPartitionTable | — |
| `mtk_peek_super` | mtkPeekSuper | — |
| `mtk_flash_package` | mtkFlashPackage | mode, scatterPath |
| `mtk_reboot` | mtkReboot | — |
| `mtk_read_partition` | mtkReadPartition | outputPath, partition |
| `mtk_write_partition` | mtkWritePartition | inputPath, partition |
| `mtk_erase_partition` | mtkErasePartition | partition |
| `mtk_disconnect` | mtkDisconnect | — |
| `cloud_driver_run` | cloudDriverRun | kind |
| `cloud_driver_download` | cloudDriverDownload | kind |
| `cloud_driver_install` | cloudDriverInstall | sessionId |
| `cloud_driver_dismiss` | cloudDriverDismiss | sessionId |
| `cloud_driver_cancel` | cloudDriverCancel | — |
| `env_hide_detect` | envHideDetect | serial |
| `env_hide_list_apks` | envHideListApks | scheme |
| `env_hide_run` | envHideRun | scheme, selectedApks, serial |
| `env_hide_cancel` | envHideCancel | — |
| `image_upload_run` | imageUploadRun | filePath, partition |
| `image_upload_cancel` | imageUploadCancel | — |
| `root_patch_preflight` | rootPatchPreflight | — |
| `root_patch_run` | rootPatchRun | flashType, localApk, localImage, otaUrl, partition, scheme, useCloud, useOplusRom |
| `root_patch_cancel` | rootPatchCancel | — |
| `jailbreak_detect` | jailbreakDetect | — |
| `jailbreak_offsets_summary` | jailbreakOffsetsSummary | — |
| `jailbreak_run` | jailbreakRun | customSo, otaUrl |
| `jailbreak_import_url` | jailbreakImportUrl | url |
| `jailbreak_parse_boot` | jailbreakParseBoot | bootPath, xblPath |
| `jailbreak_import_offsets` | jailbreakImportOffsets | path |
| `jailbreak_export_offsets` | jailbreakExportOffsets | path |
| `jailbreak_cancel` | jailbreakCancel | — |
| `jailbreak_deploy_ksu` | jailbreakDeployKsu | — |
| `device_op_redeem` | deviceOpRedeem | op |
| `smart_reboot` | smartReboot | target |
| `smart_set_active` | smartSetActive | slot |
| `fdl_authorize` | fdlAuthorize | kami |
| `open_device_manager` | openDeviceManager | — |
| `open_url` | openUrl | url |
| `open_official_site` | openOfficialSite | — |
| `open_cmd` | openCmd | — |
| `payload_validate_url` | payloadValidateUrl | url |
| `payload_list_partitions` | payloadListPartitions | source |
| `payload_parse_metadata` | payloadParseMetadata | source |
| `payload_extract` | payloadExtract | args |
| `payload_default_output_dir` | payloadDefaultOutputDir | — |
| `edl_list_cached_loaders` | edlListCachedLoaders | — |
| `edl_list_local_loaders` | edlListLocalLoaders | — |
| `mtk_list_cloud_da` | mtkListCloudDa | — |
| `mtk_ensure_cloud_da` | mtkEnsureCloudDa | brand, name |
| `mtk_ensure_cloud_auth` | mtkEnsureCloudAuth | — |
| `edl_ensure_cloud_loader` | edlEnsureCloudLoader | brand, name, op |
| `edl_resolve_oplus_loader` | edlResolveOplusLoader | loaderPath |
| `edl_list_schemes` | edlListSchemes | — |
| `edl_list_ports` | edlListPorts | — |
| `edl_session_status` | edlSessionStatus | — |
| `edl_connect` | edlConnect | port |
| `edl_disconnect` | edlDisconnect | — |
| `edl_identify_sahara` | edlIdentifySahara | scheme |
| `edl_read_gpt` | edlReadGpt | generateXml, loaderPath, memory, outputDir, scheme |
| `edl_peek_super_info` | edlPeekSuperInfo | loaderPath, memory, scheme |
| `edl_list_rawprogram_in_dir` | edlListRawprogramInDir | dir |
| `edl_browse_patch_xml` | edlBrowsePatchXml | defaultDir |
| `edl_browse_raw_xml` | edlBrowseRawXml | defaultDir |
| `edl_pick_rawprogram_files` | edlPickRawprogramFiles | defaultDir |
| `edl_normalize_rawprogram` | edlNormalizeRawprogram | paths |
| `edl_parse_rawprogram` | edlParseRawprogram | paths, sectorSize |
| `edl_will_set_boot_partition_after_xml` | edlWillSetBootPartitionAfterXml | paths |
| `edl_read_partitions` | edlReadPartitions | generateXml, loaderPath, memory, outputDir, scheme, tasks |
| `edl_xiaomi_oneclick_acc` | edlXiaomiOneclickAcc | loaderPath, memory, scheme |
| `edl_oplus_ocdt_verify` | edlOplusOcdtVerify | loaderPath, memory, rawXmlPaths, scheme |
| `edl_write_partitions` | edlWritePartitions | loaderPath, memory, protectLun5, rawXmlPaths, scheme, tasks |
| `edl_flash_raw_xml_package` | edlFlashRawXmlPackage | loaderPath, memory, protectLun5, rawXmlPaths, scheme, tasks |
| `edl_erase_partitions` | edlErasePartitions | eraseMode, loaderPath, memory, protectLun5, scheme, tasks |
| `edl_reboot` | edlReboot | mode |
| `edl_run_tool` | edlRunTool | action, loaderPath, lun, memory, outputDir, patchPaths, scheme |
| `edl_cancel` | edlCancel | — |
| `rom_load_scheme` | romLoadScheme | forceRefresh, scheme |
| `rom_load_firmware_link` | romLoadFirmwareLink | brand, file, name, version |
| `rom_load_server_device` | romLoadServerDevice | brand, file, name |
| `rom_list_xiaoyue_brands` | romListXiaoyueBrands | — |
| `rom_load_xiaoyue_devices` | romLoadXiaoyueDevices | brand |
| `rom_load_xiaoyue_pro_devices` | romLoadXiaoyueProDevices | brand, proKind |
| `rom_load_xiaoyue_majors` | romLoadXiaoyueMajors | brand, device |
| `rom_load_xiaoyue_versions` | romLoadXiaoyueVersions | brand, device, major |
| `rom_load_xiaomi_versions` | romLoadXiaomiVersions | flashType, romPageUrl |
| `rom_resolve_xiaomi_link` | romResolveXiaomiLink | flashType, pageUrl |
| `rom_resolve_xiaoyue_link` | romResolveXiaoyueLink | meta |
| `rom_resolve_redirect` | romResolveRedirect | url |
| `store_list_catalog` | storeListCatalog | refresh |
| `store_list_categories` | storeListCategories | refresh |
| `store_list_apps` | storeListApps | category, refresh |
| `store_default_download_dir` | storeDefaultDownloadDir | — |
| `store_download_quota_status` | storeDownloadQuotaStatus | — |
| `store_start_download_install` | storeStartDownloadInstall | displayName, downloadDir, fileName, remotePath, serial |
| `store_cancel_download` | storeCancelDownload | — |
| `pan123_restore` | pan123Restore | — |
| `pan123_status` | pan123Status | — |
| `pan123_login` | pan123Login | passport, password |
| `pan123_send_vcode` | pan123SendVcode | passport, traceless |
| `pan123_login_sms` | pan123LoginSms | passport, timeStamp, vcode |
| `pan123_qr_start` | pan123QrStart | — |
| `pan123_qr_poll` | pan123QrPoll | uniId |
| `pan123_cloud_available` | pan123CloudAvailable | — |
| `pan123_cloud_login` | pan123CloudLogin | — |
| `pan123_logout` | pan123Logout | — |
| `pan123_list_files` | pan123ListFiles | parentFileId |
| `pan123_download_url` | pan123DownloadUrl | item |
| `pan123_create_share` | pan123CreateShare | fileIds, shareName, sharePwd |
| `pan123_save_share` | pan123SaveShare | shareInput, sharePwd, targetParentId |
| `pan123_download_file` | pan123DownloadFile | item, savePath, skipSpaceCheck |
| `pan123_check_disk_space` | pan123CheckDiskSpace | needBytes, path |
| `pan123_pause_download` | pan123PauseDownload | — |
| `pan123_resume_download` | pan123ResumeDownload | — |
| `pan123_cancel_download` | pan123CancelDownload | — |
| `downloads_list` | downloadsList | — |
| `downloads_default_dir` | downloadsDefaultDir | — |
| `downloads_sweep_orphans` | downloadsSweepOrphans | — |
| `downloads_get_max_active` | downloadsGetMaxActive | — |
| `downloads_set_max_active` | downloadsSetMaxActive | maxActive |
| `downloads_add` | downloadsAdd | connections, filename, folder, url |
| `downloads_pause` | downloadsPause | id |
| `downloads_resume` | downloadsResume | id |
| `downloads_replace_url` | downloadsReplaceUrl | id, url |
| `downloads_cancel` | downloadsCancel | id |
| `downloads_remove` | downloadsRemove | id |
| `downloads_clear_finished` | downloadsClearFinished | — |
| `downloads_set_connections` | downloadsSetConnections | connections, id |
| `downloads_open_folder` | downloadsOpenFolder | path |
| `super_merge_validate_package` | superMergeValidatePackage | packagePath |
| `super_merge_preflight` | superMergePreflight | packagePath |
| `super_merge_run` | superMergeRun | packagePath |
| `super_merge_residue_preview` | superMergeResiduePreview | packagePath |
| `super_merge_cleanup` | superMergeCleanup | packagePath |
| `super_merge_cancel` | superMergeCancel | — |
| `ofp_default_output_dir` | ofpDefaultOutputDir | — |
| `ofp_probe` | ofpProbe | chipset, inputPath |
| `ofp_decrypt_run` | ofpDecryptRun | chipset, inputPath, mergeSuper, outputDir |
| `ofp_decrypt_cancel` | ofpDecryptCancel | — |
| `boot_info_parse` | bootInfoParse | imagePath |
| `lib_backup_enumerate` | libBackupEnumerate | serial |
| `lib_backup_start` | libBackupStart | payload |
| `lib_backup_cancel` | libBackupCancel | — |
| `qcn_check_tool` | qcnCheckTool | — |
| `qcn_list_ports` | qcnListPorts | — |
| `qcn_detect_port` | qcnDetectPort | — |
| `qcn_parse_imei` | qcnParseImei | qcnPath |
| `qcn_parse_identity` | qcnParseIdentity | qcnPath |
| `qcn_enable_diag` | qcnEnableDiag | serial |
| `qcn_backup` | qcnBackup | payload |
| `qcn_restore` | qcnRestore | payload |
| `qcn_cancel` | qcnCancel | — |
| `wallpaper_list_local` | wallpaperListLocal | — |
| `wallpaper_list_cloud_static` | wallpaperListCloudStatic | — |
| `wallpaper_list_dynamic` | wallpaperListDynamic | — |
| `wallpaper_download_static` | wallpaperDownloadStatic | fileName |
| `wallpaper_download_dynamic` | wallpaperDownloadDynamic | fileName |
| `wallpaper_exists` | wallpaperExists | path |
| `wallpaper_default_path` | wallpaperDefaultPath | — |
| `wallpaper_dirs` | wallpaperDirs | — |
| `wallpaper_import_static` | wallpaperImportStatic | sourcePath |
| `wallpaper_import_dynamic` | wallpaperImportDynamic | sourcePath |
| `wallpaper_clear_static` | wallpaperClearStatic | activePath |
| `wallpaper_clear_dynamic` | wallpaperClearDynamic | activePath |
| `background_preference_load` | backgroundPreferenceLoad | — |
| `background_preference_save` | backgroundPreferenceSave | path, preference |
| `status_bar_info` | statusBarInfo | — |
| `marquee_messages` | marqueeMessages | — |
| `ocdt_list_models` | ocdtListModels | — |
| `ocdt_generate` | ocdtGenerate | platform, projectId |
| `nv_validate_image` | nvValidateImage | path |
| `persist_validate_image` | persistValidateImage | path |
| `modem_validate_image` | modemValidateImage | path |
| `utility_xiaomi_acc_repair` | utilityXiaomiAccRepair | modemPath, persistPath |
| `loader_download_start` | loaderDownloadStart | — |
| `loader_download_cancel` | loaderDownloadCancel | — |
| `utility_xiaomi_grant_applist` | utilityXiaomiGrantApplist | — |
| `utility_xiaomi_usb_security` | utilityXiaomiUsbSecurity | — |
| `utility_oneplus_unlock_qcom` | utilityOneplusUnlockQcom | — |
| `utility_oneplus_unlock_dimensity` | utilityOneplusUnlockDimensity | — |
| `utility_oneplus_repair_baseband` | utilityOneplusRepairBaseband | — |
| `utility_xiaomi_gen3_nv` | utilityXiaomiGen3Nv | — |
| `utility_lk_sign_unlock` | utilityLkSignUnlock | — |
| `lk_runtime_status` | lkRuntimeStatus | — |
| `lk_runtime_ensure` | lkRuntimeEnsure | — |
| `lk_unlock_runtime_ensure` | lkUnlockRuntimeEnsure | — |
| `lk_quota_status` | lkQuotaStatus | args |
| `lk_patch_run` | lkPatchRun | args |
| `lk_unlock_patch_run` | lkUnlockPatchRun | args |
| `utility_android_relock_bl` | utilityAndroidRelockBl | — |
| `utility_nv_repair` | utilityNvRepair | inputPath |
| `utility_persist_acc_repair` | utilityPersistAccRepair | inputPath |
| `utility_vab_inspect_image` | utilityVabInspectImage | path |
| `utility_vab_analyze` | utilityVabAnalyze | path |
| `utility_vab_sign` | utilityVabSign | args |
| `utility_mtk_gpt_generate` | utilityMtkGptGenerate | args |
| `lun_extract_parse` | lunExtractParse | sourceDir |
| `lun_extract_start` | lunExtractStart | payload |
| `lun_extract_cancel` | lunExtractCancel | — |
| `qc_gpt_validate` | qcGptValidate | path |
| `utility_xiaomi_perfect_nv_gpt_preview` | utilityXiaomiPerfectNvGptPreview | path |
| `utility_xiaomi_perfect_nv_gpt` | utilityXiaomiPerfectNvGpt | args |
| `lenovo_sn_read_image` | lenovoSnReadImage | path |
| `lenovo_sn_write_image` | lenovoSnWriteImage | args |
| `ufs_des_inspect` | ufsDesInspect | path |
| `ufs_des_patch` | ufsDesPatch | args |
| `da_patch_run` | daPatchRun | args |
| `oplus_custom_inspect` | oplusCustomInspect | path |
| `oplus_custom_generate` | oplusCustomGenerate | args |
| `utility_lenovo_unlock_bl` | utilityLenovoUnlockBl | args |
| `utility_adb_read_lib_info` | utilityAdbReadLibInfo | — |
| `xiaomi_unlock_session_reset` | xiaomiUnlockSessionReset | — |
| `xiaomi_unlock_qr_start` | xiaomiUnlockQrStart | — |
| `xiaomi_unlock_qr_poll` | xiaomiUnlockQrPoll | args |
| `xiaomi_unlock_password_login` | xiaomiUnlockPasswordLogin | args |
| `xiaomi_unlock_submit_captcha` | xiaomiUnlockSubmitCaptcha | args |
| `xiaomi_unlock_2fa_send` | xiaomiUnlock2faSend | args |
| `xiaomi_unlock_2fa_verify` | xiaomiUnlock2faVerify | args |
| `xiaomi_unlock_set_zone` | xiaomiUnlockSetZone | args |
| `xiaomi_unlock_logout` | xiaomiUnlockLogout | — |
| `xiaomi_unlock_restore_session` | xiaomiUnlockRestoreSession | — |
| `xiaomi_unlock_preview` | xiaomiUnlockPreview | — |
| `xiaomi_unlock_bl` | xiaomiUnlockBl | — |
| `utility_cancel` | utilityCancel | — |
| `clipboard_write_text` | (direct call) | text |
| `clipboard_read_text` | (direct call) | — |
| `network_security_preflight` | (direct call) | — |
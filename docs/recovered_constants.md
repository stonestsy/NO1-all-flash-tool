# 已恢复的常量与接口（证据摘要）

> 全部来自脱壳镜像 `../All-FlashTool-unpacked/` 的已解密内存页；RVA 见
> `../All-FlashTool-unpacked/strings_ascii.txt` 与 `strings_cjk_utf8.txt`。

## 自有云服务（aft-auth / aft-edl / aft-rom）
- https://yxnyryrnyz.all-flashtool.online/rom_api.php
- https://yxnyryrnyz.all-flashtool.online/rom_link.php
- https://yxnyryrnyz.all-flashtool.online/device_op.php
- https://yxnyryrnyz.all-flashtool.online/da_api.php
- 提示：登录已失效，请重新登录后再加载
- 认证响应加密字段：iv / ts / tag / nonce；错误：响应解密或认证失败 / 响应时间戳异常
- 调试开关：ALL_FT_ALLOW_HTTP_API

## 卡密/授权签名（aft-auth / aft-security）
- https://siyanbaozi.cn/api/sig/by-card （Content-Type, kamicrates）
- https://oplus.xiriacg.top/api/sig/by-card
- https://xiaoyueapi.cn

## 小米账号与解锁（aft-utility::xiaomi_unlock）
- https://account.xiaomi.com/pass/serviceLogin
- https://account.xiaomi.com/pass/serviceLoginAuth2
- https://account.xiaomi.com/identity/auth/sendPhoneTicket
- https://account.xiaomi.com/identity/auth/sendEmailTicket
- https://account.xiaomi.com/longPolling/loginUrl
- https://account.xiaomi.com/identity/auth/verifyPhone / verifyEmail
- https://unlock.update.miui.com (+ eu-/ru-/in- 节点)

## MIUI OTA / ROM（aft-rom）
- https://update.miui.com/updates/miotaV3.php
- https://xiaomirom.com
- https://ultimateota.d.miui.com / superota.d.miui.com / bn.d.miui.com / cdnorg.d.miui.com
- https://cdn-ota.azureedge.net
- https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com

## 123 云盘（aft-pan123）
- https://login.123pan.com/api/user/sign_in
- https://login.123pan.com/api/user/get_vcode
- https://login.123pan.com/api/user/qr-code/generate / result / wx_code
- https://yun.123pan.com / https://yun.123pan.cn/wx-app-login.html

## 联想（aft-rom::lenovo / aft-utility）
- https://ptstpd.lenovo.com.cn/home/ConfigurationQuery/getMachineSequenceInfo
- https://ptstpd.lenovo.com.cn/home/ConfigurationQuery/getPadFlashingMachinemtm
- http://zsk-cdn.lenovows.com/（固件 CDN）

## 辅助网络
- http://ip-api.com/json/ (地区定位)
- https://cz88.net/api/cz88/ip/base
- https://ip.zxinc.org/api.php
- https://whois.pconline.com.cn/ipJson.jsp
- https://myip.ipip.net/s
- https://api.open-meteo.com/v1/forecast

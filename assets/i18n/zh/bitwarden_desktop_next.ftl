## App-wide
app-title = Bitwarden [Next]
about-window-title = 关于 Bitwarden

## Login — unlock
login-unlock-title = 您的密码库已锁定
login-unlock-password-placeholder = 主密码（必填）
login-unlock-pin-placeholder = PIN（必填）
login-unlock-button = 解锁
login-unlock-or = 或
login-unlock-biometrics-button = 使用生物识别解锁
login-unlock-pin-button = 使用 PIN 解锁
login-unlock-master-password-button = 使用主密码解锁
login-log-out = 注销

## Login — email entry
login-email-title = 登录到 Bitwarden
login-email-placeholder = 电子邮箱地址（必填）
login-email-remember = 记住电子邮箱
login-email-continue = 继续
login-email-or = 或
login-email-sso = 使用单点登录
login-email-new-prompt = 第一次使用 Bitwarden？
login-email-create-account = 创建账户

## Login — password entry
login-password-title = 欢迎回来
login-password-placeholder = 主密码（必填）
login-password-get-hint = 获取主密码提示
login-password-submit = 使用主密码登录
login-password-back = 返回

## Login — server selector
login-server-accessing = 正在访问 { $server }
login-server-accessing-label = 正在访问：
login-server-self-hosted = 自托管

## Login — self-hosted environment modal
login-self-hosted-modal-title = 自托管环境
login-self-hosted-modal-url-label = 服务器 URL
login-self-hosted-modal-url-helper = 指定您本地托管的 Bitwarden 安装的基础 URL。例如：https://bitwarden.company.com
login-self-hosted-modal-url-error = URL 必须以 https:// 开头
login-self-hosted-modal-save = 保存
login-self-hosted-modal-cancel = 取消

## Login — toasts
login-toast-unlock-failed-title = 解锁失败
login-toast-unlock-failed-body = 请检查您的主密码并重试。
login-toast-login-failed-title = 登录失败
login-toast-login-failed-body = 请检查您的电子邮箱和密码并重试。
login-toast-pin-unsupported = 暂不支持 PIN 解锁
login-toast-biometrics-unsupported = 暂不支持生物识别解锁

## About dialog
about-version-label = 版本
about-sdk-version-label = SDK 版本
about-os-label = 操作系统
about-architecture-label = 架构
about-copy-button = 复制
about-close-button = 关闭

## Sidebar — sections and filters
sidebar-section-vault = 密码库
sidebar-section-send = Send
sidebar-filter-my-vault = 我的密码库
sidebar-filter-favorites = 收藏
sidebar-filter-logins = 登录
sidebar-filter-cards = 支付卡
sidebar-filter-identities = 身份
sidebar-filter-notes = 笔记
sidebar-filter-ssh-keys = SSH 密钥
sidebar-filter-archive = 归档
sidebar-filter-trash = 回收站
sidebar-filter-text-send = 文本
sidebar-filter-file-send = 文件
sidebar-item-generator = 生成器
sidebar-item-import = 导入
sidebar-item-export = 导出

## Vault list
vault-title = 密码库
vault-new-button = 新建
vault-search-placeholder = 搜索
vault-column-name = 名称
vault-column-options = 选项
vault-toast-item-saved = 项目已保存
vault-toast-save-failed-title = 保存失败
vault-toast-save-failed-body = 无法保存项目。请重试。
vault-toast-decrypt-failed-title = 解密失败
vault-toast-decrypt-failed-body = 无法加载项目。请重试。
vault-toast-copied-username = 已复制用户名
vault-toast-copied-password = 已复制密码
vault-toast-copied-website = 已复制网址
vault-toast-copied-totp = 已复制验证码
vault-toast-copied-field = 已复制字段
vault-toast-copied-private-key = 已复制私钥
vault-toast-copied-public-key = 已复制公钥
vault-toast-copied-fingerprint = 已复制指纹
vault-toast-item-deleted = 项目已移至回收站
vault-toast-delete-failed-title = 删除失败
vault-toast-delete-failed-body = 无法删除项目。请重试。
vault-delete-modal-title = 删除项目？
vault-delete-modal-body = "{ $name }" 将被移至回收站。
vault-delete-modal-cancel = 取消
vault-delete-modal-confirm = 删除

## Account switcher
account-switcher-other-accounts = 其他 Bitwarden 账户
account-switcher-options = 选项
account-switcher-lock-now = 立即锁定
account-switcher-log-out = 注销
account-switcher-lock-all = 锁定所有账户
account-switcher-settings = 设置
account-switcher-add = 添加账户

## Detail pane — headers per cipher type
detail-header-login = 查看登录
detail-header-card = 查看卡片
detail-header-identity = 查看身份
detail-header-note = 查看笔记
detail-header-ssh-key = 查看 SSH 密钥
detail-header-bank-account = 查看银行账户
detail-header-drivers-license = 查看驾照
detail-header-passport = 查看护照

## Detail pane — section labels
detail-section-item-details = 项目详情
detail-section-login-credentials = 登录凭据
detail-section-autofill-options = 自动填充选项
detail-section-card-details = 卡片详情
detail-section-personal-details = 个人详情
detail-section-note = 笔记
detail-section-ssh-key = SSH 密钥

## Detail pane — fields
detail-field-name = 名称
detail-field-notes = 笔记
detail-field-username = 用户名
detail-field-password = 密码
detail-field-totp = 验证码（TOTP）
detail-totp-invalid = TOTP 种子无效
detail-field-website = 网址
detail-field-cardholder-name = 持卡人姓名
detail-field-brand = 品牌
detail-field-number = 号码
detail-field-expiration = 到期
detail-field-security-code = 安全码
detail-field-email = 电子邮箱
detail-field-phone = 电话
detail-field-company = 公司
detail-field-address = 地址
detail-field-city-region = 城市 / 地区
detail-field-public-key = 公钥
detail-field-private-key = 私钥
detail-field-fingerprint = 指纹
detail-empty-credentials = 无凭据
detail-empty-card = 无卡片详情
detail-empty-identity = 无身份详情
detail-section-custom-fields = 自定义字段
detail-field-passkey = 通行密钥
detail-field-passkey-created = 创建于 { $date }
detail-field-boolean-true = 是
detail-field-boolean-false = 否
detail-edit-button = 编辑

## Cipher form — header titles
form-title-new-item = 新建项目
form-title-edit-login = 编辑登录
form-title-edit-card = 编辑卡片
form-title-edit-identity = 编辑身份
form-title-edit-note = 编辑笔记
form-title-edit-ssh-key = 编辑 SSH 密钥
form-title-edit-bank-account = 编辑银行账户
form-title-edit-drivers-license = 编辑驾照
form-title-edit-passport = 编辑护照

## Cipher form — buttons
form-save = 保存
form-saving = 保存中…
form-cancel = 取消

## Cipher form — sections
form-section-item-details = 项目详情
form-section-login-credentials = 登录凭据
form-section-autofill-options = 自动填充选项
form-section-card-details = 卡片详情
form-section-personal-details = 个人详情
form-section-identification = 身份证明
form-section-contact-info = 联系信息
form-section-address = 地址
form-section-ssh-key = SSH 密钥
form-section-additional-options = 其他选项
form-section-custom-fields = 自定义字段

## Cipher form — item details
form-name = 名称（必填）
form-favorite = 收藏
form-reprompt = 重新提示主密码
form-notes = 笔记
form-folder = 文件夹
form-folder-none = 无文件夹
form-organization = 组织
form-organization-personal = 个人（我）
form-collections = 集合
form-collections-none = 无集合
form-collections-selected = 已选择 { $count } 个
form-collections-empty-in-org = 此组织中没有集合

## Cipher form — login
form-username = 用户名
form-password = 密码
form-totp = 验证器密钥（TOTP）

## Cipher form — websites
form-uri = 网址（URI）
form-uri-empty = 暂无网站
form-add-website = 添加网站

## Cipher form — card
form-card-cardholder = 持卡人姓名
form-card-brand = 品牌
form-card-number = 号码
form-card-exp-month = 到期月份
form-card-exp-year = 到期年份
form-card-code = 安全码
form-card-brand-placeholder = -- 选择 --
form-card-month-placeholder = -- 月份 --

## Cipher form — identity
form-identity-title = 称谓
form-identity-title-placeholder = -- 称谓 --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = 先生
form-identity-title-mrs = 夫人
form-identity-title-ms = 女士
form-identity-title-mx = Mx
form-identity-title-dr = 博士
form-identity-first-name = 名
form-identity-middle-name = 中间名
form-identity-last-name = 姓
form-identity-username = 用户名
form-identity-company = 公司
form-identity-ssn = 社会保障号码
form-identity-passport = 护照号码
form-identity-license = 驾驶证号码
form-identity-email = 电子邮箱
form-identity-phone = 电话
form-identity-address1 = 地址第 1 行
form-identity-address2 = 地址第 2 行
form-identity-address3 = 地址第 3 行
form-identity-city = 城市 / 城镇
form-identity-state = 州 / 省
form-identity-postal = 邮政编码
form-identity-country = 国家

## Cipher form — SSH key
form-ssh-public-key = 公钥
form-ssh-private-key = 私钥
form-ssh-fingerprint = 指纹

## Cipher form — custom fields
form-custom-field-type = 类型
form-custom-field-name = 名称
form-custom-field-value = 值
form-custom-field-type-text = 文本
form-custom-field-type-hidden = 隐藏
form-custom-field-type-boolean = 布尔值
form-custom-field-type-linked = 关联
form-custom-field-enabled = 已启用
form-custom-field-empty = 暂无自定义字段
form-add-custom-field = 添加自定义字段
form-custom-field-linked-unsupported = 暂不支持关联字段

## Toast — shared messages
toast-required-fields = 请填写必填字段。

## Menu bar — top-level
menu-file = 文件
menu-edit = 编辑
menu-view = 视图
menu-account = 账户
menu-window = 窗口
menu-help = 帮助

## Menu — File
menu-file-new-login = 新建登录
menu-file-new-item = 新建项目
menu-file-new-item-login = 登录
menu-file-new-item-card = 支付卡
menu-file-new-item-identity = 身份
menu-file-new-item-secure-note = 安全笔记
menu-file-new-item-ssh-key = SSH 密钥
menu-file-new-folder = 新建文件夹
menu-file-sync-now = 立即同步
menu-file-import = 导入
menu-file-export = 导出
menu-file-settings = 设置
menu-file-lock-vault = 锁定密码库
menu-file-lock-all-vaults = 锁定所有密码库
menu-file-log-out = 注销
menu-file-quit = 退出 Bitwarden

## Menu — Edit
menu-edit-undo = 撤销
menu-edit-redo = 重做
menu-edit-cut = 剪切
menu-edit-copy = 复制
menu-edit-paste = 粘贴
menu-edit-select-all = 全选
menu-edit-copy-username = 复制用户名
menu-edit-copy-password = 复制密码
menu-edit-copy-totp = 复制验证码（TOTP）

## Menu — View
menu-view-search = 搜索密码库
menu-view-generator = 生成器
menu-view-generator-history = 生成器历史
menu-view-zoom-in = 放大
menu-view-zoom-out = 缩小
menu-view-reset-zoom = 重置缩放
menu-view-toggle-fullscreen = 切换全屏

## Menu — Account
menu-account-change-password = 更改主密码
menu-account-two-step = 两步登录
menu-account-fingerprint = 指纹短语
menu-account-delete = 删除账户

## Menu — Window
menu-window-minimize = 最小化
menu-window-hide-to-tray = 隐藏到托盘
menu-window-always-on-top = 始终置顶
menu-window-close = 关闭

## Menu — Help
menu-help-feedback = 帮助与反馈
menu-help-bug = 提交错误报告
menu-help-legal = 法律
menu-help-legal-tos = 服务条款
menu-help-legal-privacy = 隐私政策
menu-help-follow = 关注我们
menu-help-web-vault = 转到网页密码库
menu-help-browser-extension = 获取浏览器扩展
menu-help-about = 关于 Bitwarden

menu-toast-sync-success = 密码库已同步
menu-toast-sync-failed-title = 同步失败
menu-toast-sync-failed-body = 无法同步密码库。请稍后重试。

menu-fingerprint-title = 您账户的指纹短语：
menu-fingerprint-learn-more = 了解更多
menu-fingerprint-close = 关闭

new-folder-modal-title = 新建文件夹
new-folder-modal-field-label = 文件夹名称（必填）
new-folder-modal-helper = 通过添加父文件夹名称后跟 "/" 来嵌套文件夹。例如：社交/论坛
new-folder-modal-save = 保存
new-folder-modal-cancel = 取消
new-folder-toast-success = 文件夹已创建
new-folder-toast-failed-title = 无法创建文件夹
new-folder-toast-failed-body = 无法保存文件夹。请重试。

## New-item picker modal
picker-title = 选择要添加的项目
picker-folder = 文件夹
picker-login-subtitle = 网站或应用
picker-card-subtitle = 信用卡或借记卡
picker-bank-account-subtitle = 银行账户信息
picker-drivers-license-subtitle = 驾照信息
picker-passport-subtitle = 旅行证件
picker-identity-subtitle = 个人信息
picker-secure-note-subtitle = 重要文本
picker-ssh-key-subtitle = 服务器登录令牌
picker-folder-subtitle = 整理项目

## Tray
tray-show-hide = 显示 / 隐藏
tray-lock-vault = 锁定密码库
tray-exit = 退出

# Endonym — see comment in the canonical en file.
language-name-self = 中文

## Settings modal
settings-title = 设置
settings-tab-security = 安全
settings-tab-integrations = 集成
settings-tab-autotype = 自动输入与复制
settings-tab-appearance = 外观
settings-tab-advanced = 高级

settings-toast-not-supported = 暂不支持此设置。

# Security tab
settings-security-access-options = 访问选项
settings-security-open-at-login = 设备登录时打开 Bitwarden
settings-security-unlock-pin = 使用 PIN 解锁
settings-security-unlock-touch = 使用 Touch ID 解锁
settings-security-session-timeout = 会话超时
settings-security-lock-after = 在以下时间后锁定
settings-security-logout-after = 在以下时间后注销
settings-security-lock-on-system-lock = 系统锁定时锁定

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } 秒
   *[other] { $n } 秒
  }
settings-duration-minutes = { $n ->
    [one] { $n } 分钟
   *[other] { $n } 分钟
  }
settings-duration-hours = { $n ->
    [one] { $n } 小时
   *[other] { $n } 小时
  }
settings-duration-never = 永不

# Integrations tab
settings-integrations-browser = 浏览器集成
settings-integrations-browser-enable = 启用浏览器集成
settings-integrations-browser-fingerprint = 要求验证指纹
settings-integrations-ssh = SSH 代理
settings-integrations-ssh-enable = 启用 SSH 代理
settings-integrations-ssh-prompt = 提示行为
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = 从不
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Autotype and copy tab
settings-autotype-heading = 自动输入
settings-autotype-enable = 启用自动输入
settings-clipboard-heading = 剪贴板
settings-clipboard-clear-after = 在以下时间后清除剪贴板
settings-clipboard-minimize-on-copy = 复制时最小化

# Appearance tab
settings-appearance-theme-heading = 主题
settings-appearance-theme = 主题
settings-appearance-theme-system = 系统
settings-appearance-theme-light = 浅色
settings-appearance-theme-dark = 深色
settings-appearance-language = 语言
settings-appearance-language-system = 系统
settings-appearance-display-heading = 显示
settings-appearance-show-favicons = 为 URL 显示图标

# Advanced tab
settings-advanced-tray = 托盘
settings-advanced-tray-enable = 显示托盘图标
settings-advanced-minimize-to-tray = 最小化到托盘
settings-advanced-close-to-tray = 关闭到托盘
settings-advanced-platform = 平台
settings-advanced-always-show-dock = 始终显示 Dock 图标
settings-advanced-allow-screenshots = 允许截图

## Send list
send-title = Send
send-new-button = 新建
send-search-placeholder = 搜索 Sends
send-column-name = 名称
send-column-deletion = 删除日期
send-empty-body = 您还没有创建任何 Send。
send-toast-item-saved = Send 已保存
send-toast-item-deleted = Send 已删除
send-toast-copied-link = Send 链接已复制
send-toast-copied-password = 已复制密码
send-toast-load-failed-title = 加载失败
send-toast-load-failed-body = 无法加载 Send。请重试。
send-toast-save-failed-title = 保存失败
send-toast-save-failed-body = 无法保存 Send。请重试。
send-toast-delete-failed-title = 删除失败
send-toast-delete-failed-body = 无法删除 Send。请重试。

## Send form
send-form-title-new-text = 创建文本 Send
send-form-title-new-file = 创建文件 Send
send-form-title-edit-text = 编辑文本 Send
send-form-title-edit-file = 编辑文件 Send
send-form-details-heading = Send 详情
send-form-additional-heading = 其他选项
send-form-name = 名称（必填）
send-form-text = 要分享的文本（必填）
send-form-hide-text = 默认隐藏文本
send-form-file-choose = 选择文件
send-form-file-choose-placeholder = 未选择文件
send-form-file-name = 文件名
send-form-file-size = 大小
send-form-deletion-date = 删除日期
send-form-deletion-hint = Send 将于 {$date} 永久删除。
send-form-who-can-view = 谁可以查看
send-form-access-link = 任何拥有此链接的人
send-form-access-people = 特定人员
send-form-access-password = 任何拥有您设置的密码的人
send-form-password = 密码（必填）
send-form-password-hint = 个人需要输入密码才能查看此 Send。
send-form-emails = 电子邮箱（必填）
send-form-send-link = Send 链接
send-form-limit-views = 限制查看次数
send-form-limit-views-hint = 达到限制后没有人可以查看此 Send。
send-form-views-left = 达到限制后没有人可以查看此 Send。剩余 {$count} 次查看。
send-form-hide-email = 对查看者隐藏您的电子邮箱地址。
send-form-private-note = 私人备注
send-form-save = 保存
send-form-cancel = 取消

send-form-preset-1h = 1 小时
send-form-preset-1d = 1 天
send-form-preset-2d = 2 天
send-form-preset-3d = 3 天
send-form-preset-7d = 7 天
send-form-preset-14d = 14 天
send-form-preset-30d = 30 天

send-delete-modal-title = 删除 Send
send-delete-modal-body = 您确定要删除 "{$name}" 吗？
send-delete-modal-cancel = 取消
send-delete-modal-confirm = 删除

## Generator
generator-title = 生成器
generator-tab-password = 密码
generator-tab-passphrase = 密码短语
generator-tab-username = 用户名

# Password tab
generator-length = 长度
generator-length-hint = 值必须在 5 到 128 之间。
generator-include = 包含
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = 最少数字
generator-min-special = 最少特殊字符
generator-avoid-ambiguous = 避免模糊字符

# Passphrase tab
generator-num-words = 单词数量
generator-num-words-hint = 值必须在 3 到 20 之间。使用 6 个或更多单词以生成强密码短语。
generator-word-separator = 单词分隔符
generator-passphrase-capitalize = 首字母大写
generator-passphrase-include-number = 包含数字

# Username tab
generator-username-type = 类型
generator-username-kind-word = 随机单词
generator-username-kind-subaddress = 加号子地址电子邮箱
generator-username-kind-catchall = Catch-all 电子邮箱
generator-username-capitalize = 首字母大写
generator-username-include-number = 包含数字
generator-username-email = 电子邮箱
generator-username-domain = 域名

# History
generator-history-open = 生成器历史
generator-history-title = 生成器历史
generator-history-heading = 最近
generator-history-empty = 暂无最近的值。
generator-history-clear = 清除历史
generator-history-just-now = 刚刚
generator-history-minutes-ago = {$count} 分钟前
generator-history-hours-ago = {$count} 小时前
generator-history-days-ago = {$count} 天前

# Toasts
generator-toast-copied = 已复制
generator-toast-failed = 无法生成

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = 您的密码库已锁定
magnify-locked-subtitle = 打开 Bitwarden 以解锁
magnify-open-bitwarden = 打开 Bitwarden
magnify-no-results = 没有匹配的项目
magnify-copy-password = 复制密码
magnify-copy-username = 复制用户名
magnify-hint-navigate = 导航

## Import modal
import-modal-title = 导入数据
import-modal-section-destination = 目标
import-modal-vault-label = 密码库（必填）
import-modal-vault-personal = 我的密码库
import-modal-folder-label = 文件夹
import-modal-folder-placeholder = - 选择文件夹 -
import-modal-collection-label = 集合（必填）
import-modal-collection-placeholder = - 选择集合 -
import-modal-section-data = 数据
import-modal-file-format-label = 文件格式（必填）
import-modal-file-helper = 选择要导入的文件
import-modal-choose-file = 选择文件
import-modal-no-file = 未选择文件
import-modal-paste-label = 或复制/粘贴导入文件的内容
import-modal-submit = 导入数据
import-modal-cancel = 取消
import-toast-unimplemented = 导入功能尚未接入 — 即将推出

## Export modal
export-modal-title = 导出密码库
export-modal-vault-label = 从以下导出（必填）
export-modal-vault-personal = 我的密码库
export-modal-banner-personal-title = 导出个人密码库
export-modal-banner-personal = 仅导出与 { $email } 关联的个人密码库项目。组织密码库项目将不会被包含。仅导出项目信息，不包括相关附件。
export-modal-banner-org-title = 导出组织密码库
export-modal-banner-org-body = 仅导出与 { $name } 关联的组织密码库。个人密码库或其他组织中的项目将不会被包含。
export-modal-file-format-label = 文件格式（必填）
export-modal-password-label = 文件密码（必填）
export-modal-continue = 继续
export-modal-cancel = 取消
export-confirm-title = 确认导出密码库
export-confirm-warning-unencrypted = 此导出包含未加密格式的密码库数据。请勿通过不安全的渠道（如电子邮箱）存储或发送导出的文件。使用完毕后请立即删除。
export-confirm-warning-file-encrypted = 此文件导出将受密码保护，需要文件密码才能解密。
export-confirm-password-label = 主密码（必填）
export-confirm-helper = 确认您的身份以继续。
export-confirm-error = 主密码无效。
export-toast-success = 密码库已导出至 { $path }
export-toast-failed-title = 导出失败
export-toast-failed-body = 无法导出密码库。请重试。

# Composable vault filters
sidebar-group-vaults = 密码库
sidebar-group-view = 视图
sidebar-group-types = 项目类型
sidebar-filter-all-vaults = 所有密码库
sidebar-filter-active-items = 有效项目
sidebar-filter-all-types = 所有项目类型

sidebar-group-folders = 文件夹
sidebar-filter-all-folders = 所有文件夹
sidebar-filter-no-folder = 无文件夹
sidebar-group-collections = 集合
sidebar-filter-all-collections = 所有集合
sidebar-filter-no-collection = 无集合

vault-column-last-changed = 最后修改
vault-column-base-url = 基础 URL
vault-filter-folder = 文件夹：{ $name }
vault-filter-collection = 集合：{ $name }
vault-filter-search = 搜索：“{ $query }”

ssh-approval-title = Allow SSH authentication?
ssh-approval-key = Key: { $name }
ssh-approval-user = SSH user: { $name }
ssh-approval-process = Local process: { $name }
ssh-approval-process-note = The process name is informational and does not verify an application or destination hostname.
ssh-approval-unbound = No verified host-key binding. This request can only be approved once.
ssh-approval-hosts = Verified host keys (forwarding path → destination):
ssh-approval-deny = Deny
ssh-approval-once = Allow once
ssh-approval-remember = Allow until lock
ssh-approval-timeout = No response within 45 seconds denies the request.
ssh-agent-copy-socket = Copy socket path
ssh-agent-socket-copied = SSH socket path copied
ssh-agent-help = Set SSH_AUTH_SOCK or the SSH IdentityAgent option to this path. Only the active unlocked account provides keys.
ssh-agent-unattended = Without prompts, any local process of your user can use permitted SSH keys while the active vault is unlocked.
ssh-agent-protected = Deleted, archived, hidden-password and master-password-reprompt keys are not offered. Remembered approvals require a verified session binding and a host-bound signature.

ssh-agent-copy-setup = Copy SSH_AUTH_SOCK command
ssh-agent-setup-copied = SSH_AUTH_SOCK command copied
ssh-approval-bound-once = The host key is verified, but this signing request is not host-bound. Only one-use approval is available.

ssh-approval-window-open = SSH approval is waiting in its confirmation window. Press Esc to cancel.

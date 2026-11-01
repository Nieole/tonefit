# CI 与阶段交付 — 四平台、解压即用、全绿才发布

**Status:** ready-for-agent

## 目标与完成标准

给公开仓库 `Nieole/tonefit` 建立 GitHub Actions 验收和阶段交付链。PR 与 main 持续验收并生成临时构建包；预发布标签触发同一提交的完整验收，再发布长期可下载的 GitHub Release 预发布。

本轮必须实际发布 **`v0.1.0-alpha.1`** 并验收远端资产，不以 YAML 合入或“编译成功”代替交付。所有必需检查全绿才发布；不得跳过/削弱失败测试、使用 `continue-on-error` 或让半套资产先公开。

## 已确认的产品边界

- 四个目标：Windows x64、Linux x64、macOS ARM64、macOS Intel；均提供带默认 TUI 的程序。
- 用户下载解压后无需安装 Rust、Homebrew、dav1d 或其他非系统运行库。包内带齐实际需要的非系统运行库、许可、说明及构建来源信息，提供最终归档的 SHA256 校验和。
- 保留全部既有输入能力，包括 AVIF、RAR。不得通过去掉功能或换解码器绕开打包难题。
- 首轮不做发行者签名或 Apple 公证。可按平台需要做本地 ad-hoc 签名，但必须明确它不是发行者身份认证，提供 Gatekeeper/SmartScreen 提示及系统放行说明。
- 首发标签、程序版本、包名一致为 `0.1.0-alpha.1`；阶段序号后续递增。
- 兼容边界：Linux glibc 2.35+（Ubuntu 22.04 同级）、macOS 15+、Windows 10/11 x64。分别记录最低构建要求和真实运行验证系统；Windows Server 2022 CI 不冒充 Win10 客户端实测，未验证的老系统不得标成已支持。
- Q995 与阻断本轮目标的跨平台问题纳入修复。已有真实素材用例的缺素材跳过必须明确报告，不宣称已实测；不得新增跳测来绕过平台问题。
- Windows 首轮真实测试暴露普通绝对路径与 `\\?\` 扩展前缀不相认；用户已明确批准补安全前缀等价，保留现有回归，不通过缩窄测试绕过。涉及保留字、尾点/尾空格或特殊分量等改变语义的命名空间仍区别对待；词义已同步《同一处》，决定由 ADR 0022 承载。

## 验收矩阵

原生 runner 固定为 `ubuntu-22.04`、`windows-2022`、`macos-15`（ARM64）、`macos-15-intel`，先核对实际架构。Rust 固定 1.98.0，Node 使用满足当前 jsdom engine 的固定 24 LTS 版本；actions 引用完整提交 SHA，依赖受锁文件约束。

四个平台沿用 `docs/agents/gate.md` 的三构建闸门与 polish。设计稿导出字节一致性与全部 JS 测试参加发布门禁。标签事件对固定 SHA 重新运行所有必需检查，不借用另一提交的成功结果。

包以 Windows ZIP / Unix tar.gz 交付。对最终归档重新提取后验证架构、版本、依赖闭包、许可、执行位（Unix）与真实转换；至少含 AVIF、RAR 及其他代表性输入。Linux 在最低基线容器中运行；macOS/Windows 清理构建环境搜索路径并审计系统/包内依赖，不让 runner 预装开发工具掩盖缺失运行库。

## 原生依赖与打包

优先从固定版本、校验和的源码构建静态 dav1d，保持现有 Cargo 解码链与 vendored UnRAR 修复。Windows 的 Rust、dav1d 与 C++ CRT 策略一致，优先静态 CRT；Linux 检查 glibc 符号版本与 C++ 运行库；macOS 检查部署目标并拒绝 Homebrew 绝对链接。任何需要随包分发的运行库必须与实际链接审计及许可证一致。

包中至少有程序、必要运行库、使用/系统提示说明、根 MIT 许可、适用第三方许可与通知、tag/commit/target/toolchain/native-dependency 版本来源。SHA256 在最终签名/重定位/归档之后计算；校验和只证明字节完整性，不宣称发行者认证。

## 发布与权限

- PR 使用普通 `pull_request`、默认只读权限，不用 `pull_request_target` 执行外来代码，不把不可信 PR 资产提升为 Release。
- 阶段标签采用预发布版本语义（alpha/beta/rc），必须与 Cargo 版本一致；解析唯一完整 SHA，确认属于真实获取的 `origin/main` 历史。构建与发布再次核对同一标签指向。
- 发布 job 仅在全部质量、设计、四目标包构建与包运行验证成功后取得 `contents: write`。构建 job 不拥有发布权限，外部文本不直接插值成 shell 代码。
- 先创建 draft prerelease，上传且核对四个精确目标包与 `SHA256SUMS`，回读验证数量、文件名、大小和哈希，再一次切为可见预发布。失败保留为未公开草稿并报告；重跑不覆盖已发布资产，不擅自重指已有阶段标签。
- 不修改仓库保护规则、申请签名凭据或创建稳定版发布。流水线具备失败报告与同标签尝试的串行化，不能把取消/跳过当成功。

## 实施顺序与归属

1. 修复 `.scratch/green-counts/issues/10-the-concurrency-check-sees-on-macos.md`（Q995）；macOS 补描述符路径探测，保留 Linux/Windows 问法及阳性对照。
2. 接入四平台质量门禁与设计验收，修复 Windows 上 Unix-only 测试的等价覆盖及其他真实构建阻断。
3. 接入原生依赖、四平台 release 包、包内验证与版本一致性。
4. 接入阶段标签安全发布与完整性校验；独立审查后在真实 GitHub CI 验证最终树。
5. 合入 main、CI 全绿后创建 `v0.1.0-alpha.1`；等待预发布完整成功，下载核验全部资产并交付链接。

交付链契约归 `.github/SPEC.md`；本地门禁命令仍归 `docs/agents/gate.md`，README 只提供用户入口。研究事实与未验证事项见 [research.md](research.md)。不改用户原有 `.gitattributes` 工作区改动。

## 非目标

稳定版自动发布、安装器或包管理器渠道、Linux/Windows ARM64、发行者签名/公证、未经验证的旧系统保证、凭空补充真实素材、修复无关业务问题。

## 已批准

产品目标、首版与签名边界、最低系统范围及上述整体实施方案已由用户逐轮确认并批准；无未确认产品假设。原生链接、跨平台行为和实际发行资产必须用真实 CI 证明，研究结论不能代替运行结果。

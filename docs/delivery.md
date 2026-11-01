# 下载与运行

面向直接使用 tonefit 的用户；源码构建另见 [README《从源码构建》](../README.md#从源码构建)。

## 获取阶段预发布

入口是 [GitHub Releases](https://github.com/Nieole/tonefit/releases)。只下载页面上已经公开、资产齐全的预发布；
本页用 `v0.1.0-alpha.1` 举例，文件名与命令本身不表示资产已经发布，也不是 CI 成功证明。
alpha 是开发预发布，不是稳定版。

发布必须在同一提交的全部必需检查、四平台最终包审计与运行验证成功后，上传完整资产并回读核验，
才可公开。没有完整预发布时请勿把分支/PR 的 Actions 临时构建包当作发行包。
发布门禁的权威说明见 [交付契约《阶段发布契约》](../.github/SPEC.md#阶段发布契约)。

在预发布页面展开 **Assets**，下载适合本机的一份归档，另下载同一版本的 `SHA256SUMS`。
不要选 GitHub 自动生成的 Source code (zip / tar.gz)，它们是源码，不是可执行包。

| 本机 | 示例：v0.1.0-alpha.1 的归档文件名 |
|---|---|
| Windows x64 | `tonefit-0.1.0-alpha.1-x86_64-pc-windows-msvc.zip` |
| Linux x64 | `tonefit-0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `tonefit-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `tonefit-0.1.0-alpha.1-x86_64-apple-darwin.tar.gz` |

macOS 可在「关于本机」查看芯片，Unix 终端可用 `uname -m` 查看架构。
后续阶段按该 Release 的版本号选包，不混用不同版本的校验和。

## 系统范围与验证边界

最低要求与兼容性策略以 [交付契约《原生构建输入》](../.github/SPEC.md#原生构建输入) 和
[《打包与最终字节验证》](../.github/SPEC.md#打包与最终字节验证) 为准；
runner/target 对应关系见 [《质量契约》](../.github/SPEC.md#质量契约)。

声明边界是 Windows 10/11 x64、macOS 15+（按架构分包）、Linux x64 glibc 2.35+
（Ubuntu 22.04 同级）。这不等于这些系统全部经过客户端实测：Windows 自动执行环境是
Windows Server 2022，不是 Windows 10/11 客户端；macOS 的两个架构使用 macOS 15 runner；
Linux 使用 Ubuntu 22.04 基线用户空间容器，共用宿主内核，不证明旧内核可用。
不把 CI 配置当作运行成功记录，也不把浏览器下载后的隔离提示算成已实测。

合格发行包必须通过最终归档的依赖闭包审计与实际转换验证，才可作为「解压即用」包发布：
**运行时无需安装 Rust、Homebrew、dav1d 或其他非系统运行库**。
四份包均启用默认 TUI；AVIF、RAR 与命令行功能在同一个程序里，并非删去解码能力的 headless 变体。

## 先校验，再解压与放行

把归档与 `SHA256SUMS` 放在同一个新目录，在该目录打开终端。
**任何校验失败、缺少对应条目或来源不明都应停止，不运行、不解除隔离。**
SHA256 只校验下载字节是否与清单一致，不认证发行者身份，也不保证程序安全；
清单若与归档一起被替换，校验不能发现这种替换。请自行确认仓库、版本与下载来源可信。

### Windows PowerShell

```powershell
$file = 'tonefit-0.1.0-alpha.1-x86_64-pc-windows-msvc.zip'
$lines = @(Get-Content -LiteralPath .\SHA256SUMS | Where-Object { $_ -match ('^[0-9a-fA-F]{64}  ' + [regex]::Escape($file) + '$') })
if ($lines.Count -ne 1) { throw '缺少或重复的校验条目，停止' }
$expected = ($lines[0] -split '  ', 2)[0]
$actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw 'SHA256 不符，停止' }
'SHA256 OK'
```

### Linux / macOS

以下以 Linux 为例；macOS 将 `file` 改为上表对应的归档名。
只核验所下载的那一份，不要求下载其余三份。

```sh
file='tonefit-0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz'
line=$(awk -v f="$file" '$2 == f { print }' SHA256SUMS)
# 必须恰好一条，且文件名与所选包一致；否则停止。
count=$(printf '%s\n' "$line" | awk 'NF { n++ } END { print n+0 }')
[ "$count" -eq 1 ] || { echo '缺少或重复的校验条目，停止'; exit 1; }
# Linux：必须看到所选归档的 OK；失败则停止。
printf '%s\n' "$line" | sha256sum -c -
```

macOS 没有 `sha256sum` 时，改用下面这一条；同样必须看到所选归档的 `OK`，
空条目、重复条目或错误均不继续：

```sh
printf '%s\n' "$line" | shasum -a 256 -c -
```

## 解压与首次系统提示

**本节仅在校验通过且信任来源后执行。** 使用普通用户目录，不需要管理员权限。
每份归档都只有一个与归档同名（去掉 `.zip` / `.tar.gz`）的顶层目录；保留整个目录，
不要只搬走程序而丢弃说明、许可与来源信息。

Windows PowerShell 解压并进入目录：

```powershell
Expand-Archive -LiteralPath $file -DestinationPath .\tonefit-unpacked
Set-Location .\tonefit-unpacked\tonefit-0.1.0-alpha.1-x86_64-pc-windows-msvc
```

Linux / macOS 解压并进入目录（沿用上节 `file`）：

```sh
tar -xzf "$file"
cd "${file%.tar.gz}"
```

这些包没有发行者签名；macOS 只有本地 **ad-hoc 签名**，不是 Developer ID 签名，
也没有 Apple 公证。系统可能显示未知发行者、SmartScreen 或 Gatekeeper 警告。
校验和不消除这些信任风险。

- **Windows**：先在 PowerShell 尝试 `.\tonefit.exe --version`。如出现 SmartScreen，
  核对所选文件后，可在该次提示选「更多信息 → 仍要运行」（系统提供时）。
  若文件属性中有「解除锁定」，也只对已校验的这一份归档使用，再重新解压。
  不关闭 SmartScreen、Defender 或配置管理员白名单；组织策略不允许放行时停止。
- **macOS**：先在终端尝试 `./tonefit --version`。若被 Gatekeeper 阻止，
  到「系统设置 → 隐私与安全性」，核对刚才被阻止的程序，再选「仍要打开」并确认。
  若命令行程序无法经该界面放行，且已核验来源和 SHA256，可仅删除当前解压程序的隔离属性：
  `xattr -d com.apple.quarantine ./tonefit`，然后重试。
  不递归清理下载目录、不使用 `sudo`、不全局关闭 Gatekeeper；签名损坏等其他错误应停止排查。
- **Linux**：归档保留执行位，正常可直接 `./tonefit --version`。
  若解压工具丢失执行位，可只对这份已校验程序执行 `chmod u+x ./tonefit`。
  若目录挂载为 `noexec` 或系统策略阻止执行，改用允许运行程序的个人目录或咨询管理员，
  不全局修改安全策略。

## 开始使用

进入解压目录后执行；不要依赖程序已经加入 PATH。

Windows PowerShell：

```powershell
.\tonefit.exe --version
.\tonefit.exe --help
.\tonefit.exe
.\tonefit.exe --profile kobo-libra-2 --out 'D:\转好的' 'D:\漫画库\某作品'
.\tonefit.exe calibrate --profile kobo-libra-2 --out '灰阶测试图.png'
```

Linux / macOS：

```sh
./tonefit --version
./tonefit --help
./tonefit
./tonefit --profile kobo-libra-2 --out "$HOME/转好的" "$HOME/漫画库/某作品"
./tonefit calibrate --profile kobo-libra-2 --out '灰阶测试图.png'
```

`--version` 应与所选 Release 一致，例如该版本为 `tonefit 0.1.0-alpha.1`。
无参数启动进入交互会话（TUI），需在交互终端里运行；带参数直接走 CLI。
示例输入可换成目录或归档（包括 `.rar`），输出目录请放在源库之外；源库只读。
参数与退出码见 [README《用法》](../README.md#用法) 和 [《退出码》](../README.md#退出码)；
测试图须在设备上以原尺寸打开，判读见 [《标定自己的设备》](../README.md#标定自己的设备)。

## 包内说明与来源

解压目录的 `README.txt` 是随包使用/系统提示，`provenance.json` 记录源 commit、tag、
目标、Rust/native 工具与依赖版本、锁文件哈希及构建状态；`manifest.json` 是包内文件哈希清单。
这些信息便于追溯，不是发行者签名。`LICENSE`、`THIRD-PARTY-NOTICES.md`、
`dependency-licenses.json` 与 `licenses/` 保留本体和第三方许可，转分发时请一起保留；
UnRAR 许可含禁止用于反推 RAR 压缩算法的限制。

原生审计/运行环境记录 `verification.json` 随对应 Actions 验收 artifact 保存，
不属于 Release 的五份下载资产，也不在归档内；不要把找不到它误认为需要另装运行库。
Release 资产应为上表四个包与一个全局 `SHA256SUMS`。

//! 介质：一条读取通道落在什么盘上，以及据此派几条并发去读。
//!
//! ADR 0009 决定第 2 条：**介质按路径探测**。给定一个路径，先解析到它所在的卷/挂载点
//! （本模块叫它[读取通道](Channel)），再查那条通道的寻道惩罚。同一次运行里不同路径各自判定、
//! 互不影响——一台机器的存储是混合的，「这台机器是 HDD 还是 SSD」这个问题没有答案。
//!
//! 探测**按通道缓存**：每页查一次挂载表不可接受（ADR 0009 的《后果》）。缓存只活在一次运行
//! 之内，运行期间挂载点变化、符号链接跨卷会让结论失真——那是同一段认下的代价。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::source::Container;

/// 一条读取通道的标识：路径解析到的那个卷/挂载点。
///
/// 是个字符串而不是一个平台类型，因为两边给出的东西本来就不同形：Windows 那边是挂载点路径
/// （`C:\`、`\\nas\share\`），Linux 那边是块设备号（`8:0`）。上层只拿它当缓存的键，
/// 不去解释它的内容。
type Channel = String;

/// 一条读取通道的介质。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Medium {
    /// 有寻道惩罚：机械盘。并发随机读会互相打断寻道，吞吐反而低于串行（ADR 0009）。
    Seeking,
    /// 无寻道惩罚：固态盘。并发是纯收益。
    Solid,
    /// 探不出结论。网络路径与 NAS 落在这里，探测本身失败也落在这里——两者都不假装自己是
    /// 本地盘的某一种（ADR 0009 决定第 3 条）。
    ///
    /// `reason` 是探测在哪一步停下的那句话。它进报告：退到保守策略这件事要说得出为什么
    /// （13 号票），否则用户看到的只是「跑得慢」。
    Unknown { reason: String },
}

impl std::fmt::Display for Medium {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Medium::Seeking => f.write_str("有寻道惩罚（机械盘）"),
            Medium::Solid => f.write_str("无寻道惩罚（固态盘）"),
            Medium::Unknown { reason } => write!(f, "未知（{reason}）"),
        }
    }
}

/// `--io-mode`：读取策略，覆盖自动探测。
///
/// 取值说的是**怎么读**，不是**盘是什么**：用户改不了盘的物理事实，改得了的是这一趟的策略。
/// ADR 0009 的备选方案里被否掉的那一条是「让用户声明介质类型」，它否的是拿声明**代替**探测；
/// 这里是探测之上的一道覆盖，NAS 上探测按未知退到串行、而用户实测它并发更快时，出口在这儿。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IoMode {
    /// 按路径探测介质，据此定读取策略。
    #[default]
    Auto,
    /// 不论介质，读取串行。
    Serial,
    /// 不论介质，读取并发。
    Concurrent,
}

impl IoMode {
    /// 按 `--io-mode` 的写法解析。取值集合不进 CLI 的类型，库这一侧对 CLI 无知——
    /// 与 `--filter`、`--dither` 同一套分工。
    pub fn resolve(name: &str) -> Result<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(IoMode::Auto),
            "serial" => Ok(IoMode::Serial),
            "concurrent" => Ok(IoMode::Concurrent),
            _ => anyhow::bail!(
                "认不出 I/O 模式 {name}：写 auto（按路径探测介质）、serial（读取串行）\
                 或 concurrent（读取并发）"
            ),
        }
    }
}

/// 读取并发度的出处。报告要说得清这个数是谁定的——不然「为什么这一卷读得慢」没有答案。
///
/// 与 [`IoPlan::readers`] 那个数分开命名：一个是**几条**，一个是**谁定的**，
/// 同一个词担两件事，读的人迟早会把它们看成一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChosenBy {
    /// 按探测到的介质定的。
    Probe,
    /// `--io-mode` 点名的。
    Named,
    /// 归档卷：一个 `ZipArchive` 就是一个游标，读取恒串行。
    ArchiveScan,
}

/// 一个卷这一趟怎么读：介质是什么，据此派几条读取。
#[derive(Debug, Clone)]
pub struct IoPlan {
    /// 源路径落在什么介质上。`--io-mode` 点名时这里照实说——覆盖的是策略，不是事实。
    pub medium: Medium,
    /// 这一趟派几条读取。1 即串行。
    pub readers: usize,
    /// 这个数的出处。
    pub chosen_by: ChosenBy,
}

impl IoPlan {
    /// 定下这一卷的读取策略。
    ///
    /// 三道顺序：归档卷恒串行 → `--io-mode` 点名的作数 → 剩下的按介质定。
    ///
    /// **归档卷排在覆盖项前面**，因为它不是一个策略选择：一个 `ZipArchive` 就是一个游标，
    /// 并发读同一个游标根本无从谈起。这不是性能上的让步——归档成员本来就按顺序码在一个文件里，
    /// 顺序扫是它最快的读法，而读取与计算的重叠由有界通道负责（见 `crate::read`），
    /// 不靠多开几条读取去买。
    ///
    /// **未知按有惩罚办**（ADR 0009 决定第 3 条的「保守并发度」）：并发在机械盘上是真损失，
    /// 在别的介质上只是没赚到。NAS 的最优策略尚未测量（`CONTEXT.md` 的《尚未确立》），
    /// 想要并发的用户走 `--io-mode concurrent`。
    pub(crate) fn decide(medium: Medium, mode: IoMode, container: Container, cores: usize) -> Self {
        if container == Container::Archive {
            return Self {
                medium,
                readers: 1,
                chosen_by: ChosenBy::ArchiveScan,
            };
        }
        let concurrent = cores.max(1);
        let (readers, chosen_by) = match (mode, &medium) {
            (IoMode::Serial, _) => (1, ChosenBy::Named),
            (IoMode::Concurrent, _) => (concurrent, ChosenBy::Named),
            (IoMode::Auto, Medium::Solid) => (concurrent, ChosenBy::Probe),
            (IoMode::Auto, Medium::Seeking | Medium::Unknown { .. }) => (1, ChosenBy::Probe),
        };
        Self {
            medium,
            readers,
            chosen_by,
        }
    }
}

impl std::fmt::Display for IoPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "介质 {} · 读取", self.medium)?;
        match self.readers {
            1 => f.write_str("串行")?,
            readers => write!(f, "并发 {readers}")?,
        }
        match self.chosen_by {
            ChosenBy::Probe => Ok(()),
            ChosenBy::Named => f.write_str("（--io-mode 点名）"),
            ChosenBy::ArchiveScan => f.write_str("（归档卷是一条顺序扫）"),
        }
    }
}

/// 按通道缓存的介质探测（ADR 0009 的《后果》：每页查一次挂载表不可接受）。
///
/// 一次运行建一个，逐卷问它。平台那两步是可换的函数指针：探测要碰真实存储栈，
/// 而「同一次运行里不同路径可得不同结论」「同一条通道只探一次」这两条性质与碰的是哪块盘无关，
/// 换一对假的进来就测得了。
pub(crate) struct Probes {
    /// 路径 → 它所在的读取通道。便宜，每卷做一次。
    channel: fn(&Path) -> Result<Channel>,
    /// 通道 → 有没有寻道惩罚。贵（要开设备、读 sysfs），按通道缓存。
    penalty: fn(&Channel) -> Result<bool>,
    seen: HashMap<Channel, Medium>,
}

impl Probes {
    /// 建一个探测器，走本平台那一对。
    pub(crate) fn new() -> Self {
        Self::with(platform::channel, platform::seek_penalty)
    }

    fn with(channel: fn(&Path) -> Result<Channel>, penalty: fn(&Channel) -> Result<bool>) -> Self {
        Self {
            channel,
            penalty,
            seen: HashMap::new(),
        }
    }

    /// 这个路径落在什么介质上。探不出来就是 [`Medium::Unknown`]，连同它停在哪一步。
    pub(crate) fn medium(&mut self, path: &Path) -> Medium {
        let channel = match (self.channel)(path) {
            Ok(channel) => channel,
            Err(error) => {
                return Medium::Unknown {
                    reason: format!("{error:#}"),
                };
            }
        };
        if let Some(medium) = self.seen.get(&channel) {
            return medium.clone();
        }
        let medium = match (self.penalty)(&channel) {
            Ok(true) => Medium::Seeking,
            Ok(false) => Medium::Solid,
            Err(error) => Medium::Unknown {
                reason: format!("{error:#}"),
            },
        };
        self.seen.insert(channel, medium.clone());
        medium
    }
}

/// 平台那一对：路径 → 通道，通道 → 寻道惩罚。
///
/// 分成两步不是为了好看：第一步便宜、每卷一次，第二步贵、按通道缓存（ADR 0009 的《后果》）。
/// 合成一步的话缓存就无处可放——探测本身已经把贵的那一半做完了。
///
/// 探不出来一律是 `Err`，上层退到保守策略并把这句话写进报告（13 号票）。这里因此**不猜**：
/// 网络路径、认不出的挂载点、系统不给答案，三种都是「未知」，不是「大概是固态盘」。
#[cfg(windows)]
mod platform {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;

    use anyhow::{Result, bail};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, GetDriveTypeW,
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::{
        DEVICE_SEEK_PENALTY_DESCRIPTOR, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
        STORAGE_PROPERTY_QUERY, StorageDeviceSeekPenaltyProperty,
    };
    use windows_sys::Win32::System::WindowsProgramming::{DRIVE_REMOTE, DRIVE_UNKNOWN};

    /// 路径所在的挂载点：`C:\`，文件夹挂载的 `C:\mnt\data\`，UNC 的 `\\nas\share\`。
    ///
    /// 拿它当通道标识而不是拿盘符：文件夹挂载点下面是**另一块**盘，按盘符缓存会把它的结论
    /// 记到宿主盘头上。
    pub(super) fn channel(path: &Path) -> Result<String> {
        let wide = wide(path.as_os_str());
        let mut buffer = vec![0u16; 512];
        // 路径不必存在也答得出来——输出根在第一趟运行时还没建出来。
        let ok = unsafe {
            GetVolumePathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) != 0
        };
        if !ok {
            bail!("{} 解析不到所在的卷", path.display());
        }
        Ok(from_wide(&buffer))
    }

    /// 这条通道有没有寻道惩罚。
    ///
    /// 走的是存储栈自己的答案（`IOCTL_STORAGE_QUERY_PROPERTY` + `StorageDeviceSeekPenaltyProperty`），
    /// 不去认型号名、不去看转速表。查询句柄按 0 访问权限打开，因此**不需要管理员权限**。
    pub(super) fn seek_penalty(channel: &String) -> Result<bool> {
        let mount = wide(OsString::from(channel).as_os_str());
        // 网络盘与认不出类型的盘当场退出：它们后面那条设备查询即便答得出来，
        // 答的也是本地某个转发层的事，不是数据真正待着的地方（ADR 0009 决定第 3 条）。
        match unsafe { GetDriveTypeW(mount.as_ptr()) } {
            DRIVE_REMOTE => bail!("{channel} 是网络路径，介质无从探测"),
            DRIVE_UNKNOWN => bail!("{channel} 的驱动器类型认不出来"),
            _ => {}
        }
        // 卷的 GUID 名对盘符与文件夹挂载点一视同仁，`\\?\Volume{…}` 去掉末尾反斜杠即设备名。
        let mut buffer = vec![0u16; 128];
        let ok = unsafe {
            GetVolumeNameForVolumeMountPointW(
                mount.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            ) != 0
        };
        if !ok {
            bail!("{channel} 取不到卷名");
        }
        let device = from_wide(&buffer);
        let device = device.trim_end_matches('\\');

        let handle = unsafe {
            CreateFileW(
                wide(OsString::from(device).as_os_str()).as_ptr(),
                // 0 访问权限：只查属性，不读也不写，因此不要管理员权限。
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            bail!("{device} 打不开：{}", std::io::Error::last_os_error());
        }
        let answer = ask(handle);
        unsafe { CloseHandle(handle) };
        answer
    }

    /// 问一个已打开的卷句柄要寻道惩罚描述符。
    fn ask(handle: windows_sys::Win32::Foundation::HANDLE) -> Result<bool> {
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceSeekPenaltyProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0],
        };
        let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR {
            Version: 0,
            Size: 0,
            IncursSeekPenalty: false,
        };
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                std::ptr::from_ref(&query).cast(),
                size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                std::ptr::from_mut(&mut descriptor).cast(),
                size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
                &mut returned,
                std::ptr::null_mut(),
            ) != 0
        };
        if !ok {
            bail!("存储栈不回答寻道惩罚：{}", std::io::Error::last_os_error());
        }
        if (returned as usize) < size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() {
            bail!("寻道惩罚描述符只回了 {returned} 字节");
        }
        Ok(descriptor.IncursSeekPenalty)
    }

    /// Rust 的字符串转成以 NUL 收尾的宽字符串。
    fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
        text.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// 宽字符串缓冲转回 Rust 的字符串，截到第一个 NUL。
    fn from_wide(buffer: &[u16]) -> String {
        let end = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        OsString::from_wide(&buffer[..end])
            .to_string_lossy()
            .into_owned()
    }
}

/// 同上，Linux 那一对：块设备号 → sysfs 里的 `queue/rotational`。
///
/// 网络文件系统（NFS、SMB）与 overlay 的设备号在 `/sys/dev/block` 下没有对应项，
/// 于是自然落到未知那一支——不必单独认它们。
#[cfg(target_os = "linux")]
mod platform {
    use std::os::linux::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result, bail};

    /// 路径所在块设备的 `主:次` 号。路径不必存在——上溯到最近的已存在祖先再问。
    pub(super) fn channel(path: &Path) -> Result<String> {
        let mut current = path;
        loop {
            if let Ok(metadata) = std::fs::metadata(current) {
                let device = metadata.st_dev();
                return Ok(format!("{}:{}", major(device), minor(device)));
            }
            current = current
                .parent()
                .with_context(|| format!("{} 与它的祖先都问不到设备号", path.display()))?;
        }
    }

    /// 这条通道有没有寻道惩罚：sysfs 里的 `queue/rotational`。
    ///
    /// 分区自己没有 `queue`，那一项挂在整块盘上，因此从分区节点往上找一层。
    pub(super) fn seek_penalty(channel: &String) -> Result<bool> {
        let node = PathBuf::from(format!("/sys/dev/block/{channel}"))
            .canonicalize()
            .with_context(|| format!("{channel} 在 /sys/dev/block 下没有对应项"))?;
        let mut current = node.as_path();
        loop {
            let rotational = current.join("queue/rotational");
            if let Ok(text) = std::fs::read_to_string(&rotational) {
                return match text.trim() {
                    "0" => Ok(false),
                    "1" => Ok(true),
                    other => bail!("{} 写着认不出的 {other}", rotational.display()),
                };
            }
            current = match current.parent() {
                // 上到 /sys/devices 之外就别再找了：那已经不是这块盘的事。
                Some(parent) if parent.starts_with("/sys") => parent,
                _ => bail!("{channel} 上找不到 queue/rotational"),
            };
        }
    }

    /// Linux 的 `dev_t` 拆成主设备号。
    fn major(device: u64) -> u64 {
        ((device >> 8) & 0xfff) | ((device >> 32) & !0xfff)
    }

    /// 同上，次设备号。
    fn minor(device: u64) -> u64 {
        (device & 0xff) | ((device >> 12) & !0xff)
    }
}

/// 别的平台上探测这件事还没有实现：一律未知，因此一律保守。
///
/// 不是「大概是固态盘」——猜错的那一半正是 ADR 0009 要避开的东西。
#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use std::path::Path;

    use anyhow::{Result, bail};

    pub(super) fn channel(path: &Path) -> Result<String> {
        Ok(path.display().to_string())
    }

    pub(super) fn seek_penalty(_channel: &String) -> Result<bool> {
        bail!("本平台还没有介质探测")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// 假的平台层：路径头一段就是它所在的通道，通道名说了它是什么盘。
    fn channel(path: &Path) -> Result<Channel> {
        let text = path.to_string_lossy();
        for known in ["hdd", "ssd", "nas"] {
            if text.contains(known) {
                return Ok(known.to_owned());
            }
        }
        anyhow::bail!("{text} 解析不到所在的卷")
    }

    thread_local! {
        /// 本条用例探了多少次寻道惩罚。缓存那一条靠它量。
        ///
        /// 挂在线程上而不是进程上：用例并行跑，一个进程级的计数器会把别条用例的探测算进来。
        static PROBED: Cell<usize> = const { Cell::new(0) };
    }

    fn penalty(channel: &Channel) -> Result<bool> {
        PROBED.with(|probed| probed.set(probed.get() + 1));
        match channel.as_str() {
            "hdd" => Ok(true),
            "ssd" => Ok(false),
            _ => anyhow::bail!("{channel} 是网络路径，介质无从探测"),
        }
    }

    fn probes() -> Probes {
        Probes::with(channel, penalty)
    }

    /// 同一次运行里，落在两块盘上的两个路径各自得到自己的结论（ADR 0009 决定第 2 条）。
    ///
    /// 这是本模块存在的理由：按次运行探测一次，混合存储下必然对一半路径给出错误答案。
    #[test]
    fn two_paths_in_one_run_can_land_on_different_media() {
        let mut probes = probes();

        assert_eq!(probes.medium(Path::new("/hdd/仓库/卷一")), Medium::Seeking);
        assert_eq!(probes.medium(Path::new("/ssd/热数据/卷二")), Medium::Solid);
    }

    /// 同一条通道只探一次，无论有多少个路径落在它上面（ADR 0009 的《后果》：
    /// 每页查一次挂载表不可接受）。
    #[test]
    fn a_channel_is_probed_once_however_many_paths_land_on_it() {
        let mut probes = probes();

        for volume in ["卷一", "卷二", "卷三"] {
            assert_eq!(
                probes.medium(&Path::new("/ssd").join(volume)),
                Medium::Solid
            );
        }

        assert_eq!(PROBED.with(Cell::get), 1, "同一条通道被反复探测");
    }

    /// 探不出来的路径退到保守策略，且报告说得出它停在哪一步（13 号票）。
    #[test]
    fn a_path_that_cannot_be_probed_falls_back_to_serial_and_says_why() {
        let mut probes = probes();

        let medium = probes.medium(Path::new("//nas/share/卷一"));

        let Medium::Unknown { reason } = &medium else {
            panic!("网络路径不该被判成本地盘的某一种：{medium}");
        };
        assert!(reason.contains("网络路径"), "{reason}");
        let plan = IoPlan::decide(medium, IoMode::Auto, Container::Directory, 8);
        assert_eq!(plan.readers, 1, "未知的介质该退到串行");
        assert!(plan.to_string().contains("网络路径"), "{plan}");
    }

    /// 解析不到卷的路径同样是未知——不猜。
    #[test]
    fn a_path_whose_channel_is_unresolvable_is_unknown_too() {
        let mut probes = probes();

        let medium = probes.medium(Path::new("/什么都不是/卷一"));

        assert!(matches!(medium, Medium::Unknown { .. }), "{medium}");
    }

    /// `--io-mode` 两个方向都覆盖得了自动探测（13 号票）。
    #[test]
    fn io_mode_overrides_the_probe_in_both_directions() {
        let serial = IoPlan::decide(Medium::Solid, IoMode::Serial, Container::Directory, 8);
        assert_eq!(serial.readers, 1);
        assert_eq!(serial.chosen_by, ChosenBy::Named);
        // 覆盖的是策略，不是事实：探到的介质照实说。
        assert_eq!(serial.medium, Medium::Solid);

        let concurrent =
            IoPlan::decide(Medium::Seeking, IoMode::Concurrent, Container::Directory, 8);
        assert_eq!(concurrent.readers, 8);
        assert_eq!(concurrent.chosen_by, ChosenBy::Named);
        assert_eq!(concurrent.medium, Medium::Seeking);
    }

    /// 自动那一档：有寻道惩罚的串行，无寻道惩罚的并发（13 号票头两条）。
    #[test]
    fn a_seek_penalty_reads_serially_and_a_solid_disk_reads_concurrently() {
        let seeking = IoPlan::decide(Medium::Seeking, IoMode::Auto, Container::Directory, 8);
        assert_eq!(seeking.readers, 1);
        assert_eq!(seeking.chosen_by, ChosenBy::Probe);
        assert!(seeking.to_string().contains("读取串行"), "{seeking}");

        let solid = IoPlan::decide(Medium::Solid, IoMode::Auto, Container::Directory, 8);
        assert_eq!(solid.readers, 8);
        assert_eq!(solid.chosen_by, ChosenBy::Probe);
        assert!(solid.to_string().contains("读取并发 8"), "{solid}");
    }

    /// 归档卷恒串行：一个 `ZipArchive` 就是一个游标，点名并发也改不了这件事。
    #[test]
    fn an_archive_reads_on_one_channel_however_it_is_asked_to() {
        for mode in [IoMode::Auto, IoMode::Concurrent] {
            let plan = IoPlan::decide(Medium::Solid, mode, Container::Archive, 8);
            assert_eq!(plan.readers, 1, "{mode:?}");
            assert_eq!(plan.chosen_by, ChosenBy::ArchiveScan, "{mode:?}");
            assert!(plan.to_string().contains("顺序扫"), "{plan}");
        }
    }

    /// 本平台那一对真的跑得起来：拿本仓库自己的路径探一次。
    ///
    /// 不断言探出的是哪一种——那取决于跑用例的这台机器装的是什么盘。要钉住的是别的：
    /// 这条路径上的 FFI 不会崩，而无论答案是什么，它都变得成一个说得出口的结论。
    #[test]
    fn the_real_platform_probe_answers_something_about_this_machine() {
        let mut probes = Probes::new();

        let medium = probes.medium(Path::new("."));

        let plan = IoPlan::decide(medium, IoMode::Auto, Container::Directory, 8);
        assert!(plan.readers >= 1);
        assert!(plan.to_string().starts_with("介质 "), "{plan}");
        // 探得出来的那两种才谈得上并发；未知一律串行。
        assert_eq!(plan.readers > 1, plan.medium == Medium::Solid, "{plan}");
    }

    /// `--io-mode` 的三个写法，认不出的当场被挡下。
    #[test]
    fn io_mode_reads_as_auto_serial_or_concurrent() {
        assert_eq!(IoMode::resolve("auto").expect("auto"), IoMode::Auto);
        assert_eq!(IoMode::resolve("Serial").expect("Serial"), IoMode::Serial);
        assert_eq!(
            IoMode::resolve(" concurrent ").expect("concurrent"),
            IoMode::Concurrent
        );
        for text in ["", "ssd", "parallel", "1"] {
            assert!(IoMode::resolve(text).is_err(), "{text} 不该解析出 I/O 模式");
        }
    }
}

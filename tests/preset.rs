//! 预设，在**真进程**上测（p1-session 的 07 号票）。
//!
//! 格式本身、往返、优先级那几条在二进制 crate 内的单元测里（`src/preset.rs` 与
//! `src/main.rs` 的 `mod tests`），那是 spec 的《Testing Decisions》给预设指的家。
//! 这里只留在别处观察不到的那几条：
//!
//! - **找得到那份文件**。用户配置目录、文件名、`--preset` 与它的接线，只有真跑一趟才走得到。
//! - **不点名就不读盘**。工作区里摆一份读不懂的预设，不点名的那一趟照样跑完——
//!   「同一条命令在两台机器上行为相同」说的就是这件事。
//! - **参数哈希收的是展开后的值**。哈希落在输出 PNG 的 tEXt 里（`tonefit:params`），
//!   而那串字节只有写出来之后才读得到：套预设那一趟与逐个敲 flag 那一趟必须写出**同一个**
//!   哈希，改了预设的内容而名字没变必须换一个。
//!
//! 每一趟用的都是一张最小的页：这里问的是参数怎么合出来的，不是像素。

mod fixtures;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use fixtures::{Volume, Workspace};

/// 一份预设文件的正文，装着「漫画」这一个预设。
///
/// CJK 的预设名在 TOML 里要加引号——裸键只收字母、数字、`-`、`_`。
const PRESETS: &str = "\
[preset.\"漫画\".device]
profile = \"kobo-libra-2\"
gray-levels = 12

[preset.\"漫画\".taste]
fit = \"inside\"
filter = \"hamming\"
cache-budget = \"64M\"
";

/// 与 [`PRESETS`] 那一份等价的一串 flag。
const TYPED_OUT: &[&str] = &[
    "--profile",
    "kobo-libra-2",
    "--gray-levels",
    "12",
    "--fit",
    "inside",
    "--filter",
    "hamming",
    "--cache-budget",
    "64M",
];

/// 参数哈希收的是**展开后的值**，不是预设的名字（07 号票的验收）。
///
/// 两趟写出的 PNG 里那一项 `tonefit:params` 必须逐字相同：套预设与逐个敲 flag 是同一趟。
/// 这一条在进程那一层才问得出来——哈希不是公开 API，它只在写出去的字节里露一次面。
#[test]
fn a_preset_hashes_to_what_it_expands_to_not_to_its_name() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let config = config_with(&space, PRESETS);

    let by_name = space.out_named("套预设");
    let typed_out = space.out_named("逐个敲");
    succeeds(run(&config, &by_name, &["--preset", "漫画"], volume.path()));
    succeeds(run(&config, &typed_out, TYPED_OUT, volume.path()));

    assert_eq!(
        params_hash(&by_name),
        params_hash(&typed_out),
        "套预设与把那几个 flag 逐个敲出来，参数哈希不一样"
    );
    // 哈希相同不足以说明输出相同——两趟的字节也比一遍。
    assert_eq!(
        std::fs::read(output_page(&by_name)).expect("读输出页"),
        std::fs::read(output_page(&typed_out)).expect("读输出页"),
        "两趟写出的页字节不同"
    );
}

/// 改了预设的内容而名字没变，下一趟必须重做（07 号票的验收）。
///
/// 断言落在**同一个输出根**上：第二趟若被幂等跳过，盘上那一页仍带着第一趟的参数哈希。
/// 哈希换了，就说明那一页是这一趟重新写出来的。
#[test]
fn changing_a_presets_content_without_changing_its_name_redoes_the_volume() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let config = config_with(&space, PRESETS);
    let out = space.out();

    succeeds(run(&config, &out, &["--preset", "漫画"], volume.path()));
    let before = params_hash(&out);

    // 名字一个字没改，滤波器换了一个。
    config_with(&space, &PRESETS.replace("hamming", "bicubic"));
    succeeds(run(&config, &out, &["--preset", "漫画"], volume.path()));

    assert_ne!(
        params_hash(&out),
        before,
        "预设的内容换了，这一卷却被当成没变过跳过了"
    );
}

/// 同一份预设、同一个源，第二趟照旧跳过：上一条断言的「变了就重做」不是靠「每趟都重做」
/// 蒙对的。
#[test]
fn a_rerun_with_the_same_preset_still_skips_the_volume() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let config = config_with(&space, PRESETS);
    let out = space.out();

    succeeds(run(&config, &out, &["--preset", "漫画"], volume.path()));
    let before = params_hash(&out);
    let second = run(&config, &out, &["--preset", "漫画"], volume.path());

    succeeds(second.clone());
    assert_eq!(params_hash(&out), before);
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("跳过"),
        "第二趟没被跳过：{}",
        String::from_utf8_lossy(&second.stdout)
    );
}

/// **不点名就不读盘。** 工作区里那份预设读都读不懂，而不点名的那一趟照样跑完。
///
/// 这是「同一条命令在两台机器上行为相同」的可观察形态：命令行不提预设时，
/// 盘上有什么都影响不到这一趟。
#[test]
fn a_preset_file_that_is_not_named_is_never_even_read() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let config = config_with(&space, "[preset.\"漫画\".taste]\nsharpen = true\n");

    succeeds(run(&config, &space.out(), TYPED_OUT, volume.path()));
}

/// 读不懂的预设当场报错，一页都不做（07 号票的验收）。
#[test]
fn a_preset_that_cannot_be_read_stops_the_run_before_it_starts() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let out = space.out();

    for (what, text) in [
        ("字段过时", "[preset.\"漫画\".taste]\nsharpen = true\n"),
        (
            "型号已删",
            "[preset.\"漫画\".device]\nprofile = \"kobo-libra-9\"\n",
        ),
        ("取值拼错", "[preset.\"漫画\".taste]\nfit = \"cover\"\n"),
        (
            "范围层混了进来",
            "[preset.\"漫画\".taste]\nout = \"别处\"\n",
        ),
    ] {
        let config = config_with(&space, text);

        let output = run(&config, &out, &["--preset", "漫画"], volume.path());

        assert_eq!(output.status.code(), Some(1), "{what} 那一份没被挡下");
        let complaint = String::from_utf8_lossy(&output.stderr);
        assert!(complaint.contains("漫画"), "{what}：{complaint}");
        assert!(!out.exists(), "{what}：报错了却已经写出了东西");
    }
    // 文件根本不在也是一条说得清的错误，而不是静默套默认值。
    let missing = run(
        &space.dir("空空如也"),
        &out,
        &["--preset", "漫画"],
        volume.path(),
    );
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("预设"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
}

/// `--preset` 供了型号时 `-p` 不再必填；其余情况必填照旧（07 号票的验收）。
#[test]
fn a_preset_that_supplies_the_profile_makes_the_required_one_optional() {
    let space = Workspace::new();
    let volume = one_page(&space);
    let config = config_with(&space, PRESETS);

    // 预设供了型号：`-p` 不必填，这一趟跑得完。
    succeeds(run(
        &config,
        &space.out_named("靠预设"),
        &["--preset", "漫画"],
        volume.path(),
    ));

    // 不点预设：必填照旧，clap 那条信息原样出来。
    let bare = run(&config, &space.out_named("光秃秃"), &[], volume.path());
    assert_ne!(bare.status.code(), Some(0), "少了型号却跑起来了");
    assert!(
        String::from_utf8_lossy(&bare.stderr).contains("--profile"),
        "{}",
        String::from_utf8_lossy(&bare.stderr)
    );

    // 点了预设、而那份预设没有型号：clap 放行之后由这一层说话。
    let silent = config_with(&space, "[preset.\"口味\".taste]\nfit = \"inside\"\n");
    let output = run(
        &silent,
        &space.out_named("没型号"),
        &["--preset", "口味"],
        volume.path(),
    );
    assert_eq!(output.status.code(), Some(1));
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("--profile") && complaint.contains("口味"),
        "{complaint}"
    );
}

/// 一个只有一页的目录卷。这些用例问的是参数，不是像素。
fn one_page(space: &Workspace) -> Volume {
    let volume = space.volume("卷一");
    volume.page("001.png", &fixtures::gradient(fixtures::TINY));
    volume
}

/// 把一份预设文件写进工作区里的用户配置目录，返回那个配置目录。
///
/// 位置与 `preset::file` 说的一致：配置目录下的 `tonefit/presets.toml`。
/// 同一个工作区里重复调它就是把那份文件改写一遍——「改了预设内容」那条用例要的正是这个。
fn config_with(space: &Workspace, text: &str) -> PathBuf {
    let config = space.dir("配置");
    let dir = config.join("tonefit");
    std::fs::create_dir_all(&dir).expect("建配置目录");
    std::fs::write(dir.join("presets.toml"), text).expect("写预设文件");
    config
}

/// 跑一趟 tonefit，把用户配置目录指到 `config` 上。
///
/// 指的办法是环境变量，而**只指子进程那一份**：`std::env::set_var` 在 edition 2024 里是
/// `unsafe` 的，而且一个进程内的用例并行跑，改全局环境会互相打架。
fn run(config: &Path, out: &Path, arguments: &[&str], volume: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tonefit"));
    command.arg("--out").arg(out).args(arguments).arg(volume);
    if cfg!(windows) {
        command.env("APPDATA", config);
    } else {
        command.env("XDG_CONFIG_HOME", config);
    }
    command.output().expect("启动 tonefit")
}

/// 这一趟该跑成：不成就把它自己的说法印出来。
fn succeeds(output: Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "这一趟没跑成：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 输出根下那一卷唯一那一页。
fn output_page(out: &Path) -> PathBuf {
    out.join("卷一").join("001.png")
}

/// 输出根下那一页记着的参数哈希。
fn params_hash(out: &Path) -> String {
    let text = fixtures::read_png_text(&output_page(out));
    fixtures::png_field(&text, "tonefit:params").expect("记录里该有参数哈希")
}

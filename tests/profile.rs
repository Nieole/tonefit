//! 面板表这个 seam 上的行为测试。
//!
//! CLI 只把 `--profile` 与 `--gray-levels` 交给这里，所以型号怎么解析、
//! 未知型号说什么，都是外部可见的事实。表本身的完整性在 `src/profile.rs` 里就地断言。

use tonefit::Profile;

#[test]
fn a_model_is_just_an_alias_for_a_panel() {
    // 同一块 7 英寸 300 PPI 面板，三个牌子三个型号：输出对它们完全一致。
    let panels: Vec<_> = ["kobo-libra-2", "boox-page", "kindle-oasis-3"]
        .map(|device| Profile::resolve(device).expect("内置型号").panel())
        .to_vec();

    assert!(
        panels.windows(2).all(|pair| pair[0] == pair[1]),
        "{panels:?}"
    );
}

#[test]
fn a_model_name_is_read_regardless_of_case_and_separators() {
    for spelling in [
        "Kobo Libra 2",
        "KOBO_LIBRA_2",
        "kobo.libra.2",
        " kobo-libra-2 ",
    ] {
        let profile =
            Profile::resolve(spelling).unwrap_or_else(|error| panic!("{spelling}：{error}"));
        assert_eq!(profile.device(), "kobo-libra-2", "{spelling}");
    }
}

#[test]
fn gray_levels_override_the_panels_own_count() {
    let built_in = Profile::resolve("kobo-libra-2").expect("内置型号");

    // 真机上数出来的可分辨级数可以少于面板号称的 16（ADR 0003）。
    let overridden = built_in.clone().with_gray_levels(8).expect("8 级数得出来");

    assert_eq!(overridden.panel().gray_levels, 8);
    // 只动灰阶数：分辨率与 PPI 仍是内置表里那块面板，型号也没变。
    assert_eq!(overridden.panel().resolution, built_in.panel().resolution);
    assert_eq!(overridden.panel().ppi, built_in.panel().ppi);
    assert_eq!(overridden.device(), built_in.device());
}

#[test]
fn a_gray_level_count_no_panel_could_have_is_refused() {
    for levels in [0, 1, 257] {
        match Profile::resolve("kobo-libra-2")
            .expect("内置型号")
            .with_gray_levels(levels)
        {
            Ok(profile) => panic!("{levels} 级不该被收下，却给出了 {profile}"),
            Err(error) => assert!(error.to_string().contains("灰阶数"), "{levels}：{error}"),
        }
    }
}

#[test]
fn an_unknown_model_is_refused_with_the_built_in_list() {
    let error = Profile::resolve("kobo-libre-2")
        .expect_err("未知型号应当报错")
        .to_string();

    assert!(error.contains("kobo-libre-2"), "没说是哪个型号：{error}");
    // 清单要能让用户挑出一个面板相同的型号顶上：型号名与它的面板都得在。
    assert!(error.contains("kobo-libra-2"), "清单里没有型号：{error}");
    assert!(error.contains("1264×1680"), "没给出面板：{error}");
    assert!(error.contains("--gray-levels"), "没给出兜底办法：{error}");
}

#[test]
fn a_product_line_that_changed_panels_is_listed_by_generation() {
    // Paperwhite 11 代是 6.8 英寸，12 代换成了 7 英寸。名字不带代次就会静默给出错的目标尺寸。
    let eleventh = Profile::resolve("kindle-paperwhite-11").expect("内置型号");
    let twelfth = Profile::resolve("kindle-paperwhite-12").expect("内置型号");
    assert_ne!(eleventh.panel(), twelfth.panel());

    // 不带代次的名字不该蒙对一个：它得掉进未知型号那条路，让用户看见清单。
    let error = Profile::resolve("kindle-paperwhite")
        .expect_err("不带代次的名字应当报错")
        .to_string();
    assert!(error.contains("kindle-paperwhite-11"), "{error}");
    assert!(error.contains("kindle-paperwhite-12"), "{error}");
}

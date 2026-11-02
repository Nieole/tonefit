# 24: 迁移剩余库调用者并删除临时门面

**What to build:** 程序、集成测试、doctest、benchmark 与 harness 直接使用 processing／domain 的真实 interface；没有调用者后删除根库门面及过渡装配。最终运行时代码恰好三个 crate，根程序不再同时伪装成兼容处理库。

**Blocked by:** 11「卷编排归位，结束源码位置暂存」。

**Status:** ready-for-agent

**规格覆盖：** task-ddd-reorganization；用户故事：2、3、4、6、7、8、12、17、18、22、73、74、75、76、78。

- [ ] 按实际当前调用清单迁移剩余程序、呈现、集成测试、doctest、benchmark、harness 及 fixture 消费者，使其直接使用真实领域／处理 interface，不要求兼容原平铺 Rust 导出形式。
- [ ] 只在确认没有调用者后删除对应临时重导出、根库门面、旧配置适配及源码位置暂存；仍有其他未完成票消费的窄适配暂不强删，登记其最终退出条件并由 25 核验清零。
- [ ] 验证 root 到 processing 到 domain 的单向依赖，根可直接用领域值；恰好三个运行时 crate，xtask 与交付工具不进入产物依赖，不新增 common、contracts 或 fixture crate。
- [ ] 保持 domain 的 score 及 processing 的 run、write_calibration_chart、write_proof 四道产品 seam；不得为迁移集成测试而批量公开原私有 module，必要私有断言使用所属外置测试装配。
- [ ] 同步整理共享测试源码依赖、Cargo 目标选择、feature 转发、版本来源、开发特性隔离、许可／交付包图及所有源码扫描、测试名、平台证明消费者；默认根构建和 gate/polish 入口保持有效。
- [ ] 按性质索引记录每项测试调用与断言调整，更新已经成立的架构与 module spec、必要 ADR 及确实新增的领域术语；不提前宣称其他深化票已完成。
- [ ] 运行真实调用者、结构／特性守卫、受影响交付脚本检查、cargo xtask gate 与 cargo xtask polish，确认三个运行时包测试实际执行。

## 不做会怎样

过渡门面会固化成长期 shallow module，调用者仍学习旧平铺结构；内部实际拆成多个包也无法带来真实可导航性和单向编译约束。

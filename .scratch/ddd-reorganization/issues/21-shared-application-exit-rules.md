# 21: CLI 与会话共用根程序退出规则

**What to build:** CLI 与会话使用根程序的同一结果 module，把处理报告映射为进程退出结果；main 只装配启动，不再作为其他 adapter 的规则仓库。用户脚本继续得到既有的拒绝、隔离、卷失败和无法访问优先级。

**Blocked by:** 01「完整 workspace 闸门与外置测试装配」。

**Status:** ready-for-agent

**规格覆盖：** task-ddd-reorganization；用户故事：6、9、47、48、78。

- [ ] 将退出常量、报告映射及真实共用结果规则从 main 收入根程序共用 module，CLI 和会话消费同一 interface，互不回调对方入口私有实现。
- [ ] 保持拒绝、隔离、卷失败、无法访问与成功的原优先级及退出方式；processing 只提供分类事实，不决定进程退出。
- [ ] 不顺带修改 CLI 参数、无参分派、输出措辞、报告结构或运行控制；不把共用 Profile 设置解析重新放回根结果 module，设置仍由 12、13 完成。
- [ ] 经真实 CLI 子进程验证输出、错误及退出码组合，并验证会话采用同一结果映射；本票不依赖无关 settings 或输入协调迁移的先后。
- [ ] 同步外置／替换所属测试、辅助与 fixture，按性质索引记录旧覆盖，不为测试扩大生产 interface。
- [ ] 更新实际调用、源码守卫及 owning spec，运行退出码／CLI／会话聚焦检查、cargo xtask gate 与 cargo xtask polish。

## 不做会怎样

会话继续依赖 main 的私有函数，或者各入口各写一份退出政策；处理库还可能错误地承担进程生命周期决定。

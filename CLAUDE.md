# tonefit

## Agent skills

### Issue tracker

Issues and specs live as markdown files under `.scratch/<feature-slug>/` in this repo.
See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage roles, used verbatim as `Status:` strings.
See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` at the repo root plus `docs/adr/`.
See `docs/agents/domain.md`.

## 写代码前

`CONTEXT.md` 定义领域术语——类型名、模块名、测试名、issue 标题一律取自它。

改动触及缩放、判据、位深、抖动、几何裁切、缓存、编码器、处理范围中的任何一项，
先读 `docs/adr/` 里对应的那一篇。

需要实测数字时，来源只有 `docs/measurements.md`。

## 文档写作

agent 消费的文档（`CONTEXT.md`、`docs/`、issue、spec）按这五条写：

1. **结果**——写当前成立的事实与决定。变更史交给 git。
2. **单一职责**——一个文件只承载它那一件事。
3. **渐进披露**——入口给骨架和指路，细节下沉一层。
4. **单一出处**——每条数据、每个结论只有一个权威位置，其余引用它的小节名。
5. **稳定引用**——引用用小节标题或术语，文件改动不使引用失效。

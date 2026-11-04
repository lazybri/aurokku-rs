# 13 · rp2354 分支会话工作规则（用户定义 · 提示词组成部分）

> 定义时间：2026-11-04（Asia/Shanghai）。
> 用户原话：「定义，你只能在rp2354分支内工作，允许在rp2354分支内自动推送迭代，开始推理前需要阅读先前留存的上下文，我最后会人工要求留存上下文。」
> 本文件是 `rp2354` 分支后续会话的**提示词组成部分**：任何会话在开始推理或执行任何改动之前，必须先读本文件，再按 §2 阅读先前留存的上下文。

---

## 1. 规则

1. **分支边界（硬约束）**：只能在 `rp2354` 分支内工作。
   - `rp2354` 的工作树为 `C:/Users/Administrator/Documents/proj/aurokku-rs.worktrees/rp2354`；
   - 不修改、不切换、不提交 `main`、`qt6`、`docs/context`、`libwdi-static` 等其它分支；
   - 需要参考其它分支的内容时，只能只读获取（如 `git show origin/docs/context:docs/plan/12-session-context.md`），不得 checkout 或改动。
2. **自动推送（本分支内授权）**：允许在 `rp2354` 分支内自动提交并推送迭代（commit + `push origin rp2354`），无需逐次征求用户同意。
   - 该授权只覆盖 `rp2354` 分支；其它任何分支、tag、GitHub Release 的推送仍需用户明确同意；
   - 每次推送前必须完成本分支内可执行的验证（格式化 / 构建 / 测试 / 目标三元组检查），失败不得推送。
3. **先读上下文再推理**：开始推理前必须阅读先前留存的上下文，至少包括：
   - 本文件；
   - `docs/plan/` 下本分支已有的规划与上下文文档（当前为 06–10）；
   - `docs/context` 分支上的规划与上下文文档（当前为 00–05、11、12），只读；
   - 阅读完成后先确认「当前仓库状态 / 已完成项 / 下一步」，再开始改动。
4. **上下文留存由用户触发**：会话上下文的留存（撰写新的会话上下文文档）由用户在最后**人工要求**；在此之前不主动新建「上下文 / 会话总结」类文档，也不得把本规则以外的运行记录写入上下文文档。
5. **冲突处理**：本文件与更早的默认约定不一致时，以本文件为准（尤其是原本「rp2354 推送需先征询」的旧约定，已被第 2 条覆盖）。

---

## 2. 先前留存上下文索引

| # | 文档 | 所在分支 | 说明 |
|---|---|---|---|
| 00 | `docs/plan/00-merge-and-libwdi-plan.md` | docs/context | 总体执行计划（已执行完毕） |
| 01 | `docs/plan/01-main-history-0.2.0-0.3.1.md` | docs/context | main 提交史与移植单元 |
| 02 | `docs/plan/02-qt6-gap-analysis.md` | docs/context | qt6 架构地图与冲突单元 |
| 03 | `docs/plan/03-libwdi-decoupling.md` | docs/context | libwdi 解耦设计 |
| 04 | `docs/plan/04-session-context.md` | docs/context | Windows 驱动栈 + CI 上下文 |
| 05 | `docs/plan/05-session-context.md` | docs/context | 差距分析 + libwdi 规划上下文 |
| 06 | `docs/plan/06-rp2354-port-feasibility.md` | **rp2354** | RP2354 移植可行性总报告 |
| 07 | `docs/plan/07-rp2354-platform-and-toolchain.md` | **rp2354** | 平台 / USB host / 自制板 |
| 08 | `docs/plan/08-core-portability.md` | **rp2354** | 核心库 no_std 可移植性与 U1–U6 |
| 09 | `docs/plan/09-oled-buttons-ui.md` | **rp2354** | OLED + 按键交互设计 |
| 10 | `docs/plan/10-core-nostd-spike.md` | **rp2354** | no_std 核心抽取 spike 与验证 |
| 11 | `docs/plan/11-session-context.md` | docs/context | RP2354 研究拆分与结论 |
| 12 | `docs/plan/12-session-context.md` | docs/context | 00–05 执行记录（libwdi / 0.3.2 / qt6） |
| 13 | `docs/plan/13-session-rules.md` | **rp2354** | 本文件（工作规则） |

> 说明：本文件占用编号 13；用户之后人工要求留存的会话上下文文档应顺延为 14、15……。

---

## 3. 生效方式

- 每次会话开始（首条消息 / 恢复会话 / fork 后）按 §1.3 阅读上下文，并把本文件作为提示词的一部分。
- 本文件只在 `rp2354` 分支内生效；其它分支的历史推送策略见 docs/context 分支 `docs/plan/README.md`。

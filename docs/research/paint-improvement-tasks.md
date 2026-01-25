# Zeterm 字符绘制改进任务分析

> 基于 Zed 终端渲染逻辑的对比分析
> 创建日期: 2026-01-13

---

## 一、概述

### 1.1 当前状态

Zeterm 已实现基础的字符绘制功能，但与 Zed 相比存在显著的性能差距：

| 维度 | Zed | Zeterm | 差距 |
|------|-----|--------|------|
| 文本批处理 | ✅ BatchedTextRun | ❌逐字符绘制 | **严重** |
| 背景合并 | ✅ BackgroundRegion | ❌ 逐单元格绘制 | **严重** |
| 视口裁剪 | ✅ 只渲染可见区域 | ❌ 渲染所有内容 | **中等** |
| 对比度调整 | ✅ ensure_minimum_contrast | ❌ 无 | **轻微** |

### 1.2 性能影响估算

假设 80x24 终端，50% 单元格有内容：

| 操作 | Zeterm 当前 | Zed 优化后 | 差距 |
|------|-------------|------------|------|
| shape_line 调用 | ~960 次 | ~50-100 次 | **10-20x** |
| paint_quad 调用 | ~500+ 次 | ~10-50 次 | **10-50x** |

### 1.3 改进目标

1. **性能目标**：绘制调用次数减少 80% 以上
2. **功能目标**：支持选择高亮、搜索匹配、对比度调整
3. **代码质量**：保持代码简洁，易于维护

---

## 二、高优先级任务(P0)

### 2.1 实现文本批处理 (BatchedTextRun)

**任务描述**：将相邻的、样式相同的字符合并成批次，减少 `shape_line()` 调用。

**预估时间**：4-6 小时

**实现步骤**：

1. 创建 `BatchedTextRun` 结构体
2. 实现 `can_append()` 方法判断样式是否相同
3. 实现 `append_char()` 方法追加字符
4. 实现 `paint()` 方法绘制批量文本
5. 修改 `paint()` 方法使用批处理逻辑

**参考代码**：

```rust
pub struct BatchedTextRun {
    pub start_point: Point<i32>,
    pub text: String,
    pub cell_count: usize,
    pub style: TextRun,
    pub font_size: Pixels,
}

impl BatchedTextRun {
    fn can_append(&self, other_style: &TextRun) -> bool {
        self.style.font == other_style.font
            && self.style.color == other_style.color
            && self.style.underline == other_style.underline
            && self.style.strikethrough == other_style.strikethrough
    }

    fn append_char(&mut self, c: char) {
        self.text.push(c);
        self.cell_count += 1;
        self.style.len += c.len_utf8();
    }
}
```

**验收标准**：
- [ ] 相同样式的相邻字符被合并
- [ ] shape_line 调用次数减少 80% 以上
- [ ] 渲染结果与优化前一致

---

### 2.2 实现背景区域合并 (BackgroundRegion)

**任务描述**：合并相邻的背景区域，减少 `paint_quad()` 调用。

**预估时间**：3-4 小时

**实现步骤**：

1. 创建 `BackgroundRegion` 结构体
2. 实现 `can_merge_with()` 方法
3. 实现 `merge_with()` 方法
4. 实现 `merge_background_regions()` 合并算法
5. 创建 `LayoutRect` 结构体用于绘制

**参考代码**：

```rust
struct BackgroundRegion {
    start_line: i32,
    start_col: i32,
    end_line: i32,
    end_col: i32,
    color: Hsla,
}

impl BackgroundRegion {
    fn can_merge_with(&self, other: &Self) -> bool {
        if self.color != other.color {
            return false;
        }
        // 水平相邻或垂直相邻
        // ...
    }
}
```

**验收标准**：
- [ ] 相邻的相同颜色背景被合并
- [ ] paint_quad 调用次数减少 50% 以上
- [ ] 渲染结果与优化前一致

---

### 2.3 重构绘制流程 (prepaint/paint 分离)

**任务描述**：将预处理逻辑从`paint()` 移到 `prepaint()`，实现预处理与绘制分离。

**预估时间**：2-3 小时

**实现步骤**：

1. 扩展 `LayoutState` 结构体，添加预处理数据字段
2. 在 `prepaint()` 中执行批处理和背景合并
3. 在 `paint()` 中只执行绘制操作

**新的 LayoutState**：

```rust
pub struct LayoutState {
    pub background_color: Hsla,
    pub font_metrics: FontMetrics,
    pub cols: usize,
    pub rows: usize,
    // 新增字段
    pub batched_text_runs: Vec<BatchedTextRun>,
    pub background_rects: Vec<LayoutRect>,
    pub cursor_layout: Option<CursorLayout>,
}
```

**验收标准**：
- [ ] prepaint() 完成所有预处理
- [ ] paint() 只执行绘制调用
- [ ] 代码结构清晰，易于维护

---

## 三、中优先级任务 (P1)

### 3.1 实现视口裁剪优化

**任务描述**：只渲染可见区域的单元格，跳过视口外的内容。

**预估时间**：2-3 小时

**实现步骤**：

1. 在 `prepaint()` 中计算可见区域边界
2. 计算可见行范围 (rows_above_viewport, visible_row_count)
3. 过滤单元格迭代器，只处理可见行
4. 处理边界情况（终端完全在视口外）

**验收标准**：
- [ ] 滚动时只渲染可见区域
- [ ] 终端在视口外时跳过处理
- [ ] 滚动性能明显提升

---

### 3.2 添加选择高亮支持

**任务描述**：支持文本选择的高亮显示。

**预估时间**：3-4 小时

**实现步骤**：

1. 在 `LayoutState` 中添加 `highlighted_ranges` 字段
2. 从终端内容获取选择范围
3. 实现 `to_highlighted_range_lines()` 转换函数
4. 在 `paint()` 中绘制高亮区域

**验收标准**：
- [ ] 选择文本时显示高亮背景
- [ ] 高亮颜色从主题获取
- [ ] 支持跨行选择

---

### 3.3 添加搜索匹配高亮

**任务描述**：支持搜索结果的高亮显示。

**预估时间**：2-3 小时

**实现步骤**：

1. 从终端获取搜索匹配列表
2. 将匹配范围添加到 `highlighted_ranges`
3. 使用不同颜色区分选择和搜索匹配

**验收标准**：
- [ ] 搜索匹配显示高亮
- [ ] 当前匹配与其他匹配颜色不同
- [ ] 滚动时高亮位置正确

---

## 四、低优先级任务 (P2)

### 4.1 添加对比度调整

**任务描述**：确保文本在背景上可读，自动调整低对比度颜色。

**预估时间**：2 小时

**实现步骤**：

1. 实现 `ensure_minimum_contrast()` 函数
2. 实现 `is_decorative_character()` 检测装饰字符
3. 在 `cell_style()` 中应用对比度调整

**验收标准**：
- [ ] 低对比度文本自动调整
- [ ] Powerline 符号等装饰字符保持原色

---

### 4.2 添加超链接支持

**任务描述**：支持终端中的超链接检测和悬停提示。

**预估时间**：3-4 小时

**验收标准**：
- [ ] 检测 URL 并显示下划线
- [ ] 悬停时显示提示
- [ ] Ctrl+点击打开链接

---

### 4.3 添加 IME 输入支持

**任务描述**：支持输入法预编辑文本显示。

**预估时间**：4-6 小时

**验收标准**：
- [ ] 显示 IME 预编辑文本
- [ ] 正确处理组合字符
- [ ] 光标位置正确

---

## 五、任务总结

### 5.1 任务清单

| 优先级 | 任务 | 预估时间 | 性能收益 |
|--------|------|----------|----------|
| **P0** | 实现文本批处理 (BatchedTextRun) | 4-6h | **10-20x** |
| **P0** | 实现背景区域合并 (BackgroundRegion) | 3-4h | **10-50x** |
| **P0** | 重构绘制流程 (prepaint/paint 分离) | 2-3h | 代码质量 |
| **P1** | 实现视口裁剪优化 | 2-3h | 滚动性能 |
| **P1** | 添加选择高亮支持 | 3-4h | 功能完善 |
| **P1** | 添加搜索匹配高亮 | 2-3h | 功能完善 |
| **P2** | 添加对比度调整 | 2h | 可读性 |
| **P2** | 添加超链接支持 | 3-4h | 功能完善 |
| **P2** | 添加 IME 输入支持 | 4-6h | 功能完善 |

### 5.2 实施顺序建议

```
Phase 1: 性能优化 (P0)
├── 1. 实现 BatchedTextRun
├── 2. 实现 BackgroundRegion
└── 3. 重构 prepaint/paint 分离

Phase 2: 功能增强 (P1)
├── 4. 视口裁剪优化
├── 5. 选择高亮支持
└── 6. 搜索匹配高亮

Phase 3: 完善细节 (P2)
├── 7. 对比度调整
├── 8. 超链接支持
└── 9. IME 输入支持
```

### 5.3 总工时估算

|阶段 | 任务数 | 预估时间 |
|------|--------|----------|
| Phase 1 (P0) | 3 | 9-13小时 |
| Phase 2 (P1) | 3 | 7-10 小时 |
| Phase 3 (P2) | 3 | 9-12 小时 |
| **总计** | **9** | **25-35 小时** |

---

## 参考资料

- [Zed 终端字符绘制逻辑分析](./zed-terminal-paint-analysis.md)
- [Zed 源码](https://github.com/zed-industries/zed/blob/main/crates/terminal_view/src/terminal_element.rs)
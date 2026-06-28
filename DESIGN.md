# DESIGN.md — Opticrum Web Console 设计规范

> 基于 B 端企业级 Dashboard 设计系统，用于 Web Console (`static/`) 的 UI 重构。
> 设计稿基准：1440×900px（PC 优先），参考 Ant Design 5.x 设计语言。

---

## 1. 整体布局结构（Layout Grid）

```
┌──────────────────────────────────────────────────────────┐
│  Header (64px)                                          │
│  Logo + "Tieying.Guo / 元·包销" | 通知图标 | 用户头像/角色 │
├────────────┬─────────────────────────────────────────────┤
│  Sidebar   │  Content Area                              │
│  220px     │  (24-column grid, gap: 16px)               │
│  fixed     │                                            │
│            │  ┌───── KPI Cards Row ─────────────┐      │
│  Nav items │  │ Card1  Card2  Card3  Card4  Card5│      │
│  icons+txt │  └──────────────────────────────────┘      │
│            │                                            │
│  collapsi- │  ┌─ Chart Area ───┐ ┌─ Stats/Feedback ─┐  │
│  ble to    │  │ Line Chart      │ │ Ranking / Stars  │  │
│  64px      │  │ (col-span: 16)  │ │ (col-span: 8)    │  │
│            │  └─────────────────┘ └───────────────────┘  │
│            │                                            │
│            │  ┌─ Table / Detail ──────────────────────┐ │
│            │  │ Orders / Matches Table (col-span: 24) │ │
│            │  └────────────────────────────────────────┘ │
└────────────┴─────────────────────────────────────────────┘
```

### 1.1 Header（顶部导航栏）
| 属性 | 值 |
|------|-----|
| 高度 | 64px |
| 背景 | `#ffffff` 或 `#fafafa` |
| 下边框 | `1px solid #e8e8e8` |
| 左侧 | Logo (32×32px) + 产品名 "Tieying.Guo / 元·包销" (16px, weight 600) |
| 右侧 | 通知图标 (Badge) + 用户头像 (36×36px circle) + 角色名 "齐经理" (14px) |
| Padding | 0 24px |
| 定位 | `position: sticky; top: 0; z-index: 100;` |

### 1.2 Sidebar（左侧导航）
| 属性 | 值 |
|------|-----|
| 宽度 | 展开 220px / 折叠 64px |
| 背景 | `#ffffff` |
| 右边框 | `1px solid #f0f0f0` |
| 菜单项高度 | 48px |
| 图标尺寸 | 18×18px，距左 24px |
| 文字 | 14px，距图标 12px |
| 一级菜单 | 粗体 (weight 600)，不可折叠 |
| 子菜单 | 缩进 20px，hover/active 有背景高亮 `#e6f7ff` |
| Active 态 | 左侧 3px 蓝色竖条 + 背景 `#e6f7ff` + 文字 `#1890ff` |
| 底部 | 折叠按钮 (16px 箭头图标) |
| 定位 | `position: fixed; top: 64px; left: 0; bottom: 0; overflow-y: auto;` |

### 1.3 Content（主内容区）
| 属性 | 值 |
|------|-----|
| 定位 | `margin-left: 220px;` (sidebar 宽度) |
| 最小宽度 | `calc(100vw - 220px)` |
| 背景 | `#f5f5f5` |
| Padding | 24px |
| 栅格 | 24 列 CSS Grid / Flexbox |
| 列间距 | 16px |
| 行间距 | 16px |

---

## 2. 色彩系统（Color Palette）

### 2.1 主色调
```css
:root {
  /* Primary Blue */
  --primary-50:  #e6f7ff;
  --primary-100: #bae7ff;
  --primary-200: #91d5ff;
  --primary-300: #69c0ff;
  --primary-400: #40a9ff;
  --primary-500: #1890ff;  /* 主色 */
  --primary-600: #096dd9;
  --primary-700: #0050b3;

  /* Neutral Gray */
  --gray-50:  #fafafa;
  --gray-100: #f5f5f5;
  --gray-200: #f0f0f0;
  --gray-300: #e8e8e8;
  --gray-400: #d9d9d9;
  --gray-500: #bfbfbf;
  --gray-600: #8c8c8c;
  --gray-700: #595959;
  --gray-800: #434343;
  --gray-900: #262626;

  /* Semantic Colors */
  --success:  #52c41a;  /* 绿色 - 增长/成功 */
  --danger:   #ff4d4f;  /* 红色 - 下降/错误/退单 */
  --warning:  #faad14;  /* 黄色 - 警告 */
  --info:     #1890ff;  /* 蓝色 - 信息 */

  /* Chart Colors */
  --chart-1: #1890ff;
  --chart-2: #52c41a;
  --chart-3: #faad14;
  --chart-4: #ff4d4f;
  --chart-5: #722ed1;
  --chart-6: #13c2c2;

  /* Background */
  --bg-page:      #f5f5f5;
  --bg-card:      #ffffff;
  --bg-sidebar:   #ffffff;
  --bg-header:    #ffffff;

  /* Text */
  --text-primary:   #262626;
  --text-secondary: #595959;
  --text-muted:     #8c8c8c;
  --text-disabled:  #bfbfbf;

  /* Border */
  --border-light: #f0f0f0;
  --border-base:  #e8e8e8;
  --border-dark:  #d9d9d9;

  /* Shadow */
  --shadow-sm:  0 1px 2px rgba(0,0,0,0.04);
  --shadow-base: 0 2px 8px rgba(0,0,0,0.08);
  --shadow-lg:  0 4px 16px rgba(0,0,0,0.12);
}
```

### 2.2 配色原则
- 主色 `#1890ff` 用于按钮、链接、Active 态、图表主系列
- 增长/上升使用 `#52c41a`（绿色箭头 ↑）
- 下降/退单使用 `#ff4d4f`（红色箭头 ↓）
- 文字层级：主文字 `#262626`、辅助文字 `#595959`、说明文字 `#8c8c8c`
- 背景层级：页面 `#f5f5f5` → 卡片 `#ffffff` → 悬浮 `#fafafa`

---

## 3. 字体与排版（Typography）

### 3.1 字体族
```css
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC',
             'Hiragino Sans GB', 'Microsoft YaHei', 'Helvetica Neue',
             Helvetica, Arial, sans-serif;
```
- **中文优先**：PingFang SC（Mac）/ Microsoft YaHei（Windows）
- **等宽字体**（代码/Tx Hash）：`'SF Mono', 'Fira Code', 'Consolas', monospace`

### 3.2 字号与行高
| 层级 | 字号 | 行高 | 用途 |
|------|------|------|------|
| H1 | 24px / 600 | 32px | 页面标题 |
| H2 | 20px / 600 | 28px | 模块标题 |
| H3 | 16px / 600 | 24px | 卡片标题 |
| Body-L | 16px / 400 | 24px | 正文 |
| Body | 14px / 400 | 22px | 通用正文、表格 |
| Caption | 13px / 400 | 20px | 辅助说明、日期、单位 |
| Small | 12px / 400 | 18px | 标签、Badge、次要信息 |
| KPI Number | 32px / 700 | 40px | KPI 大数字 |
| KPI Unit | 14px / 400 | 22px | KPI 单位 |

### 3.3 排版原则
- **行高**：正文 1.5-1.6，标题 1.3-1.4
- **对齐**：文字左对齐，数字右对齐，金额右对齐
- **中文标点**：使用全角中文标点（，。、）
- **数字格式**：千位分隔符逗号 `1,234,567`，货币 `¥1,234.00`

---

## 4. 卡片体系（Card System）

### 4.1 基础卡片
```css
.card {
  background: var(--bg-card);
  border-radius: 8px;
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-base);
  padding: 24px;
}
```
- **圆角**：统一 8px（KPI 卡片可用 12px）
- **阴影**：`0 2px 8px rgba(0,0,0,0.08)` —— 轻微层次感
- **边框**：`1px solid #f0f0f0`
- **Padding**：20-24px
- **卡片间距**：16px（同列）/ 16px（同行）
- **Hover 态**：阴影加深 `0 4px 12px rgba(0,0,0,0.12)`，微上移 2px

### 4.2 KPI 指标卡
```
┌─────────────────────────┐
│  ¥ 本月收益总额          │  ← 图标 + 标题 (14px, --text-muted)
│  ¥955,632.00            │  ← 大数字 (32px, 700, --text-primary)
│  ↑ 16%  较上月上涨       │  ← 环比 (13px, --success / --danger)
│  ●──●──●──● 迷你趋势图  │  ← Sparkline (40px 高)
└─────────────────────────┘
```
- KPI 数字需有 `font-variant-numeric: tabular-nums` 保证数字等宽
- 环比箭头：↑ 绿色 (`#52c41a`) / ↓ 红色 (`#ff4d4f`)
- 背景可用微渐变：`linear-gradient(135deg, #ffffff, #fafafa)`

### 4.3 图表容器卡片
```
┌─────────────────────────────────┐
│  图表标题          [曲线][饼图] │  ← 标题栏：标题(16px) + Tabs(右)
│  ─────────────────────────────  │  ← 分割线
│                                 │
│         Chart Area              │  ← 图表区 (min-height: 300px)
│                                 │
│  ─────────────────────────────  │
│  图例 ···                       │  ← 底部图例
└─────────────────────────────────┘
```
- 右上角 Tab 切换（曲线 / 饼图 / 柱状图）
- 日期范围选择器放在标题栏右侧

---

## 5. 图表规范（Charts）

### 5.1 折线图（Line Chart）
- **用途**：展示趋势数据（订单量变化、收益走势）
- **样式**：平滑曲线 (`tension: 0.4`)，线宽 2px
- **填充**：渐变透明填充 `rgba(24,144,255,0.1)` → `rgba(24,144,255,0)`
- **数据点**：实心圆 4px，Hover 放大至 6px
- **Tooltip**：白色背景 + 阴影，显示日期 + 数值
- **X 轴**：时间段标签（1-5日、6-10日...）
- **Y 轴**：自动缩放，带网格线 `#f0f0f0`

### 5.2 饼图/环形图（Pie/Donut Chart）
- **用途**：构成分析（订单状态分布、渠道占比）
- **样式**：内径 60%（环形），外径 90%
- **颜色**：使用 chart-1 ~ chart-6 色板
- **标签**：百分比显示在外侧，带引导线
- **图例**：右对齐或底部居中，方形色块 + 文字
- **中心文字**：总计数值（可选）

### 5.3 柱状图（Bar Chart）
- **用途**：对比数据（月度收入对比、渠道对比）
- **样式**：分组柱，柱宽 60%，组内间距 4px
- **圆角**：柱顶 4px 圆角
- **Hover**：柱体颜色加深 10%，显示 Tooltip
- **X 轴**：月份/类别
- **Y 轴**：金额/数量

### 5.4 水平条形图（Horizontal Bar）
- **用途**：排名、评分分布
- **样式**：条形高度 20px，圆角 4px
- **标签**：左对齐类别名，右对齐数值
- **示例**：星级反馈统计（★★★★★ 85%）

---

## 6. 组件库（Components）

### 6.1 按钮（Button）
| 类型 | 背景 | 文字色 | 边框 | Hover |
|------|------|--------|------|-------|
| 主按钮 (Primary) | `#1890ff` | `#fff` | 无 | `#40a9ff` |
| 次按钮 (Default) | `#fff` | `#262626` | `#d9d9d9` | `#1890ff` 边框 + `#e6f7ff` 背景 |
| 文字按钮 (Text) | 透明 | `#1890ff` | 无 | `#e6f7ff` 背景 |
| 危险按钮 (Danger) | `#ff4d4f` | `#fff` | 无 | `#ff7875` |
| 尺寸 | Small: 24px / Default: 32px / Large: 40px |
| 圆角 | 6px |
| 字体 | 14px |

### 6.2 表格（Table）
```css
.table {
  width: 100%;
  border-collapse: collapse;
}
.table th {
  background: #fafafa;
  color: var(--text-secondary);
  font-weight: 500;
  font-size: 13px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-light);
  text-align: left;
}
.table td {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-light);
  font-size: 14px;
}
.table tr:hover td {
  background: #fafafa;
}
```
- 表头固定（sticky），表格可滚动
- 排序箭头在列头右侧（▲▼）
- 支持行选择（Checkbox 首列）
- 分页器在表格底部居中

### 6.3 表单（Form）
- Label 左对齐，14px，`--text-secondary`
- Input 高度 32px，边框 `1px solid #d9d9d9`，圆角 6px
- Focus 态：边框 `#1890ff`，外发光 `0 0 0 2px rgba(24,144,255,0.2)`
- 错误态：边框 `#ff4d4f`，下方红色提示文字
- 表单项间距 24px

### 6.4 标签/徽标（Tag / Badge）
- 圆角 4px，padding 2px 8px，font-size 12px
- 颜色变体：blue / green / red / yellow / gray
- 状态映射：Live→green, Exhausted→gray, Destroyed→red, Pending→yellow

### 6.5 图标（Icons）
- **推荐图标库**：Ant Design Icons（SVG），或 Lucide Icons
- **尺寸**：16px（行内）、20px（菜单）、24px（大图标）
- **颜色**：继承文字颜色，或使用 `--text-muted`
- **常用图标**：📊 Dashboard, 💰 Wallet, 📋 Orders, 🔗 Channels, ⚡ Matches, ⚙️ Settings

### 6.6 评分组件（Rate / Stars）
- 5 颗星，支持半星
- 默认色 `#faad14`
- 右侧显示平均分 + 评价数：`4.8 (128 条评价)`

---

## 7. 页面结构重组（Page Restructure）

### 7.1 导航菜单层级
```
📊 数据中心 (Dashboard)
  ├── 运营概览
  └── 收益统计
📋 订单数据
  ├── 链上订单
  ├── 匹配记录
  └── 历史订单
💼 资金管理
  ├── 钱包管理
  └── 提取记录
⚙️ 系统设置
  ├── Fiber 通道
  ├── 自动匹配
  └── 外部签名
```

### 7.2 各页面内容规划

#### Dashboard（运营概览）
- **第一行**：5 个 KPI 指标卡（总匹配数、本月收益、活跃订单、可用通道、提取总额）
- **第二行**：折线图（收益趋势，col-span 16）+ 饼图（订单状态分布，col-span 8）
- **第三行**：柱状图（月度匹配量，col-span 14）+ 反馈/排行榜（col-span 10）

#### 订单数据 / 链上订单
- 筛选栏（状态 / 容量范围 / 时间区间）
- 数据表格（Tx Hash, Capacity, Rate, Status, Actions）
- 分页器

#### 钱包管理
- 卡片式钱包列表（每个钱包一张小卡：地址、余额、标签）
- 导入表单（Modal 弹窗）

---

## 8. 响应式策略（Responsive）

| 断点 | 布局调整 |
|------|----------|
| ≥1440px | 完整布局，Sidebar 展开 220px，24 列栅格全显 |
| 1200–1439px | KPI 卡片由 5 列变 4 列，图表区 col-span 调整 |
| 992–1199px | Sidebar 折叠为 64px（仅图标），KPI 卡片 3 列 |
| 768–991px | Sidebar 隐藏（Hamburger Menu），单列堆叠 |
| <768px | 全屏模式，表格横向滚动，KPI 卡片 2 列 |

---

## 9. 交互规范（Interaction）

### 9.1 过渡动画
```css
transition: all 0.2s cubic-bezier(0.645, 0.045, 0.355, 1);
```
- Hover 态：0.15-0.2s
- Modal 弹出：0.3s，带缩放 + 淡入
- Sidebar 折叠：0.3s，宽度过渡
- 图表切换：0.3s 淡入淡出

### 9.2 数据刷新
- 自动刷新：Dashboard 每 30s 轮询
- 手动刷新：刷新按钮 + 旋转动画
- 加载态：骨架屏 (Skeleton) 或 Spinner
- 空状态：插图 + "暂无数据" 提示文字

### 9.3 反馈
- 操作成功：顶部绿色 Toast（2s 自动消失）
- 操作失败：顶部红色 Toast + 错误信息
- 确认操作：Modal 对话框（"确定删除此钱包？"）
- 长时间操作：按钮 Loading 态（Spin + 禁用）

---

## 10. 实现建议

### 10.1 技术选型
- **CSS 方案**：CSS Variables（设计 Token）+ 语义化 class
- **图表库**：Chart.js 4.x（轻量，无框架依赖）或 ECharts（更丰富的中文支持）
- **图标**：SVG Sprite 或 Icon Font
- **无框架依赖**：纯 HTML/CSS/JS（与当前架构一致）

### 10.2 文件结构
```
static/
├── index.html          # 主页面（重构为侧边栏 + 内容布局）
├── css/
│   ├── variables.css   # CSS 变量（颜色/字体/间距）
│   ├── layout.css      # 布局（Header/Sidebar/Content/Grid）
│   ├── components.css  # 组件（Card/Button/Table/Form/Tag/Modal）
│   └── pages.css       # 页面特定样式
├── js/
│   ├── app.js          # 主逻辑（导航/路由）
│   ├── api.js          # API 请求封装
│   ├── charts.js       # 图表初始化与更新
│   └── components.js   # 通用组件（Toast/Modal/Loading）
└── assets/
    ├── logo.svg
    └── icons/          # SVG 图标
```

### 10.3 迁移策略
1. 先建 CSS 变量文件，替换现有硬编码颜色
2. 重构 HTML 布局结构（Header + Sidebar + Content）
3. 逐个页面迁移到新卡片系统
4. 引入图表库，Dashboard 图表替换统计数字
5. 添加响应式断点和交互动效

---

## 11. 检查清单（Checklist）

- [ ] Header：Logo + 产品名 + 用户头像
- [ ] Sidebar：可折叠，图标+文字，Active 高亮
- [ ] KPI 卡片：大数字 + 环比 + Sparkline
- [ ] 图表：折线/饼图/柱状图，统一配色
- [ ] 表格：排序、Hover 高亮、分页
- [ ] 卡片圆角 8px，阴影统一
- [ ] 按钮圆角 6px，主/次/危险三态
- [ ] 中文标点，数字千位分隔
- [ ] 响应式：≥1440 / 1200 / 992 / 768 / <768
- [ ] 过渡动画 0.2s
- [ ] 空状态、加载态、错误态处理

---

*版本: v1.0 | 更新: 2026-06-27*

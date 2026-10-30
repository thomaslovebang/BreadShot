# FocusFlow Phase 7 技术设计

> Phase 9 is the current product stage. This document is the Phase 7 design record and is unchanged below.

## 目标

Phase 7 将 Phase 6.2 的“自动缩放录屏”升级为第一版“本地智能导演”。用户仍然只需录制一次，FocusFlow 自动生成更自然的镜头、点击反馈和声音，并允许在应用内预览、切换风格后重新生成。

Phase 7 仍然完全本地、免费、无水印，不引入云服务、账号系统、视觉识别或完整视频剪辑器。

## 用户流程

1. 用户选择导演风格并开始录制。
2. FocusFlow 记录屏幕与鼠标行为。
3. 结束后应用约 1 秒重新出现，后台生成默认成片。
4. 成片生成后直接在应用内预览。
5. 用户可以切换风格并重新生成，不需要重新录制。
6. 用户接受结果后直接打开成片所在位置。

## Phase 7 功能边界

### 1. Director Profiles

提供三个内置风格：

- `Clean`：克制、稳定、适合开发工具和 SaaS 演示；
- `Dynamic`：更明显的推进和更快的镜头移动；
- `Soft`：较低缩放倍率和更柔和的过渡。

风格只影响参数，不复制三套算法。参数包括停留阈值、点击/停留缩放倍率、进入/退出速度、保持时间、镜头合并距离、点击反馈强度和音效音量。

### 2. Camera Director 2.0

- 为快速连续点击设置冷却和区域合并，避免反复抽动；
- 相邻关注点优先平移，不频繁恢复全屏；
- 远距离移动根据距离自动延长转场；
- 为镜头中心增加低通平滑和安全边界；
- 录制结尾保证自然恢复全屏；
- 保持 Phase 6.2 的真实时间戳和时长保护。

### 3. Click Effects

- 左键点击：短促的圆形扩散反馈；
- 右键点击：视觉上可区分但保持克制；
- 鼠标高亮仅在点击附近短暂出现，不长期遮挡界面；
- 特效坐标随 Camera Timeline 变换，缩放或移动镜头时仍对准真实点击位置。

### 4. Click Sound

- 使用项目自带、可自由分发的短音效；
- 根据点击时间戳混入音轨；
- 原视频没有音轨时创建音轨，有音轨时进行混音；
- 默认音量克制，风格预设可以调整或关闭；
- 音频长度严格受原视频时长限制。

### 5. Result Preview

- 应用内播放最终成片；
- 显示生成状态、原片时长和成片时长；
- 支持在原始视频和最终成片之间切换；
- 支持切换风格并重新生成；
- 支持打开输出文件夹。

## 数据设计

不破坏现有 `camera.json` schema 1。Phase 7 新增独立的项目描述文件：

```json
{
  "schema_version": 1,
  "source_video": "recording.mp4",
  "mouse_trace": "recording.mouse.jsonl",
  "camera_timeline": "recording.camera.json",
  "final_video": "recording.final.mp4",
  "director_profile": "clean",
  "effects": {
    "click_visual": true,
    "click_sound": true
  }
}
```

点击特效时间线从鼠标轨迹派生，不写入 Camera Timeline。这样镜头职责与视觉/音频特效职责分离，Phase 8 的手动编辑器可以复用同一项目文件。

## 架构调整

```text
mouse.jsonl
   ├── Focus Engine 2.0 ──> camera.json
   └── Effect Engine ─────> click effect timeline

project.json + source.mp4
             │
             v
      Composite Renderer
       ├── camera transform
       ├── click visuals
       ├── click audio
       └── duration guard
             │
             v
          final.mp4
```

新增模块职责：

- `director_profile`：风格名称到算法/渲染参数的确定性映射；
- `effect_engine`：从鼠标事件生成点击视觉和音频事件；
- `project_store`：保存可重新生成的项目描述；
- `renderer`：从单一 zoompan 扩展为组合渲染管线。

## 兼容与安全策略

- 旧的 Phase 6.2 鼠标轨迹和 Camera Timeline 继续可读取；
- 原始视频永远不覆盖；
- 重新生成先输出临时文件，成功后再替换最终成片；
- 渲染失败时保留上一版成功成片；
- 每次渲染记录 FFmpeg 日志和所选风格；
- 自动比较原片与成片时长，超出容差则判定生成失败；
- 所有功能离线执行，不上传屏幕、鼠标或视频数据。

## 实施顺序

### Phase 7A：导演风格与运镜升级

- 建立 Director Profile 数据模型；
- 改进镜头合并、距离自适应转场与防抖；
- 增加单元测试和 Camera Timeline 回归样例。

验收：同一轨迹可以稳定生成三种不同风格；连续点击不抽动；时长不变化。

### Phase 7B：点击视觉与音效

- 建立 Effect Timeline；
- 合成左右键点击动画；
- 混入本地点击音效；
- 校验特效与缩放后鼠标位置对齐。

验收：点击反馈准确、克制，不出现位置漂移或音画不同步。

### Phase 7C：应用内预览与重新生成

- 增加结果预览页；
- 切换原片/成片；
- 切换风格后后台重新生成；
- 显示进度、错误与输出入口。

验收：不重新录制即可更换风格并看到新成片。

### Phase 7D：整体回归与质量调优

- 1080p、2K、多显示器和长视频回归；
- 时长、同步、黑边、抖动和失败恢复测试；
- 固化 Phase 7 测试说明与备份。

## Phase 7 完成标准

- 用户录一次即可获得带自然运镜、点击反馈和音效的成片；
- 三种风格有清楚区别，但都不会显得夸张；
- 成片与原片时长一致，无跳秒、黑边、明显抖动；
- 应用内可以预览并在不重新录制的情况下重新生成；
- Phase 6.2 的录制、快捷键、隐藏/恢复和原始视频保存能力全部保持正常。


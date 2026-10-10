# zitie 字帖生成器

Web 版字帖(copybook)生成器:输入任意文字,生成可打印的 A4 PDF 字帖。支持汉字、英文及更多语言,无需注册。

灵感来自 [钟毓汉字字帖生成器](https://gitee.com/pollybird/zhongyu_chinese_copybook) 与 [钟毓英语衡水体字帖生成器](https://gitee.com/pollybird/zhongyu_english_copybook_-hengshui)(均为 Electron 桌面应用),本项目把同样的能力做成了自托管的 Web 服务,排版与 PDF 生成完全在 Rust 服务端完成。

## 功能

- **六种纸型**:米字格 / 田字格 / 回宫格 / 作文纸 / 四线三格 / 单横线
- **四种模式**:描红(浅色字供描写)、抄写(空格,可选行首示范字)、描红+抄写、纯字帖
- **拼音标注**:汉字格子上方标注带声调拼音(多音字取常用读音,不做上下文消歧)
- **任意语言**:按 Unicode 字素簇(grapheme cluster)逐字入格——汉字、英文、谚文、emoji 均可;字素内部经 rustybuzz 整形,支持印度系文字记号定位
- **RTL 与竖排**:阿拉伯文、希伯来文等自动(或手动指定)从右向左排列;传统竖排(列内从上到下、列序从右向左)适用于全部方块格纸型,作文纸竖排即是古籍/诗词格式
- **自定义字体**:内置开源字体(霞鹜文楷 / Short Stack / Andika),支持上传 .ttf/.otf(衡水体、阿拉伯字体等请自行上传)
- **页眉**:标题 + 最多 3 个自定义字段(班级/姓名/学号,空值画横线)+ 页码
- **生成历史**:最近 50 次生成按用户保存,可一键恢复完整参数
- **无注册**:匿名 cookie 关联偏好设置与上传的字体,偏好存 SQLite
- **浏览器缓存**:PDF 按参数内容寻址(`sha256(规范化参数)`),`immutable + ETag`,同参数字帖只计算一次
- **中英界面**:按浏览器语言自动适配,右上角可切换(服务端错误消息为中文)

## 技术栈

Rust · Axum · SeaORM + SQLite(启动时自动迁移)· krilla(PDF)· skrifa / rustybuzz(字体度量与整形)· 原生 HTML/JS/CSS 前端(无构建步骤)

排版引擎(`src/layout/`)是纯计算,输出中间表示(IR),渲染层(`src/render/`)把 IR 翻译成 krilla 调用——两层通过 IR 解耦,均可独立单测。渲染确定性:同一参数永远产出字节相同的 PDF。

## 快速开始

```bash
# 1. 下载内置字体(约 26MB,仅首次;OFL 等开源协议)
./scripts/fetch-fonts.sh

# 2. 启动
cargo run --release
# 默认监听 0.0.0.0:8080,打开 http://localhost:8080
```

数据(SQLite、上传字体、PDF 缓存)都落在 `./data` 目录。

### 配置(环境变量)

| 变量 | 默认 | 说明 |
|---|---|---|
| `ZITIE_ADDR` | `0.0.0.0:8080` | 监听地址 |
| `ZITIE_DATA_DIR` | `data` | 数据目录 |
| `ZITIE_ASSETS_DIR` | `assets` | 内置字体目录 |
| `ZITIE_STATIC_DIR` | `static` | 前端文件目录 |
| `ZITIE_CACHE_QUOTA_MB` | `500` | PDF 磁盘缓存配额,超限按 LRU 淘汰 |
| `ZITIE_SECURE_COOKIE` | `false` | https 部署时设为 `1` |

### Docker

```bash
docker build -t zitie .
docker run -d -p 8080:8080 -v zitie-data:/data zitie
```

### 发布(GitHub Actions)

- `main` 分支 push / PR 自动跑 fmt + clippy + 全量测试(`.github/workflows/ci.yml`)
- 打 `v*` tag 自动构建发布(`.github/workflows/release.yml`):
  - 五平台二进制包(linux amd64/arm64 musl、macOS 双架构、windows x64),内含前端、内置字体与许可证,解压即用
  - 多架构 Docker 镜像(amd64 + arm64)推送 `ghcr.io/dwdcth/zitie`:由预编译 musl 二进制装配(`Dockerfile.release`,纯 COPY 无编译,秒级);本地源码构建用主 `Dockerfile`

```bash
git tag v0.1.0 && git push origin v0.1.0   # 触发一次发布
```

## REST API

| 方法/路径 | 说明 |
|---|---|
| `GET/PUT /api/preferences` | 偏好读写(匿名 cookie 关联) |
| `GET /api/fonts` | 字体列表(内置 + 本人上传) |
| `POST /api/fonts` | 上传字体(multipart 字段 `font`,≤20MB,.ttf/.otf/.ttc) |
| `DELETE /api/fonts/{id}` | 删除本人上传的字体 |
| `POST /api/generate` | 生成:参数校验 + 布局,返回 `{url, hash, pages}`;写入生成历史 |
| `GET /api/pdf/{hash}.pdf` | 获取 PDF(内容寻址,`immutable` 缓存;`?dl=1` 下载) |
| `GET /api/history` | 最近 50 条生成历史 |
| `GET /api/history/{id}` | 取单条历史的完整参数(恢复用) |
| `DELETE /api/history/{id}` | 删除历史条目 |

示例:

```bash
curl -c jar -X POST localhost:8080/api/generate \
  -H 'content-type: application/json' \
  -d '{"text":"永和九年","paper":"mizi","mode":"miaohong","font_id":"lxgw-wenkai"}'
# → {"url":"/api/pdf/….pdf","hash":"…","pages":1}
curl -b jar -OJ localhost:8080/api/pdf/….pdf?dl=1
```

参数结构见 `src/layout/model.rs` 的 `GenerateParams`(全字段可省略,均有默认值)。

## 内置字体与版权

| 字体 | 协议 | 用途 |
|---|---|---|
| 霞鹜文楷 [lxgw/LxgwWenKai](https://github.com/lxgw/LxgwWenKai) | OFL 1.1 | 中文默认(楷书风格) |
| Short Stack(Google Fonts) | OFL | 英文默认(教学印刷体) |
| Andika(SIL) | OFL | 英文备选(识字教学体,x 高度清晰) |

字体二进制不入库,由 `scripts/fetch-fonts.sh` 下载并校验 sha256;许可证文本在 `assets/fonts/OFL-*.txt`。衡水体、田英章楷书等商用字体请用户自行上传,本项目不分发。

## 开发

```bash
cargo test          # 排版黄金值 / PDF 冒烟 / API 集成(含拼音、RTL、历史)
cargo clippy --all-targets
```

## 许可

Apache-2.0

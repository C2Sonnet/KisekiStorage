# Windowsでのビルド方法（開発・保守者向け）

この内容は開発・保守者向けです。一般利用者への製品案内は [README](../README.md) を参照してください。

## 必要な環境

- Windows 10またはWindows 11
- Rust（stable、MSVC toolchain推奨）
- Visual Studio Build ToolsのC++開発ツールとWindows SDK
- ビルド時にcrates.ioへアクセスできるネットワーク環境（依存クレートが未取得の場合）

## ビルド

PowerShellでリポジトリのルートへ移動します。

```powershell
cargo build --release
```

または `build_release.bat` を実行します。

生成される実行ファイル：

```text
target\release\kiseki-storage.exe
```

UACマニフェスト（管理者実行要求）とアプリのICOは、Windows向けビルド時にリソースとして埋め込まれます。Zen Maru GothicのRegular/BoldもソースからEXEへ埋め込まれるため、フォントは別途インストール不要です。

## 簡易テスト

```powershell
cargo test
```

その後、exeを起動し、UAC、Zen Maru Gothicの表示、ウィンドウサイズ、カードのクリック、スクロール、RAM/Disk更新、5つのボタンをWindows上で確認してください。

**v1.2.0の確認ポイント**

- v1.0.0で発生した `E0599 / ctx.content_rect()` が再発しないこと
- 日本語・英数字、強調見出し、容量バー、タグが正しく描画されること
- 存在しないA:・B:・Z:ドライブに触れてもアプリが停止せず、案内が表示されること
- OneDriveの同期先が未設定の場合に案内が表示されること
- エクスプローラー起動時に意図しない別プログラムが実行されないこと
- ネットワークドライブが応答しない場合に、UIが操作可能なままであること

未検証のビルドであるため、Rustのエラー・警告が出た場合はその全文を収集し修正してください。

Cargo.lockは、再現可能性のためWindowsで初回ビルド後に生成・コミットしてください。現行のソースZIPにはCargo.lockとEXEは同梱されていません。

## 名前とバージョン

- `Cargo.toml` の`name`は `kiseki-storage` です。
- `Cargo.toml` の`version`を更新すると、ウィンドウタイトル内のバージョン表示も変わります。
- Windowsリソース（`assets/app.manifest`）は、リリース時にバージョンや名称を確認してください。

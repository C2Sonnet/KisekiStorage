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

UACマニフェスト（管理者実行要求）とアプリのICOは、Windows向けビルド時にリソースとして埋め込まれます。

## 簡易テスト

```powershell
cargo test
```

その後、exeを起動し、UAC、ウィンドウサイズ、カードのクリック、スクロール、RAM/Disk更新、5つのボタンをWindows上で確認してください。

Cargo.lockは、再現可能性のためWindowsで初回ビルド後に生成・コミットしてください。現行のソースZIPにはCargo.lockとEXEは同梱されていません。

## 名前とバージョン

- `Cargo.toml` の`name`は `kiseki-storage` です。
- `Cargo.toml` の`version`を更新すると、ウィンドウタイトル内のバージョン表示も変わります。
- Windowsリソース（`assets/app.manifest`）は、リリース時にバージョンや名称を確認してください。

# 2024年度卒業研究

## 手順

### サーバの準備

#### 1. Realsenseのudevルールの設定（初回のみ）

以下のコマンドを実行してRealsenseのudevルールをインストールしてください．

- リポジトリの追加

```sh
sudo mkdir -p /etc/apt/keyrings
curl -sSf https://librealsense.intel.com/Debian/librealsense.pgp | sudo tee /etc/apt/keyrings/librealsense.pgp > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/librealsense.pgp] https://librealsense.intel.com/Debian/apt-repo `lsb_release -cs` main" | \
sudo tee /etc/apt/sources.list.d/librealsense.list
sudo apt-get update
```

- udevルールのインストール

```sh
sudo apt-get install -y librealsense2-udev-rules
```

#### 2. 環境構築

ビルド・実行環境の構築はNixで行います．[nix-installer](https://github.com/DeterminateSystems/nix-installer)の手順に従ってNixをインストールしてください．
Nixをインストールしたら，以下のコマンドで環境に入れます．初回は時間がかかります．

```sh
cd sotsuken2024
nix develop
```

#### 3. IPアドレスの確認

以下のコマンドを実行してサーバのIPアドレスを確認してください．あとでクライアントから接続する際に必要です．

```sh
ip a
```

#### 4. サーバのビルド・実行

サーバは以下のコマンドでビルド・実行できます．
Realsense D435iを接続してから実行してください．

```sh
cargo run --release -p vrrop_server serve
```

Ctrl+Cでサーバを終了できます．

### クライアントの準備

#### 1. クライアントのインストール

ビルド済みクライアントは[リリースページ](https://github.com/Pylgos/sotsuken2024/releases/tag/v1.0.0)からダウンロードできます．
ADBなどでMeta Questにインストールしてください．

#### 2. サーバへの接続

1. クライアントアプリ（VRROP Server）を起動する．
1. 左コントローラに表示される設定パネルの「Server Address」欄に右コントローラを向け，右コントローラのトリガーを押して選択する，
1. ポップアップしたキーボードにサーバのIPアドレスを入力する．

#### 3. 操作方法

- 左グリップ+左スティック前後左右:　左コントローラの方向に移動
- 左グリップ+右スティック左右: スナップターン

## クライアントのビルド

クライアントをソースコードからビルドする場合はは以下の手順に従ってください．

1. ライブラリのビルド

```sh
cargo build --release -p godot_vrrop_client
cargo build --release -p godot_vrrop_client --target aarch64-linux-android
```

1. APKのビルド  
godot_vrrop_client/projectディレクトリをGodotエディタで開き，[公式ドキュメント](https://docs.godotengine.org/ja/4.3/tutorials/export/exporting_for_android.html)に従ってエクスポートしてください．

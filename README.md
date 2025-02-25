# 2024年度卒業研究

## 環境構築

ビルド・実行環境の構築はNixで行います．[nix-installer](https://github.com/DeterminateSystems/nix-installer)の手順に従ってNixをインストールしてください．
Nixをインストールしたら，以下のコマンドで環境に入れます．初回は時間がかかります．

```sh
nix develop
```

## Realsenseのudevルールの設定

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

## サーバのビルド・実行

サーバは以下のコマンドでビルド・実行できます．
Realsense D435iを接続してから実行してください．

```sh
cargo run --release -p vrrop_server serve
```

Ctrl+Cでサーバを終了できます．

## クライアントのビルド

クライアントは以下の手順でビルドできます．

- ライブラリのビルド

```sh
cargo build --release -p godot_vrrop_client
cargo build --release -p godot_vrrop_client --target aarch64-linux-android
```

- APKのビルド
godot_vrrop_client/projectディレクトリをGodotエディタで開き，[公式ドキュメント](https://docs.godotengine.org/ja/4.3/tutorials/export/exporting_for_android.html)に従ってエクスポートしてください．

## サーバへのの接続

- サーバのIPアドレスを確認する．
- クライアントアプリを起動する．
- 左手に設定パネルが表示されるので，IPアドレス入力欄を右手のトリガーで選択し，ポップアップするキーボードでサーバのIPアドレスを入力する．

## 操作方法

- 左グリップ+左スティック前後左右:　左コントローラの方向に移動
- 左グリップ+右スティック左右: スナップターン

import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from matplotlib import rcParams

# 日本語フォントの設定
rcParams["font.family"] = "Noto Sans CJK JP"  # 環境に応じてフォント名を調整してください
rcParams["axes.unicode_minus"] = False  # マイナス記号が文字化けしないように設定

# CSVファイルの読み込み
odometry_file = "stats/odometry.csv"
images_file = "stats/images.csv"

odometry_data = pd.read_csv(odometry_file)
images_data = pd.read_csv(images_file)

# タイムスタンプの基準を画像データの2つ目のタイムスタンプに調整
start_time = images_data["stamp"].iloc[1]  # 2つ目のタイムスタンプ
odometry_data["stamp"] -= start_time
images_data["stamp"] -= start_time

# タイムスタンプを基準に統合
combined_timestamps = np.union1d(odometry_data["stamp"], images_data["stamp"])

# レイテンシ (ms) をプロット
plt.figure(figsize=(12, 8))  # グラフのサイズを大きく

# オドメトリのレイテンシをプロット
plt.plot(
    odometry_data["stamp"],
    odometry_data["latency"] * 1000,  # 秒からミリ秒に変換
    label="オドメトリのレイテンシ (ms)",
    color="blue",
    linestyle="--",
    linewidth=2,
)

# 画像のレイテンシをプロット
plt.plot(
    images_data["stamp"],
    images_data["latency"] * 1000,  # 秒からミリ秒に変換
    label="画像のレイテンシ (ms)",
    color="orange",
    linestyle="--",
    linewidth=2,
)

# グラフの装飾
plt.title(
    "レイテンシの変化（画像データ2つ目のタイムスタンプ基準）",
    fontsize=18,
)
plt.xlabel(
    "時間 (秒)",
    fontsize=16,
)
plt.ylabel(
    "レイテンシ (ms)",
    fontsize=16,
)
plt.legend(
    fontsize=14,
    loc="upper right",
)
plt.grid(True)

# 軸の範囲を調整（横軸をデータ範囲にピッタリ合わせる）
plt.xlim(0, combined_timestamps.max())  # 横軸: 0秒から最大値
plt.ylim(
    0, max(odometry_data["latency"].max(), images_data["latency"].max()) * 1000
)  # 最大値に基づいて縦軸を設定

# グラフの表示
plt.tight_layout()
plt.show()
